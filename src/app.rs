//! Uygulama durumu ve tuş/IO etkileşimi.

use std::sync::mpsc::Sender;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::config::AppConfig;
use crate::discovery::{self, Lang, Script};
use crate::runner::{self, IoMsg, JobState, MAX_LINES};

/// Ana döngüye gelen olaylar (tuşlar + süreç çıktısı).
pub enum AppEvent {
    Key(KeyEvent),
    Paste(String),
    Io(IoMsg),
}

pub struct Palette {
    /// filtre sorgusu
    pub query: String,
    pub cursor: usize, // char index
    /// scripts dizinine indeksler; skora göre sıralı
    pub items: Vec<usize>,
    pub selected: usize,
    /// Some((script_idx, args metni, imleç)) => argü girişi aşamasındayız
    pub args_mode: Option<(usize, String, usize)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeAction {
    OpenList,
    Rerun,
    Help,
    Quit,
}

pub struct App {
    pub cfg: AppConfig,
    pub scripts: Vec<Script>,
    pub palette: Option<Palette>,
    pub job: Option<JobState>,
    pub last: Option<runner::JobSummary>,
    /// çıktıda alttan kaydırma sayısı (0 = en altta, takip açık)
    pub scroll_back: u16,
    /// Ana ekrandaki seçili eylem.
    pub home_selected: usize,
    pub help_open: bool,
    pub help_scroll: u16,
    pub quit: bool,
    pub toast: Option<(String, Instant)>,
    pub spin: usize,
    pub tx: Sender<AppEvent>,
}

const SPIN_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn lang_order(lang: Lang) -> usize {
    match lang {
        Lang::Python => 0,
        Lang::Node => 1,
        Lang::Rust => 2,
        Lang::Custom => 3,
    }
}

impl App {
    pub fn new(cfg: AppConfig, tx: Sender<AppEvent>) -> App {
        let mut app = App {
            scripts: Vec::new(),
            palette: None,
            job: None,
            last: None,
            scroll_back: 0,
            home_selected: 0,
            help_open: false,
            help_scroll: 0,
            quit: false,
            toast: None,
            spin: 0,
            cfg,
            tx,
        };
        app.refresh_scripts();
        app
    }

    pub fn refresh_scripts(&mut self) {
        self.scripts = discovery::scan(&self.cfg);
        if let Some(p) = &mut self.palette {
            p.args_mode = None;
            Self::filter(&self.scripts, &p.query, &mut p.items);
            p.selected = 0;
        }
    }

