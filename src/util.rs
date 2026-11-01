//! Küçük yardımcılar: fuzzy arama, arg/komut ayrıştırma, ANSI temizleme vb.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// Basit fuzzy skor: query'in hay_string alt dizisi (subsequence) olmasına bakar.
/// Ardışık eşleşme ve kelime başı bonusu verir. None = eşleşme yok.
pub fn fuzzy_score(query: &str, hay: &str) -> Option<i32> {
    let q: Vec<char> = query.chars().flat_map(|c| c.to_lowercase()).collect();
    if q.is_empty() {
        return Some(0);
    }
    let h: Vec<char> = hay.chars().flat_map(|c| c.to_lowercase()).collect();
    let mut score = 0i32;
    let mut qi = 0usize;
    let mut prev_match: Option<usize> = None;
    for (i, &hc) in h.iter().enumerate() {
        if qi < q.len() && q[qi] == hc {
            qi += 1;
            score += if prev_match == Some(i.saturating_sub(1)) { 2 } else { 1 };
            let word_start = i == 0 || !h[i - 1].is_alphanumeric();
            if word_start {
                score += 3;
            }
            prev_match = Some(i);
        }
    }
    if qi == q.len() { Some(score) } else { None }
}

/// Boşlukla ayrılmış argüman satırını argv listesine çevirir.
/// "çift" ve 'tek' tırnak desteklenir; ters eğik çizgi özel bir anlama gelmez.
pub fn split_args(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = line.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' => {
                chars.next();
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            '"' | '\'' => {
                let quote = c;
                chars.next();
                while let Some(&c2) = chars.peek() {
                    if c2 == quote {
                        chars.next();
                        break;
                    }
                    cur.push(c2);
                    chars.next();
                }
            }
            _ => {
                cur.push(c);
                chars.next();
            }
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Komut şablonunu ("cargo run -- {{args}}" gibi) argv listesine çevirir.
/// `{{args}}`, `{{dir}}` gibi token'lar olduğu gibi korunur (runner genişletir).
pub fn split_command(cmd: &str) -> Option<Vec<String>> {
    let words = split_args(cmd);
    (words.is_empty() || words[0].trim() != "{{args}}").then_some(words)
}

/// ANSI kaçış dizilerini (renk kodları vb.) temizler.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.peek() {
                Some('[') => {
                    chars.next();
                    while let Some(&nc) = chars.peek() {
                        chars.next();
                        if ('\x40'..='\x7e').contains(&nc) {
                            break;
                        }
                    }
                }
                // '[' değilse: ESC + tek karakter (OS komutları vb.) yut
                Some(_) => { chars.next(); }
                None => {}
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// PATH içinde çalıştırılabilir dosyayı arar (Windows: .exe uzantısı denenir).
pub fn find_in_path(bin: &str) -> Option<PathBuf> {
    let probe = if bin.contains('\\') || bin.contains('/') {
        vec![PathBuf::from(bin)]
    } else {
        let mut v = Vec::new();
        let binp = Path::new(bin);
        v.push(binp.to_path_buf());
        if cfg!(windows) {
            v.push(PathBuf::from(bin.to_owned() + ".exe"));
        }
        v
    };

    if bin.contains('\\') || bin.contains('/') {
        for p in &probe {
            if p.is_file() {
                return Some(p.clone());
            }
        }
        return None;
    }

    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for p in &probe {
            let full = dir.join(p);
            if full.is_file() {
                return Some(full);
            }
        }
    }
    None
}

/// Süreyi okunur biçimde yazar: "0.9s", "1m 05s", "1h 02m".
pub fn fmt_duration(d: Duration) -> String {
    let total = d.as_secs();
    let ms = d.subsec_millis();
    if total < 60 {
        format!("{}.{:02}s", total, ms / 10)
    } else if total < 3600 {
        format!("{}m {:02}s", total / 60, total % 60)
    } else {
        format!("{}h {:02}m", total / 3600, (total % 3600) / 60)
    }
}

/// Kaynak anahtarını derleme cache'i için güvenli dosya adına çevirir.
pub fn cache_file_name(key: &str) -> String {
    let bad = ['/', '\\', ':', '*', '?', '"', '<', '>', '|', ' '];
    let mut s: String = key.chars().map(|c| if bad.contains(&c) { '_' } else { c }).collect();
    s.truncate(80);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args() {
        assert_eq!(split_args("a b c"), vec!["a", "b", "c"]);
        assert_eq!(split_args("add \"Hello World\" --x 'a b'"), vec!["add", "Hello World", "--x", "a b"]);
        assert!(split_args("   ").is_empty());
    }

    #[test]
    fn ansi() {
        assert_eq!(strip_ansi("\x1b[31mred\x1b[0m"), "red");
        assert_eq!(strip_ansi("ok"), "ok");
    }

    #[test]
    fn fuzzy() {
        assert!(fuzzy_score("hp", "hello.py").is_some());
        assert!(fuzzy_score("xz", "hello.py").is_none());
        assert!(fuzzy_score("", "her şey").is_some());
    }

    #[test]
    fn cache_name() {
        assert_eq!(cache_file_name("tools/build.rs"), "tools_build.rs");
    }
}
