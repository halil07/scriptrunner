//! OpenCode tarzı terminal UI (ratatui).

use std::time::Duration;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::{App, HomeAction};
use crate::config;
use crate::discovery::Lang;
use crate::runner::{JobState, Outcome};
use crate::util;

const ACCENT: Color = Color::Rgb(255, 150, 30);
const MUTED: Color = Color::Rgb(125, 125, 125);
const OK: Color = Color::Rgb(88, 200, 120);
const ERR: Color = Color::Rgb(240, 92, 88);
const SEL_BG: Color = Color::Rgb(62, 42, 16);

fn lang_color(l: Lang) -> Color {
    match l {
        Lang::Python => Color::Rgb(250, 210, 80),
        Lang::Node => Color::Rgb(74, 184, 112),
        Lang::Rust => Color::Rgb(255, 120, 60),
        Lang::Custom => Color::Rgb(96, 190, 255),
    }
}

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(area);

    draw_header(f, app, chunks[0]);
    draw_rule(f, chunks[1]);
    draw_body(f, app, chunks[2]);
    draw_status(f, app, chunks[3]);
    if app.palette.is_some() {
        draw_palette(f, app, chunks[2]);
    }
}

// ---------------------------------------------------------------- header/rule

fn draw_header(f: &mut Frame, app: &App, r: Rect) {
    let dir = config::shorten(&app.cfg.scripts_dir, 42);
    let line = Line::from(vec![
        Span::styled("◼ ", Style::new().fg(ACCENT).bold()),
        Span::styled("scriptrunner", Style::new().fg(Color::White).bold()),
        Span::styled(format!("  ·  scriptler: {}", app.scripts.len()), Style::new().fg(MUTED)),
        Span::styled("  ·  ", Style::new().fg(MUTED)),
        Span::styled(dir, Style::new().fg(MUTED)),
    ]);
    f.render_widget(Paragraph::new(line).style(Style::new().bg(Color::Black)), r);
}

fn draw_rule(f: &mut Frame, r: Rect) {
    f.render_widget(
        Paragraph::new(Line::from(Span::styled("─".repeat(r.width as usize), Style::new().fg(Color::Rgb(50, 50, 50))))),
        r,
    );
}

// ---------------------------------------------------------------- body (log)

fn draw_body(f: &mut Frame, app: &App, r: Rect) {
    match &app.job {
        Some(job) => draw_job(f, app, job, r),
        None => draw_welcome(f, app, r),
    }
}

fn draw_welcome(f: &mut Frame, app: &App, r: Rect) {
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled("  Ana menüden bir işlem seç:", Style::new().fg(Color::White).bold())),
        Line::from(""),
        Line::from(Span::styled(format!("  @ {:?}", app.cfg.scripts_dir), Style::new().fg(ACCENT))),
        Line::from(Span::styled("  Scriptleri bu klasöre bırak:", Style::new().fg(MUTED))),
        Line::from(Span::styled("  · python  : python/ornek.py", Style::new().fg(lang_color(Lang::Python)))),
        Line::from(Span::styled("  · node    : nodejs/ornek.js", Style::new().fg(lang_color(Lang::Node)))),
        Line::from(Span::styled("  · rust    : rust/ornek.rs  (tek dosya; rustc ile derlenir)", Style::new().fg(lang_color(Lang::Rust)))),
        Line::from(Span::styled("  · özel    : Klasör/script.toml  (command şablonu)", Style::new().fg(lang_color(Lang::Custom)))),
        Line::from(""),
        Line::from(Span::styled("  ↑↓ seç · Enter uygula", Style::new().fg(MUTED))),
    ];
    if app.scripts.is_empty() {
        lines.insert(4, Line::from(Span::styled("  (henüz script yok — `scriptrunner --open` ile klasörü açıp script bırak)", Style::new().fg(Color::Yellow))));
    }

    lines.push(Line::from(""));
    for (index, action) in app.home_actions().iter().enumerate() {
        let selected = index == app.home_selected;
        let (label, shortcut) = match action {
            HomeAction::OpenList => ("Listeyi aç", "Enter / Ctrl+P"),
            HomeAction::Rerun => ("Son işi tekrar çalıştır", "Enter / Ctrl+R"),
            HomeAction::Quit => ("Çıkış", "Enter / q / Ctrl+C"),
        };
        let marker_style = if selected { Style::new().fg(ACCENT).bold() } else { Style::new().fg(MUTED) };
        let label_style = if selected {
            Style::new().fg(Color::White).bg(SEL_BG).bold()
        } else {
            Style::new().fg(Color::White)
        };
        let shortcut_style = if selected {
            Style::new().fg(ACCENT).bg(SEL_BG)
        } else {
            Style::new().fg(MUTED)
        };
        lines.push(Line::from(vec![
            Span::styled(if selected { "  ▸ " } else { "    " }, marker_style),
            Span::styled(label, label_style),
            Span::styled(format!("   {shortcut}"), shortcut_style),
        ]));
    }
    f.render_widget(Paragraph::new(lines), r);
}

