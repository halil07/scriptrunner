//! Çalıştırma motoru: aşamaları (stage), süreçleri, çıktı okuma iş parçacıklarını yönetir.
//!
//! - Python/Node: tek aşama (yorumlayıcı + dosya + argümanlar)
//! - Rust: iki aşama (rustc derleme -> çalıştırma); derleme cache'te tutulur,
//!   kaynak dosyadan yeniyse derleme atlanır.
//! - Manifest'li: şablondan üretilir ("{{args}}" / "{{dir}}"), tek aşama.

use std::collections::VecDeque;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::Instant;

use anyhow::{anyhow, bail, Result};

use crate::app::AppEvent;
use crate::config::AppConfig;
use crate::discovery::{Lang, Script};
use crate::util;

pub const MAX_LINES: usize = 20_000;
const MAX_INNER_LINE: usize = 20_000;

#[derive(Debug, Clone)]
pub struct Stage {
    pub exe: String,
    pub args: Vec<String>,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Ok,
    Failed(i32),
    Killed,
}

impl Outcome {
    pub fn icon(self) -> &'static str {
        match self {
            Outcome::Ok => "✔",
            Outcome::Killed => "⏹",
            Outcome::Failed(_) => "✗",
        }
    }
    pub fn text(self) -> String {
        match self {
            Outcome::Ok => "tamam".into(),
            Outcome::Killed => "durduruldu".into(),
            Outcome::Failed(c) => format!("hata (exit {c})"),
        }
    }
}

/// Arka plan okuyucu iş parçacıkları UI'a bunları yollar.
pub enum IoMsg {
    Line { stream: u8, text: String }, // stream: 0=stdout, 1=stderr
    Closed,
}

#[derive(Debug, Clone)]
pub struct JobSummary {
    pub script_key: String,
    pub title: String,
    pub args: String,
    pub outcome: Option<Outcome>,
    pub duration: Option<std::time::Duration>,
}

pub struct JobState {
    pub script_key: String,
    pub title: String,
    pub args_line: String,
    pub started: Instant,
    pub cwd: Option<PathBuf>,
    pub pending: VecDeque<Stage>,
    pub phase_label: String,
    pub phase_cmd: String,
    pub child: Option<Child>,
    pub lines: Vec<(u8, String)>,
    pub open_streams: usize,
    pub killed: bool,
    pub last_exit: Option<i32>,
    pub exit_since: Option<Instant>,
    pub outcome: Option<Outcome>,
    pub ended: Option<Instant>,
}

impl JobState {
    pub fn is_running(&self) -> bool {
        self.outcome.is_none()
    }

    pub fn summary(&self) -> JobSummary {
        JobSummary {
            script_key: self.script_key.clone(),
            title: self.title.clone(),
            args: self.args_line.clone(),
            outcome: self.outcome,
            duration: Some(if let Some(e) = self.ended { e - self.started } else { self.started.elapsed() }),
        }
    }

    /// Süreci sonlandırır (Ctrl+C).
    pub fn kill(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            self.killed = true;
        }
    }
}

