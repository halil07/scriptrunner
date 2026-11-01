//! sr — Ctrl+P ile script seçip argüman vererek çalıştıran terminal uygulaması.

mod app;
mod config;
mod discovery;
mod help;
mod runner;
mod ui;
mod util;

use std::io::{self, stdout, Write};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event, EnableBracketedPaste, DisableBracketedPaste};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;

use app::{App, AppEvent};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(|s| s.as_str()) {
        None | Some("-t") | Some("--tui") => run_tui(),
        Some("-l" | "--list") => cmd_list(),
        Some("-d" | "--dir") => {
            match config::AppConfig::load() {
                Ok(cfg) => println!("{}", cfg.scripts_dir.display()),
                Err(e) => eprintln!("yapılandırma yüklenemedi: {e}"),
            }
            0
        }
        Some("-o" | "--open") => {
            match config::AppConfig::load() {
                Ok(cfg) => {
                    #[cfg(windows)]
                    {
                        let _ = std::process::Command::new("explorer")
                            .arg(&cfg.scripts_dir)
                            .spawn();
                    }
                    #[cfg(not(windows))]
                    {
                        println!("{}", cfg.scripts_dir.display());
                    }
                }
                Err(e) => eprintln!("yapılandırma yüklenemedi: {e}"),
            }
            0
        }
        Some("run" | "--run") => cmd_run(&args[1..]),
        Some("-h" | "--help" | "help") => {
            help();
            0
        }
        Some(unknown) => {
            eprintln!("bilinmeyen argüman: {unknown}\n");
            help();
            2
        }
    };
    std::process::exit(code);
}

fn help() {
    println!("{}", help::text());
}

fn cmd_list() -> i32 {
    let cfg = match config::AppConfig::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("yapılandırma yüklenemedi: {e}");
            return 1;
        }
    };
    let scripts = discovery::scan(&cfg);
    if scripts.is_empty() {
        println!("script yok — {}", cfg.scripts_dir.display());
        return 0;
    }
    for s in &scripts {
        let key = if s.key.len() < 20 { format!("{:<20}", s.key) } else { s.key.clone() };
        let mark = if s.available { " " } else { "!" };
        println!("{mark} {} [{}] {}", key, s.lang.label(), s.description);
        if !s.available {
            if let Some(r) = &s.missing_reason {
                println!("    ↳ {r}");
            }
        }
    }
    0
}

fn cmd_run(args: &[String]) -> i32 {
    let cfg = match config::AppConfig::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("yapılandırma yüklenemedi: {e}");
            return 1;
        }
    };
    let scripts = discovery::scan(&cfg);
    let Some(key) = args.first() else {
        eprintln!("kullanım: sr run <key> [args...]\nanahtarları görmek için: sr --list");
        return 2;
    };
    let Some(script) = discovery::find_key(&scripts, key) else {
        eprintln!("script bulunamadı: {key} (--list'e bak)");
        return 2;
    };
    if !script.available {
        eprintln!("{}", script.missing_reason.clone().unwrap_or_else(|| "kullanılamaz".into()));
        return 3;
    }
    let args_line = args[1..].join(" ");
    match runner::run_headless(script, &cfg, &args_line) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("hata: {e:#}");
            4
        }
    }
}

fn run_tui() -> i32 {
    match run_tui_inner() {
        Ok(_) => 0,
        Err(e) => {
            // terminal durumunu kabaya çevirip hatayı yaz
            let _ = leave_tui();
            eprintln!("hata: {e:#}");
            1
        }
    }
}

fn run_tui_inner() -> Result<()> {
    let Some(cfg) = startup_config()? else {
        return Ok(());
    };

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let _ = stdout().execute(EnableBracketedPaste);

    let result = tui_loop(cfg);

    let _ = leave_tui();
    result
}

fn leave_tui() -> Result<()> {
    let _ = stdout().execute(DisableBracketedPaste);
    let _ = stdout().execute(LeaveAlternateScreen);
    disable_raw_mode()?;
    Ok(())
}

fn startup_config() -> Result<Option<config::AppConfig>> {
    let config_path = config::AppConfig::config_file_path();
    let config_missing = !config_path.is_file();
    let marker_path = config_path.with_file_name(".startup-check-complete");
    let is_first_start = !marker_path.is_file();
    if is_first_start {
        println!("sr ilk kurulum kontrolü\n");
    }
    let create_config = if config_missing {
        println!("Yapılandırma dosyası bulunamadı:");
        println!("  {}", config_path.display());
        confirm("Örnek config.toml oluşturulsun mu? [E/h]: ")?
    } else {
        false
    };

    let cfg = config::AppConfig::load_with_config_creation(create_config)?;

    if is_first_start {
        println!("\nSistem gereksinimleri:");
        let checks = [
            ("Python", cfg.python.as_str()),
            ("Node.js", cfg.node.as_str()),
            ("Rust", cfg.rustc.as_str()),
        ];
        let mut missing = Vec::new();
        for (name, executable) in checks {
            match util::find_in_path(executable) {
                Some(path) => println!("  ✓ {name:<8} {}", path.display()),
                None => {
                    println!("  ✗ {name:<8} {executable} bulunamadı");
                    missing.push(name);
                }
            }
        }

        if missing.is_empty() {
            println!("\nGerekli yorumlayıcılar bulundu.");
        } else {
            println!("\nEksik: {}.", missing.join(", "));
            println!("İlgili dili kullanmak için yükleyin veya config.toml içindeki [interpreters] bölümüne yolunu yazın.");
            println!("sr bu araçları otomatik yüklemez; diğer diller kullanılabilir.");
            if !confirm("Eksikler olmadan ana ekrana devam edilsin mi? [E/h]: ")? {
                println!("Başlatma iptal edildi. Eksik yorumlayıcıları kurup tekrar deneyin.");
                return Ok(None);
            }
        }
        println!("\nAna ekrana geçmek için Enter'a basın.");
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        std::fs::write(marker_path, "checked\n")?;
    }

    Ok(Some(cfg))
}

fn confirm(prompt: &str) -> Result<bool> {
    print!("{prompt}");
    stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "" | "e" | "evet" | "y" | "yes"
    ))
}

fn tui_loop(cfg: config::AppConfig) -> Result<()> {
    let (tx, rx) = mpsc::channel::<AppEvent>();
    let event_tx = tx.clone();
    let mut app = App::new(cfg, tx);

    let mut terminal = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    // Tuş olayları ayrı iş parçacığında toplanır.
    let _event_thread = std::thread::spawn(move || loop {
        if crossterm::event::poll(Duration::from_millis(50)).unwrap_or(false) {
            match crossterm::event::read() {
                Ok(ev) => {
                    let mapped = match ev {
                        Event::Key(k) => Some(app::AppEvent::Key(k)),
                        Event::Paste(t) => Some(app::AppEvent::Paste(t)),
                        _ => None,
                    };
                    if let Some(m) = mapped {
                        if event_tx.send(m).is_err() {
                            break;
                        }
                    }
                }
                Err(_) => break,
            }
        }
    });

    loop {
        // birden fazla bekleyen olayı işle
        for _ in 0..32 {
            match rx.recv_timeout(Duration::from_millis(25)) {
                Ok(m) => app.app_event(m),
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        app.tick();

        terminal.draw(|f| ui::draw(f, &app))?;

        if app.quit {
            break;
        }
    }

    // Çıkarken çalışan süreci bırak
    if let Some(job) = app.job.as_mut() {
        if job.is_running() {
            job.kill();
        }
    }
    let _ = terminal.clear();
    Ok(())
}