fn draw_job(f: &mut Frame, app: &App, job: &JobState, r: Rect) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::new().fg(Color::Rgb(50, 50, 50)))
        .style(Style::new().bg(Color::Black));
    let inner = block.inner(r);
    f.render_widget(block, r);

    if inner.width < 4 || inner.height == 0 {
        return;
    }

    let lines: Vec<Line> = job
        .lines
        .iter()
        .map(|(stream, text)| {
            let color = match stream {
                0 => Color::White,
                _ => ERR,
            };
            Line::from(Span::styled(format!(" {}", text), Style::new().fg(color)))
        })
        .collect();

    let total = lines.len() as u16;
    let h = inner.height;
    let back = app.scroll_back.min(total);
    let offset = total.saturating_sub(h).saturating_sub(back);

    let para = Paragraph::new(lines)
        .style(Style::new().bg(Color::Black))
        .scroll((offset, 0));
    f.render_widget(para, inner);

    // sağ üst köşe: satır sayısı/konum
    if app.job.as_ref().map_or(false, |j| j.is_running()) {
        let s = format!(" {} {} ", app.spinner(), job.phase_label);
        let x = r.x + r.width.saturating_sub(s.chars().count() as u16 + 2);
        f.render_widget(Paragraph::new(Line::from(Span::styled(s, Style::new().fg(ACCENT)))), Rect { x, y: r.y, width: r.width.saturating_sub(2), height: 1 });
    } else if let Some(o) = job.outcome {
        let s = format!(" {} {} ", o.icon(), o.text());
        let color = match o {
            Outcome::Ok => OK,
            Outcome::Failed(_) => ERR,
            Outcome::Killed => Color::Rgb(255, 190, 60),
        };
        let x = r.x + r.width.saturating_sub(s.chars().count() as u16 + 2);
        f.render_widget(Paragraph::new(Line::from(Span::styled(s, Style::new().fg(color).bold()))), Rect { x, y: r.y, width: r.width.saturating_sub(2), height: 1 });
    }
}

// ---------------------------------------------------------------- status

fn draw_status(f: &mut Frame, app: &App, r: Rect) {
    let left: Vec<Span> = if let Some((msg, t)) = &app.toast {
        if t.elapsed() < Duration::from_secs(4) {
            vec![Span::styled(" ! ", Style::new().fg(Color::Black).bg(Color::Rgb(255, 190, 60)).bold()),
                 Span::raw(" "),
                 Span::styled(msg.clone(), Style::new().fg(Color::Rgb(255, 200, 90)))]
        } else { status_left(app) }
    } else {
        status_left(app)
    };

    let hint = if app.job.as_ref().is_some_and(|job| !job.is_running()) {
        " Backspace ana ekran · PgUp/PgDn kaydır  "
    } else {
        " Ctrl+P scriptler · Ctrl+R tekrar · Ctrl+C durdur/çık · PgUp/PgDn kaydır  "
    };
    let right = Span::styled(hint, Style::new().fg(MUTED));

    let mut spans = left;
    let left_w: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let right_w = right.content.chars().count();
    if right_w < r.width as usize {
        let pad = r.width as usize - right_w;
        if left_w < pad {
            spans.push(Span::raw(" ".repeat(pad.saturating_sub(left_w))));
            spans.push(right);
            f.render_widget(Paragraph::new(Line::from(spans)).style(Style::new().bg(Color::Black)), r);
            return;
        }
    }
    spans.push(right);
    f.render_widget(Paragraph::new(Line::from(spans)).style(Style::new().bg(Color::Black)), r);
}