/// Stage listesi + çalışma dizini üretir.
pub fn build_stages(script: &Script, cfg: &AppConfig, args_line: &str) -> Result<(VecDeque<Stage>, PathBuf)> {
    let user_args = util::split_args(args_line);

    let base_dir = match script.lang {
        Lang::Custom => script.path.clone(), // klasör
        // Tek dosyalık scriptler kendi klasöründe çalışır (scripts kökündeki
        // eski davranışla aynı; alt klasördekiler göreli dosyalarına erişir).
        _ => script
            .path
            .parent()
            .filter(|p| p.is_dir())
            .unwrap_or(&cfg.scripts_dir)
            .to_path_buf(),
    };
    let cwd = match &script.cwd {
        Some(rel) => {
            let p = Path::new(rel);
            if p.is_absolute() { p.to_path_buf() } else { base_dir.join(p) }
        }
        None => base_dir,
    };

    let stages: VecDeque<Stage> = match script.lang {
        Lang::Python => VecDeque::from(vec![Stage {
            exe: cfg.python.clone(),
            args: {
                let mut v = vec!["-u".into(), script.path.to_string_lossy().into_owned()];
                v.extend(user_args);
                v
            },
            label: "python".into(),
        }]),
        Lang::Node => VecDeque::from(vec![Stage {
            exe: cfg.node.clone(),
            args: {
                let mut v = vec![script.path.to_string_lossy().into_owned()];
                v.extend(user_args);
                v
            },
            label: "node".into(),
        }]),
        Lang::Rust => {
            let exe_out = cfg
                .cache_dir
                .join("rust")
                .join(format!("{}.exe", util::cache_file_name(&script.key)));
            let stale = |out: &Path| match (std::fs::metadata(out), std::fs::metadata(&script.path)) {
                (Ok(o), Ok(s)) => match (o.modified(), s.modified()) {
                    (Ok(om), Ok(sm)) => om < sm,
                    _ => true,
                },
                _ => true,
            };

            let run = Stage {
                exe: exe_out.to_string_lossy().into_owned(),
                args: user_args,
                label: script.key.clone(),
            };
            if exe_out.is_file() && !stale(&exe_out) {
                return Ok((VecDeque::from(vec![run]), cwd));            }
            let build = Stage {
                exe: cfg.rustc.clone(),
                args: vec![
                    "--edition=2021".into(),
                    script.path.to_string_lossy().into_owned(),
                    "-o".into(),
                    exe_out.to_string_lossy().into_owned(),
                ],
                label: "rustc derleniyor".into(),
            };
            // Windows'ta WDAC, imzasız süreç zincirinin yazdığı exe'yi
            // engelleyebiliyor; derlemeyi cmd üzerinden sarmak güven yolunu açıyor.
            #[cfg(windows)]
            let build = {
                let mut args = vec!["/c".to_string(), build.exe];
                args.extend(build.args);
                Stage { exe: "cmd".into(), args, label: build.label }
            };
            VecDeque::from(vec![build, run])
        }
        Lang::Custom => {
            let Some(tpl) = &script.command else {
                bail!("script.toml içinde 'command' yok");
            };
            let words = util::split_command(tpl).ok_or_else(|| anyhow!("komut ayrıştırılamadı: {tpl}"))?;
            if words.is_empty() {
                bail!("script.toml 'command' boş");
            }
            let mut argv: Vec<String> = Vec::new();
            let mut has_args_token = false;
            for w in &words {
                if w == "{{args}}" {
                    has_args_token = true;
                    argv.extend(user_args.iter().cloned());
                } else if w.contains("{{args}}") {
                    has_args_token = true;
                    argv.push(w.replace("{{args}}", args_line.trim()));
                } else if w.contains("{{dir}}") {
                    argv.push(w.replace("{{dir}}", &script.path.to_string_lossy()));
                } else {
                    argv.push(w.clone());
                }
            }
            if !has_args_token && !user_args.is_empty() {
                argv.extend(user_args);
            }
            let exe = argv.remove(0);
            let label = exe.split(['\\', '/']).last().unwrap_or(&exe).to_string();
            VecDeque::from(vec![Stage { exe, args: argv, label }])
        }
    };

    Ok((stages, cwd))
}

/// Python çocuklarında UTF-8'i zorunlu kılar (Windows'ta cp aktif olunca patlar).
fn apply_lang_env(cmd: &mut Command, exe: &str) {
    let low = exe.to_ascii_lowercase();
    if low.contains("python") {
        cmd.env("PYTHONIOENCODING", "utf-8");
        cmd.env("PYTHONUTF8", "1");
    }
}

/// Yeni bir iş kurar ve ilk aşamayı başlatır.
pub fn start_job(script: &Script, cfg: &AppConfig, args_line: &str, tx: &Sender<AppEvent>) -> Result<JobState> {
    let (stages, cwd) = build_stages(script, cfg, args_line)?;
    let mut job = JobState {
        script_key: script.key.clone(),
        title: script.name.clone(),
        args_line: args_line.trim().to_string(),
        started: Instant::now(),
        cwd: Some(cwd),
        pending: stages,
        phase_label: String::new(),
        phase_cmd: String::new(),
        child: None,
        lines: Vec::new(),
        open_streams: 0,
        killed: false,
        last_exit: None,
        exit_since: None,
        outcome: None,
        ended: None,
    };
    spawn_next_stage(&mut job, tx)?;
    Ok(job)
}