    fn filter(scripts: &[Script], query: &str, out: &mut Vec<usize>) {
        out.clear();
        if query.trim().is_empty() {
            out.extend(0..scripts.len());
            out.sort_by(|&a, &b| {
                lang_order(scripts[a].lang)
                    .cmp(&lang_order(scripts[b].lang))
                    .then_with(|| scripts[a].key.cmp(&scripts[b].key))
            });
            return;
        }
        let mut scored: Vec<(usize, i32, usize)> = Vec::new();
        for (i, s) in scripts.iter().enumerate() {
            let hay = format!("{} {} {} {}", s.key, s.name, s.lang.label(), s.description);
            if let Some(score) = crate::util::fuzzy_score(query.trim(), &hay) {
                scored.push((lang_order(s.lang), score, i));
            }
        }
        scored.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| b.1.cmp(&a.1))
                .then_with(|| scripts[a.2].key.cmp(&scripts[b.2].key))
        });
        out.extend(scored.into_iter().map(|(_, _, i)| i));
    }

    fn open_palette(&mut self) {
        self.refresh_scripts();
        let mut items = Vec::new();
        Self::filter(&self.scripts, "", &mut items);
        self.palette = Some(Palette {
            query: String::new(),
            cursor: 0,
            items,
            selected: 0,
            args_mode: None,
        });
    }

    fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
    }

    pub fn spinner(&self) -> &'static str {
        SPIN_FRAMES[self.spin % SPIN_FRAMES.len()]
    }

    pub fn home_actions(&self) -> Vec<HomeAction> {
        let mut actions = vec![HomeAction::OpenList];
        if self.last.is_some() {
            actions.push(HomeAction::Rerun);
        }
        actions.push(HomeAction::Help);
        actions.push(HomeAction::Quit);
        actions
    }

    fn activate_home_action(&mut self) {
        match self.home_actions().get(self.home_selected).copied() {
            Some(HomeAction::OpenList) => self.open_palette(),
            Some(HomeAction::Rerun) => self.rerun_last(),
            Some(HomeAction::Help) => {
                self.help_open = true;
                self.help_scroll = 0;
            }
            Some(HomeAction::Quit) => self.quit = true,
            None => self.home_selected = 0,
        }
    }

    pub fn app_event(&mut self, ev: AppEvent) {
        match ev {
            AppEvent::Key(k) => {
                if k.kind == KeyEventKind::Release {
                    return;
                }
                self.on_key(k);
            }
            AppEvent::Paste(text) => {
                let cleaned = text.replace(['\r', '\n'], " ");
                self.insert_at_focus(&cleaned);
            }
            AppEvent::Io(m) => self.on_io(m),
        }
    }

    fn insert_at_focus(&mut self, text: &str) {
        if let Some(pal) = &mut self.palette {
            if let Some((_, s, c)) = &mut pal.args_mode {
                input_insert(s, c, text);
            } else {
                input_insert(&mut pal.query, &mut pal.cursor, text);
                Self::filter(&self.scripts, &pal.query, &mut pal.items);
                pal.selected = 0;
            }
        }
    }

    fn on_io(&mut self, m: IoMsg) {
        let Some(job) = &mut self.job else { return };
        match m {
            IoMsg::Line { stream, text } => {
                if job.lines.len() < MAX_LINES {
                    job.lines.push((stream, text));
                }
            }
            IoMsg::Closed => {
                job.open_streams = job.open_streams.saturating_sub(1);
            }
        }
    }

    /// Ana döngünün her turunda çağrılır.
    pub fn tick(&mut self) {
        let was_done = self.job.as_ref().map_or(true, |j| j.outcome.is_some());
        if let Some(job) = &mut self.job {
            runner::tick(job, &self.tx);
        }
        if !was_done {
            if let Some(job) = &self.job {
                if job.outcome.is_some() {
                    self.last = Some(job.summary());
                }
            }
        }
        if self.job.as_ref().is_some_and(|j| j.is_running()) {
            self.spin = self.spin.wrapping_add(1);
        }
    }

    // ---------------- tuşlar ----------------

    fn on_key(&mut self, k: KeyEvent) {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        match k.code {
            // küresel: Ctrl+P palet, Ctrl+C durdur/çık, Ctrl+R tekrar
            KeyCode::Char('p' | 'P') if ctrl => {
                if self.palette.is_some() {
                    self.palette = None;
                } else {
                    self.open_palette();
                }
                return;
            }
            KeyCode::Char('c') if ctrl => {
                if self.job.as_ref().is_some_and(|j| j.is_running()) {
                    self.job.as_mut().unwrap().kill();
                    self.toast("isteğiniz süreç durduruldu");
                } else {
                    self.quit = true;
                }
                return;
            }
            KeyCode::Char('r') if ctrl => {
                self.rerun_last();
                return;
            }
            _ => {}
        }

        if self.palette.is_some() {
            let pal = self.palette.take().expect("palet durumu");
            self.on_palette_key(k, pal);
            return;
        }

        if self.help_open {
            self.on_help_key(k);
            return;
        }

        self.on_home_key(k);
    }

    fn on_help_key(&mut self, k: KeyEvent) {
        let max_scroll = crate::help::text().lines().count().saturating_sub(1) as u16;
        match k.code {
            KeyCode::Esc | KeyCode::Backspace => {
                self.help_open = false;
                self.help_scroll = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => self.help_scroll = self.help_scroll.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                self.help_scroll = self.help_scroll.saturating_add(1).min(max_scroll)
            }
            KeyCode::PageUp => self.help_scroll = self.help_scroll.saturating_sub(20),
            KeyCode::PageDown => self.help_scroll = self.help_scroll.saturating_add(20).min(max_scroll),
            KeyCode::Home => self.help_scroll = 0,
            KeyCode::End => self.help_scroll = max_scroll,
            _ => {}
        }
    }

    fn on_home_key(&mut self, k: KeyEvent) {
        if let Some(job) = &self.job {
            let total = job.lines.len() as i64;
            let back = self.scroll_back as i64;
            match k.code {
                KeyCode::Up => self.scroll_back = (back + 1).clamp(0, total) as u16,
                KeyCode::Down => self.scroll_back = (back - 1).max(0) as u16,
                KeyCode::PageUp => self.scroll_back = (back + 20).clamp(0, total) as u16,
                KeyCode::PageDown => self.scroll_back = (back - 20).max(0) as u16,
                KeyCode::Home => self.scroll_back = total as u16,
                KeyCode::End => self.scroll_back = 0,
                _ => {}
            }
            self.scroll_back = self.scroll_back.min(job.lines.len() as u16);
        }
        match k.code {
            KeyCode::Backspace if self.job.as_ref().is_some_and(|job| !job.is_running()) => {
                self.job = None;
                self.scroll_back = 0;
                self.home_selected = 0;
            }
            KeyCode::Up | KeyCode::Char('k') if self.job.is_none() => {
                let count = self.home_actions().len().max(1);
                self.home_selected = (self.home_selected + count - 1) % count;
            }
            KeyCode::Down | KeyCode::Char('j') if self.job.is_none() => {
                let count = self.home_actions().len().max(1);
                self.home_selected = (self.home_selected + 1) % count;
            }
            KeyCode::Enter if self.job.is_none() => self.activate_home_action(),
            KeyCode::Char('q') | KeyCode::Char('Q') => self.quit = true,
            KeyCode::Esc => {}
            _ => {}
        }
    }

    fn on_palette_key(&mut self, k: KeyEvent, mut pal: Palette) {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let mut keep = true;

        if let Some((idx, args, cursor)) = &mut pal.args_mode {
            match k.code {
                KeyCode::Enter | KeyCode::Tab => {
                    let args_text = args.clone();
                    let _ = cursor;
                    if self.run_index(*idx, &args_text) {
                        keep = false; // çalıştı; palet kapandı
                    }
                }
                KeyCode::Esc => pal.args_mode = None,
                KeyCode::Char(c) if !ctrl => input_insert(args, cursor, &c.to_string()),
                KeyCode::Char('u') if ctrl => {
                    args.clear();
                    *cursor = 0;
                }
                KeyCode::Char('w') if ctrl => input_del_word(args, cursor),
                KeyCode::Backspace => input_backspace(args, cursor),
                KeyCode::Delete => input_delete(args, cursor),
                KeyCode::Left => *cursor = cursor.saturating_sub(1),
                KeyCode::Right => *cursor = (*cursor + 1).min(args.chars().count()),
                KeyCode::Home => *cursor = 0,
                KeyCode::End => *cursor = args.chars().count(),
                KeyCode::Char('a') if ctrl => *cursor = 0,
                KeyCode::Char('e') if ctrl => *cursor = args.chars().count(),
                _ => {}
            }
            if keep {
                self.palette = Some(pal);
            }
            return;
        }

        // Liste aşaması
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => {
                let n = pal.items.len().max(1);
                pal.selected = (pal.selected + n - 1) % n;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let n = pal.items.len().max(1);
                pal.selected = (pal.selected + 1) % n;
            }
            KeyCode::Char(c) if !ctrl => {
                input_insert(&mut pal.query, &mut pal.cursor, &c.to_string());
                Self::filter(&self.scripts, &pal.query, &mut pal.items);
                pal.selected = 0;
            }
            KeyCode::Char('u') if ctrl => {
                pal.query.clear();
                pal.cursor = 0;
                Self::filter(&self.scripts, &pal.query, &mut pal.items);
                pal.selected = 0;
            }
            KeyCode::Char('w') if ctrl => {
                input_del_word(&mut pal.query, &mut pal.cursor);
                Self::filter(&self.scripts, &pal.query, &mut pal.items);
                pal.selected = 0;
            }
            KeyCode::Backspace => {
                input_backspace(&mut pal.query, &mut pal.cursor);
                Self::filter(&self.scripts, &pal.query, &mut pal.items);
                pal.selected = 0;
            }
            KeyCode::Delete => input_delete(&mut pal.query, &mut pal.cursor),
            KeyCode::Left => pal.cursor = pal.cursor.saturating_sub(1),
            KeyCode::Right => pal.cursor = (pal.cursor + 1).min(pal.query.chars().count()),
            KeyCode::Home => pal.cursor = 0,
            KeyCode::End => pal.cursor = pal.query.chars().count(),
            KeyCode::Enter => {
                match pal.items.get(pal.selected).copied() {
                    Some(idx) => {
                        let script = &self.scripts[idx];
                        if script.available {
                            let cur = script.default_args.chars().count();
                            pal.args_mode = Some((idx, script.default_args.clone(), cur));
                        } else {
                            let msg = script.missing_reason.clone().unwrap_or_else(|| "kullanılamaz".into());
                            self.toast(msg);
                        }
                    }
                    None => self.toast("eşleşen script yok"),
                }
            }
            KeyCode::Tab => {
                if let Some(idx) = pal.items.get(pal.selected).copied() {
                    let started = self.run_index(idx, "");
                    if started {
                        keep = false;
                    }
                } else {
                    self.toast("eşleşen script yok");
                }
            }
            KeyCode::Esc => keep = false,
            _ => {}
        }

        if keep {
            self.palette = Some(pal);
        }
    }

    /// idx'teki scripti verilen argümanla çalıştırır. true = başladı.
    fn run_index(&mut self, idx: usize, args: &str) -> bool {
        if self.job.as_ref().is_some_and(|j| j.is_running()) {
            self.toast("bir iş hâlâ çalışıyor — Ctrl+C ile durdurabilirsin");
            return false;
        }
        let Some(s) = self.scripts.get(idx) else {
            self.toast("script bulunamadı");
            return false;
        };
        if !s.available {
            self.toast(s.missing_reason.clone().unwrap_or_else(|| "kullanılamaz".into()));
            return false;
        }
        let s = s.clone();
        match runner::start_job(&s, &self.cfg, args, &self.tx) {
            Ok(job) => {
                self.job = Some(job);
                self.palette = None;
                self.scroll_back = 0;
                true
            }
            Err(e) => {
                self.toast(format!("başlatılamadı: {e}"));
                false
            }
        }
    }

    fn rerun_last(&mut self) {
        if self.job.as_ref().is_some_and(|j| j.is_running()) {
            self.toast("çalışan iş var — önce Ctrl+C ile durdur");
            return;
        }
        let Some(prev) = self.last.clone() else {
            self.toast("tekrar için önce bir iş çalıştır");
            return;
        };
        self.refresh_scripts();
        let idx = self.scripts.iter().position(|s| s.key.eq_ignore_ascii_case(&prev.script_key));
        let Some(idx) = idx else {
            self.toast("script artık listede değil (silinmiş olabilir)");
            return;
        };
        self.run_index(idx, &prev.args);
    }
}

