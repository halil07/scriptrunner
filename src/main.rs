//! sr — Ctrl+P ile script seçip argüman vererek çalıştıran terminal uygulaması.

mod app;
mod config;
mod discovery;
mod help;
mod i18n;
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
                Err(e) => eprintln!("{}: {e}", crate::i18n::text("Yapılandırma yüklenemedi", "Could not load configuration")),
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
                Err(e) => eprintln!("{}: {e}", crate::i18n::text("Yapılandırma yüklenemedi", "Could not load configuration")),
            }
            0
        }
        Some("run" | "--run") => cmd_run(&args[1..]),
        Some("-h" | "--help" | "help") => {
            help();
            0
        }
        Some(unknown) => {
            eprintln!("{}: {unknown}\n", crate::i18n::text("Bilinmeyen argüman", "Unknown argument"));
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
            eprintln!("{}: {e}", crate::i18n::text("Yapılandırma yüklenemedi", "Could not load configuration"));
            return 1;
        }
    };
    let scripts = discovery::scan(&cfg);
    if scripts.is_empty() {
        println!("{} — {}", crate::i18n::text("Script yok", "No scripts found"), cfg.scripts_dir.display());
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
            eprintln!("{}: {e}", crate::i18n::text("Yapılandırma yüklenemedi", "Could not load configuration"));
            return 1;
        }
    };
    let scripts = discovery::scan(&cfg);
    let Some(key) = args.first() else {
        eprintln!("{}: sr run <key> [args...]\n{}: sr --list",
            crate::i18n::text("Kullanım", "Usage"),
            crate::i18n::text("Anahtarları görmek için", "See available keys with"));
        return 2;
    };
    let Some(script) = discovery::find_key(&scripts, key) else {
        eprintln!("{}: {key} (--list)", crate::i18n::text("Script bulunamadı", "Script not found"));
        return 2;
    };
    if !script.available {
        eprintln!("{}", script.missing_reason.clone().unwrap_or_else(|| crate::i18n::text("kullanılamaz", "unavailable").into()));
        return 3;
    }
    let args_line = args[1..].join(" ");
    match runner::run_headless(script, &cfg, &args_line) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{}: {e:#}", crate::i18n::text("Hata", "Error"));
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
            eprintln!("{}: {e:#}", crate::i18n::text("Hata", "Error"));
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
        println!("{}\n", crate::i18n::text("sr ilk kurulum kontrolü", "sr first-run setup"));
    }
    let create_config = if config_missing {
        println!("{}:", crate::i18n::text("Yapılandırma dosyası bulunamadı", "Configuration file not found"));
        println!("  {}", config_path.display());
        confirm(crate::i18n::text("Örnek config.toml oluşturulsun mu? [E/h]: ", "Create a sample config.toml? [Y/n]: "))?
    } else {
        false
    };

    let cfg = config::AppConfig::load_with_config_creation(create_config)?;

    if is_first_start {
        println!("\n{}:", crate::i18n::text("Sistem gereksinimleri", "System requirements"));
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
                    let missing_text = crate::i18n::text("{tool} bulunamadı", "{tool} not found").replace("{tool}", executable);
                    println!("  ✗ {name:<8} {missing_text}");
                    missing.push(name);
                }
            }
        }

        if missing.is_empty() {
            println!("\n{}", crate::i18n::text("Gerekli yorumlayıcılar bulundu.", "All required interpreters were found."));
        } else {
            println!("\n{}: {}.", crate::i18n::text("Eksik", "Missing"), missing.join(", "));
            println!("{}", crate::i18n::text(
                "İlgili dili kullanmak için yükleyin veya config.toml içindeki [interpreters] bölümüne yolunu yazın.",
                "Install the missing interpreter or set its path in the [interpreters] section of config.toml."
            ));
            println!("{}", crate::i18n::text("sr bu araçları otomatik yüklemez; diğer diller kullanılabilir.", "sr does not install these tools; other languages remain available."));
            if !confirm(crate::i18n::text("Eksikler olmadan ana ekrana devam edilsin mi? [E/h]: ", "Continue to the main screen without them? [Y/n]: "))? {
                println!("{}", crate::i18n::text("Başlatma iptal edildi. Eksik yorumlayıcıları kurup tekrar deneyin.", "Startup cancelled. Install the missing interpreters and try again."));
                return Ok(None);
            }
        }
        println!("\n{}", crate::i18n::text("Ana ekrana geçmek için Enter'a basın.", "Press Enter to continue to the main screen."));
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