/// Bekleyen aşama varsa yeni süreci başlatır, yoksa sonucu kesinleştirir.
pub fn spawn_next_stage(job: &mut JobState, tx: &Sender<AppEvent>) -> Result<()> {
    if job.child.is_some() || job.outcome.is_some() {
        return Ok(());
    }
    if let Some(stage) = job.pending.pop_front() {
        job.phase_label = stage.label.clone();
        job.phase_cmd = {
            let mut s = String::from(&stage.exe);
            for a in &stage.args {
                s.push(' ');
                s.push_str(a);
            }
            s
        };

        let mut cmd = Command::new(&stage.exe);
        cmd.args(&stage.args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());
        if let Some(cwd) = &job.cwd {
            cmd.current_dir(cwd);
        }
        apply_lang_env(&mut cmd, &stage.exe);

        match cmd.spawn() {
            Ok(mut child) => {
                let out = child.stdout.take().expect("piped stdout");
                let err = child.stderr.take().expect("piped stderr");
                job.open_streams = 2;
                spawn_reader(Box::new(out), 0, tx.clone());
                spawn_reader(Box::new(err), 1, tx.clone());
                job.child = Some(child);
            }
            Err(e) => {
                let msg = format!("⛔ başlatılamadı: '{}' — {e}\n  ipucu: config.toml [interpreters] bölümüne tam yol yazabilirsiniz", stage.exe);
                push_line(job, 1, &msg);
                job.outcome = Some(Outcome::Failed(-2));
                job.ended = Some(Instant::now());
            }
        }
    } else if job.open_streams == 0 {
        finalize(job);
    }
    Ok(())
}

/// Süreç durumunu denetler ve aşama geçişlerini yürütür.
pub fn tick(job: &mut JobState, tx: &Sender<AppEvent>) {
    if let Some(child) = job.child.as_mut() {
        match child.try_wait() {
            Ok(Some(status)) => {
                job.child = None;
                job.last_exit = Some(status.code().unwrap_or(-1));
                job.exit_since = Some(Instant::now());
            }
            Ok(None) => {}
            Err(_) => {
                job.child = None;
                job.last_exit = None;
                job.exit_since = Some(Instant::now());
            }
        }
    }

    if job.child.is_none() && job.outcome.is_none() {
        if job.open_streams == 0 {
            let failed_last = job.last_exit.is_some_and(|c| c != 0);
            if !job.pending.is_empty() && !failed_last && !job.killed {
                let _ = spawn_next_stage(job, tx);
            } else {
                // Derleme başarısız veya kalan aşama yok
                if job.killed {
                    job.outcome = Some(Outcome::Killed);
                } else if let Some(c) = job.last_exit {
                    job.outcome = Some(if c == 0 { Outcome::Ok } else { Outcome::Failed(c) });
                } else {
                    job.outcome = Some(Outcome::Failed(-1));
                }
                job.ended = Some(Instant::now());
            }
        } else if job.killed && job.exit_since.is_some_and(|t| t.elapsed().as_millis() > 2000) {
            // Süreç sonlandı ama boru kapanmadı: zorla bitir
            job.open_streams = 0;
            job.outcome = Some(Outcome::Killed);
            job.ended = Some(Instant::now());
        }
    }
}

fn finalize(job: &mut JobState) {
    if job.outcome.is_some() {
        return;
    }
    job.outcome = Some({
        if job.killed {
            Outcome::Killed
        } else if let Some(c) = job.last_exit {
            if c == 0 { Outcome::Ok } else { Outcome::Failed(c) }
        } else {
            Outcome::Failed(-1)
        }
    });
    job.ended = Some(Instant::now());
}

pub fn push_line(job: &mut JobState, stream: u8, text: &str) {
    if job.lines.len() >= MAX_LINES {
        return;
    }
    // Çok uzun tek satırı kırp
    let mut owned = text.to_string();
    if owned.chars().count() > MAX_INNER_LINE {
        owned = owned.chars().take(MAX_INNER_LINE).collect::<String>() + " …(kırpıldı)";
    }
    job.lines.push((stream, owned));
}

/// Bir borudan okur, satırlara bölerek UI kanalına yollar.
fn spawn_reader(mut pipe: Box<dyn Read + Send>, stream: u8, tx: Sender<AppEvent>) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        let mut carry: Vec<u8> = Vec::with_capacity(4096);
        loop {
            match pipe.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    carry.extend_from_slice(&buf[..n]);
                    while let Some(pos) = carry.iter().position(|&b| b == b'\n') {
                        let line: Vec<u8> = carry.drain(..=pos).collect();
                        let text = sanitize(&line[..line.len() - 1]);
                        let _ = tx.send(AppEvent::Io(IoMsg::Line { stream, text }));
                    }
                    // Tek satır takılmışsa aşırı büyümesini engelle
                    if carry.len() > 1_000_000 {
                        let text = sanitize(&carry);
                        carry.clear();
                        let _ = tx.send(AppEvent::Io(IoMsg::Line { stream, text }));
                    }
                }
                Err(_) => break,
            }
        }
        if !carry.is_empty() {
            let text = sanitize(&carry);
            let _ = tx.send(AppEvent::Io(IoMsg::Line { stream, text }));
        }
        let _ = tx.send(AppEvent::Io(IoMsg::Closed));
    });
}