// --------- küçük metin düzenleme yardımcıları (char index imleçli) ---------

fn char_count(s: &str) -> usize {
    s.chars().count()
}

fn clamp_cursor(s: &str, i: usize) -> usize {
    i.min(char_count(s))
}

pub fn input_insert(s: &mut String, cursor: &mut usize, text: &str) {
    let byte = char_to_byte(s, *cursor);
    s.insert_str(byte, text);
    *cursor = clamp_cursor(s, *cursor + char_count(text));
}

pub fn input_backspace(s: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    let byte_end = char_to_byte(s, *cursor);
    let byte_start = char_to_byte(s, *cursor - 1);
    s.replace_range(byte_start..byte_end, "");
    *cursor -= 1;
}

pub fn input_delete(s: &mut String, cursor: &mut usize) {
    let n = char_count(s);
    if *cursor >= n {
        return;
    }
    let byte_start = char_to_byte(s, *cursor);
    let byte_end = char_to_byte(s, *cursor + 1);
    s.replace_range(byte_start..byte_end, "");
}

/// İmleçten geriye doğru kelime siler (Ctrl+W).
pub fn input_del_word(s: &mut String, cursor: &mut usize) {
    let chars: Vec<char> = s.chars().collect();
    let mut i = *cursor;
    while i > 0 && chars[i - 1].is_whitespace() {
        i -= 1;
    }
    while i > 0 && !chars[i - 1].is_whitespace() {
        i -= 1;
    }
    s.replace_range(char_to_byte(s, i)..char_to_byte(s, *cursor), "");
    *cursor = i;
}