fn status_left(app: &App) -> Vec<Span<'static>> {
    if let Some(job) = &app.job {
        if job.is_running() {
            let el = util::fmt_duration(job.started.elapsed());
            return vec![
                Span::styled(format!(" {} {} ", app.spinner(), job.phase_label), Style::new().fg(ACCENT).bold()),
                Span::styled(format!(" {} ", job.title), Style::new().fg(Color::White)),
                Span::styled(el, Style::new().fg(MUTED)),
            ];
        }
    }
    if let Some(s) = &app.last {
        let (icon, color) = match s.outcome {
            Some(Outcome::Ok) => (Outcome::Ok.icon(), OK),
            Some(Outcome::Failed(_)) => (Outcome::Failed(0).icon(), ERR),
            Some(Outcome::Killed) => (Outcome::Killed.icon(), Color::Rgb(255, 190, 60)),
            None => ("•", MUTED),
        };
        let mut left = vec![
            Span::styled(format!(" {icon} "), Style::new().fg(color).bold()),
            Span::styled(s.title.clone(), Style::new().fg(Color::White)),
            Span::styled(" tamamlandı", Style::new().fg(MUTED)),
        ];
        if let Some(d) = s.duration {
            left.push(Span::styled(format!(" · {}", util::fmt_duration(d)), Style::new().fg(MUTED)));
        }
        return left;
    }
    Span::styled(" hazır", Style::new().fg(MUTED)).into_span_list()
}

// ---------------------------------------------------------------- palette

fn draw_palette(f: &mut Frame, app: &App, bounds: Rect) {
    // (args_mode veri topu: ödünç krizini önlemek için yukarıda klonla)
    let args_mode = app.palette.as_ref().and_then(|p| p.args_mode.clone());

    if let Some((idx, args_text, cursor)) = args_mode {
        let Some(script) = app.scripts.get(idx) else { return };
        let width = bounds.width.saturating_sub(4).min(82).max(20);
        let height = 5.min(bounds.height);
        let r = centered(width, height, bounds);
        draw_args_modal(f, script, &args_text, cursor, r);
        return;
    }

    let Some(pal) = app.palette.as_ref() else { return };
    let width = bounds.width.saturating_sub(4).min(92).max(20);
    let row_count = palette_row_count(app, pal);
    let list_h = (row_count.max(1) as u16)
        .min(bounds.height.saturating_sub(8))
        .clamp(1, 16);
    let height = (list_h + 4).min(bounds.height);
    let r = centered(width, height, bounds);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(Color::Rgb(50, 50, 50)))
        .title(Span::styled(" scripts ", Style::new().fg(ACCENT).bold()))
        .style(Style::new().bg(Color::Black));
    let inner = block.inner(r);
    f.render_widget(Clear, r);
    f.render_widget(block, r);

    let cols = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(2),
        Constraint::Length(1),
    ])
    .split(inner);

    // filtre satırı
    draw_input_line(f, "> ", &pal.query, pal.cursor, cols[0]);
    let count = Span::styled(format!(" {} eşleşme ", pal.items.len()), Style::new().fg(MUTED));
    let cx = cols[0].x + cols[0].width.saturating_sub(count.content.chars().count() as u16);
    f.render_widget(Paragraph::new(Line::from(count)), Rect { x: cx, y: cols[0].y, width: cols[0].width.saturating_sub(pal.query.chars().count() as u16 + 4), height: 1 });

    // liste
    let mut items = Vec::with_capacity(row_count);
    let mut selected_row = None;
    let mut last_lang = None;
    for (pos, &si) in pal.items.iter().enumerate() {
        let script = &app.scripts[si];
        if last_lang != Some(script.lang) {
            items.push(make_section_item(script.lang));
            last_lang = Some(script.lang);
        }
        if pos == pal.selected {
            selected_row = Some(items.len());
        }
        items.push(make_list_item(&app.scripts, si, pos == pal.selected, cols[1].width));
    }
    if items.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "  Eşleşen script yok",
            Style::new().fg(MUTED),
        ))));
    }

    let mut state = ratatui::widgets::ListState::default();
    state.select(selected_row);
    let list = List::new(items)
        .highlight_style(Style::new().bg(SEL_BG).fg(Color::White))
        .style(Style::new().bg(Color::Black));
    f.render_stateful_widget(list, cols[1], &mut state);

    // hint
    let hint = Span::styled(" ↑↓ seç · Enter: argüman ver · Tab: hemen çalıştır · Esc kapat", Style::new().fg(MUTED));
    f.render_widget(Paragraph::new(Line::from(hint)), cols[2]);
}

fn palette_row_count(app: &App, pal: &crate::app::Palette) -> usize {
    if pal.items.is_empty() {
        return 1;
    }
    let mut count = 0;
    let mut last_lang = None;
    for &si in &pal.items {
        let lang = app.scripts[si].lang;
        if last_lang != Some(lang) {
            count += 1;
            last_lang = Some(lang);
        }
        count += 1;
    }
    count
}

fn make_section_item(lang: Lang) -> ListItem<'static> {
    let title = match lang {
        Lang::Python => "Python",
        Lang::Node => "Node.js",
        Lang::Rust => "Rust",
        Lang::Custom => "Özel komutlar",
    };
    ListItem::new(Line::from(Span::styled(
        format!(" ── {title} ──"),
        Style::new().fg(lang_color(lang)).bold(),
    )))
}