/// Bir satırın ham baytlarını metne çevirir (satır sonu \n zaten ayrık).
/// - Windows CRLF'i normalize eder: satır sondaki tek \r yutulur.
///   (Python, pipeli stdout'a \r\n yazar; \r'ı satır başına kadar akıtan
///   eski davranış her normal satırı tamamen boşaltıyordu — print kaybı.)
/// - \r ile çizilen ilerleme çubuklarının son karesi korunur
///   (ör. "25%\r50%" → "50%").
/// - ANSI kaçış dizileri temizlenir.
fn sanitize(raw: &[u8]) -> String {
    let mut s = String::from_utf8_lossy(raw).into_owned();
    if s.ends_with('\r') {
        s.pop();
    }
    if let Some(pos) = s.rfind('\r') {
        s.drain(..=pos);
    }
    util::strip_ansi(&s)
}

/// CLI (--run) için senkron, TUI'sız çalıştırma.
pub fn run_headless(script: &Script, cfg: &AppConfig, args_line: &str) -> Result<i32> {
    let (mut stages, cwd) = build_stages(script, cfg, args_line)?;
    let mut last_code: i32 = 0;

    while let Some(stage) = stages.pop_front() {
        eprintln!("▶ {}", stage.label);
        let mut cmd = Command::new(&stage.exe);
        cmd.args(&stage.args);
        cmd.current_dir(&cwd);
        apply_lang_env(&mut cmd, &stage.exe);
        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .spawn()
            .map_err(|e| anyhow!("başlatılamadı: '{}' — {e}", stage.exe))?;
        let out = child.stdout.take().unwrap();
        let err = child.stderr.take().unwrap();
        let pipes: Vec<Box<dyn std::io::Read + Send>> = vec![Box::new(out), Box::new(err)];
        let mut threads = Vec::new();
        for mut pipe in pipes {
            threads.push(std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                loop {
                    match pipe.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            // doğrudan gerçek stdout/stderr'a yaz (o anki ordering yaklaşık)
                            use std::io::Write;
                            let mut so = std::io::stdout();
                            let _ = so.write_all(&buf[..n]);
                            let _ = so.flush();
                        }
                        Err(_) => break,
                    }
                }
            }));
        }
        let status = child.wait()?;
        for t in threads {
            let _ = t.join();
        }
        last_code = status.code().unwrap_or(-1);
        if last_code != 0 {
            return Ok(last_code);
        }
    }
    Ok(last_code)
}

#[cfg(test)]
mod sanitize_tests {
    use super::sanitize;

    #[test]
    fn crlf_satırının_metni_korunur() {
        // Python pipeli stdout'a \r\n yazar; \n okuyucuda koparıldıktan sonra
        // buraya "\r" ile gelir. Eski davranış satırı tamamen boşaltıyordu.
        assert_eq!(sanitize("satir 1\r".as_bytes()), "satir 1");
        assert_eq!(sanitize("✔ bitti (exit 0)\r".as_bytes()), "✔ bitti (exit 0)");
    }

    #[test]
    fn crlf_olmayan_satır_değişmez() {
        // okuyucu \n'i koparıp buraya gönderir; girdi asla \n ile bitmez
        assert_eq!(sanitize("hello".as_bytes()), "hello");
        assert_eq!(sanitize("".as_bytes()), "");
        assert_eq!(sanitize("tron veri".as_bytes()), "tron veri");
    }

    #[test]
    fn ilerleme_çubuğu_son_kareyi_tutar() {
        assert_eq!(sanitize("25%\r50%\r".as_bytes()), "50%");
        assert_eq!(sanitize("indiriliyor 25%\r".as_bytes()), "indiriliyor 25%");
    }

    #[test]
    fn ansi_temizlenir() {
        assert_eq!(sanitize("\x1b[31mkırmızı\x1b[0m\r".as_bytes()), "kırmızı");
    }
}