/// string'te verilen char index'in byte offset'i
fn char_to_byte(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map_or(s.len(), |(b, _)| b)
}

// ---------------------------------------------------------------- testler

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    fn scratch_cfg(tag: &str) -> AppConfig {
        let tmp = std::env::temp_dir().join(format!("sr_app_{tag}_{}", std::process::id()));
        std::fs::create_dir_all(tmp.join("scripts")).unwrap();
        AppConfig {
            root: tmp.clone(),
            scripts_dir: tmp.join("scripts"),
            cache_dir: tmp.join("cache"),
            python: "python".into(),
            node: "node".into(),
            rustc: "rustc".into(),
        }
    }

    /// TUI ana döngüsünü terminal olmadan taklit eder: kanal olayları işlenir,
    /// tick çalışır; tüm print satırlarının job'a ulaştığı doğrulanır.
    #[test]
    fn python_prints_reach_job_state() {
        // Python bulunmayan makinelerde bu entegrasyon testi uygulanamaz.
        if crate::util::find_in_path("python").is_none() {
            return;
        }
        let cfg = scratch_cfg("py");
        std::fs::write(
            cfg.scripts_dir.join("test_print.py"),
            "print('satir 1')\nprint('satir 2')\nprint('satir 3')\n",
        )
        .unwrap();

        let scratch_root = cfg.root.clone();
        let (tx, rx) = mpsc::channel::<AppEvent>();
        let mut app = App::new(cfg, tx);
        let idx = app
            .scripts
            .iter()
            .position(|s| s.key == "test_print.py")
            .expect("script bulunamadı");
        assert!(app.run_index(idx, ""), "iş başlatılamadı");
        assert!(app.job.is_some(), "job Some olmalı");

        // tui_loop'unkiyle aynı drain şablonu
        for _ in 0..600 {
            for _ in 0..32 {
                match rx.recv_timeout(Duration::from_millis(25)) {
                    Ok(m) => app.app_event(m),
                    Err(_) => break,
                }
            }
            app.tick();
            if app.job.as_ref().is_some_and(|j| !j.is_running()) {
                // son kapanış olaylarını da boşalt
                for _ in 0..32 {
                    match rx.recv_timeout(Duration::from_millis(5)) {
                        Ok(m) => app.app_event(m),
                        Err(_) => break,
                    }
                }
                break;
            }
        }

        let job = app.job.as_ref().unwrap();
        let stdout_text: Vec<String> = job
            .lines
            .iter()
            .filter(|(s, _)| *s == 0)
            .map(|(_, t)| t.clone())
            .collect();
        assert_eq!(stdout_text.len(), 3, "satır kaybı: {stdout_text:?}");
        assert!(stdout_text.iter().any(|t| t.contains("satir 1")), "satir 1 yok: {stdout_text:?}");
        assert!(stdout_text.iter().any(|t| t.contains("satir 2")));
        assert!(stdout_text.iter().any(|t| t.contains("satir 3")));
        assert!(matches!(job.outcome, Some(runner::Outcome::Ok)));
        std::fs::remove_dir_all(scratch_root).ok();
    }

    #[test]
    fn ctrl_p_paletteyi_acar_ve_dilleri_siralar() {
        let cfg = scratch_cfg("palette");
        for (folder, file, contents) in [
            ("python", "hello.py", "print(1)"),
            ("nodejs", "hello.js", "console.log(1)"),
            ("rust", "hello.rs", "fn main() {}"),
        ] {
            let dir = cfg.scripts_dir.join(folder);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(file), contents).unwrap();
        }
        let scratch_root = cfg.root.clone();

        let (tx, _rx) = mpsc::channel::<AppEvent>();
        let mut app = App::new(cfg, tx);
        app.app_event(AppEvent::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('p'),
            crossterm::event::KeyModifiers::CONTROL,
        )));

        let palette = app.palette.as_ref().expect("Ctrl+P paleti açmadı");
        let langs: Vec<Lang> = palette
            .items
            .iter()
            .map(|&i| app.scripts[i].lang)
            .collect();
        assert_eq!(langs, vec![Lang::Python, Lang::Node, Lang::Rust]);

        app.app_event(AppEvent::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('p'),
            crossterm::event::KeyModifiers::CONTROL,
        )));
        assert!(app.palette.is_none(), "ikinci Ctrl+P paleti kapatmalı");
        std::fs::remove_dir_all(scratch_root).ok();
    }

    #[test]
    fn backspace_biten_is_gorunumunden_ana_ekrana_doner() {
        let cfg = scratch_cfg("backspace");
        let (tx, _rx) = mpsc::channel::<AppEvent>();
        let mut app = App::new(cfg, tx);
        let now = Instant::now();
        app.job = Some(runner::JobState {
            script_key: "hello.py".into(),
            title: "hello".into(),
            args_line: String::new(),
            started: now,
            cwd: None,
            pending: std::collections::VecDeque::new(),
            phase_label: "python".into(),
            phase_cmd: String::new(),
            child: None,
            lines: vec![(0, "tamamlandı".into())],
            open_streams: 0,
            killed: false,
            last_exit: Some(0),
            exit_since: None,
            outcome: Some(runner::Outcome::Ok),
            ended: Some(now),
        });
        app.scroll_back = 1;

        app.app_event(AppEvent::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Backspace,
            crossterm::event::KeyModifiers::NONE,
        )));

        assert!(app.job.is_none(), "biten iş görünümden kaldırılmalı");
        assert_eq!(app.scroll_back, 0);
    }

    #[test]
    fn ana_menu_liste_yardim_ve_cikis_eylemlerini_acar() {
        let cfg = scratch_cfg("home_menu");
        let (tx, _rx) = mpsc::channel::<AppEvent>();
        let mut app = App::new(cfg, tx);

        app.app_event(AppEvent::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        )));
        assert!(app.palette.is_some(), "ilk menü seçeneği script listesini açmalı");

        // Paleti kapatıp ana menüden Yardım'ı aç.
        app.app_event(AppEvent::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        )));
        app.app_event(AppEvent::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Down,
            crossterm::event::KeyModifiers::NONE,
        )));
        app.app_event(AppEvent::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        )));
        assert!(app.help_open, "ana menüden Yardım seçilebilmeli");

        app.app_event(AppEvent::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        )));
        app.app_event(AppEvent::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Down,
            crossterm::event::KeyModifiers::NONE,
        )));
        app.app_event(AppEvent::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        )));
        assert!(app.quit, "ana menüden Çıkış seçilebilmeli");
    }
}