fn make_list_item(scripts: &[crate::discovery::Script], si: usize, sel: bool, width: u16) -> ListItem<'static> {
    let s = &scripts[si];
    let w = width as usize;

    let mut name = s.key.clone();
    if !s.available {
        name.push_str(" ⚠");
    }
    let max_name = w.saturating_sub(14).max(6);
    if name.chars().count() > max_name {
        name = name.chars().take(max_name.saturating_sub(1)).collect::<String>() + "…";
    }
    let tag = format!(" [{}]", s.lang.label());
    let desc = s.description.clone();
    let desc_max = w.saturating_sub(name.chars().count() + tag.chars().count() + 14);
    let desc = if desc.chars().count() > desc_max {
        let mut d: String = desc.chars().take(desc_max).collect();
        d.push('…'); d
    } else { desc };

    let name_len = name.chars().count();
    let tag_len = tag.chars().count();
    let name_color = if s.available { Color::White } else { MUTED };
    let mut spans = vec![
        Span::styled(if sel { "▸ " } else { "  " }.to_string(), Style::new().fg(if sel { ACCENT } else { MUTED }).bold()),
        Span::styled(name, Style::new().fg(name_color).bold().add_modifier(if s.available { Modifier::empty() } else { Modifier::DIM })),
        Span::styled(tag, Style::new().fg(lang_color(s.lang))),
    ];
    let fill = w.saturating_sub(name_len + tag_len + desc.chars().count() + 19);
    if !desc.is_empty() {
        spans.push(Span::styled(format!("  {}", desc), Style::new().fg(MUTED)));
    }
    if fill > 0 {
        spans.push(Span::raw(" ".repeat(fill)));
    }
    ListItem::new(Line::from(spans))
}

fn draw_args_modal(f: &mut Frame, script: &crate::discovery::Script, args: &str, cursor: usize, r: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(Color::Rgb(50, 50, 50)))
        .title(Span::styled(
            format!(" args → {} [{}] ", script.name, script.lang.label()),
            Style::new().fg(ACCENT).bold(),
        ))
        .style(Style::new().bg(Color::Black));
    let inner = block.inner(r);
    f.render_widget(Clear, r);
    f.render_widget(block, r);

    let cols = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);

    // açıklama ya da yol
    let desc = if script.description.is_empty() {
        script.path.to_string_lossy().into_owned()
    } else {
        script.description.clone()
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(desc, Style::new().fg(MUTED)))),
        cols[0],
    );
    draw_input_line(f, "args › ", args, cursor, cols[1]);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            if script.default_args.is_empty() {
                " Enter çalıştır · Esc geri".to_string()
            } else {
                format!(" varsayılan: \"{}\" · Enter çalıştır · Esc geri", script.default_args)
            },
            Style::new().fg(MUTED),
        ))),
        cols[2],
    );
}

// ---------------------------------------------------------------- helpers

/// metin entered: prompt + [from..to] pencere + imleç bloğu
fn draw_input_line(f: &mut Frame, prompt: &str, value: &str, cursor: usize, r: Rect) {
    let avail = r.width.saturating_sub(prompt.chars().count() as u16 + 2) as usize;
    let chars: Vec<char> = value.chars().collect();
    let start = cursor.saturating_sub(avail);
    let end = (start + avail).min(chars.len());

    let pre: String = chars[start..cursor].iter().collect();
    let cur: String = chars.get(cursor).map(|c| c.to_string()).unwrap_or_else(|| " ".into());
    let post: String = chars[(cursor + 1).min(end)..end].iter().collect();

    let mut spans = vec![
        Span::styled(" ", Style::new()),
        Span::styled(prompt.to_string(), Style::new().fg(ACCENT).bold()),
    ];
    if start > 0 {
        spans.push(Span::styled("…", Style::new().fg(MUTED)));
    }
    spans.push(Span::raw(pre));
    spans.push(Span::styled(cur, Style::new().fg(Color::Black).bg(ACCENT)));
    spans.push(Span::raw(post));
    if end < chars.len() {
        spans.push(Span::styled("…", Style::new().fg(MUTED)));
    }

    f.render_widget(Paragraph::new(Line::from(spans)).style(Style::new().bg(Color::Black)), r);
}

fn centered(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect { x, y, width, height }
}

// Span::into_span_list yerine küçük yardımcı
trait SpanExt {
    fn into_span_list(self) -> Vec<Span<'static>>;
}

impl SpanExt for Span<'static> {
    fn into_span_list(self) -> Vec<Span<'static>> {
        vec![self]
    }
}
