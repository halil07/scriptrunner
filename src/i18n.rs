use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Turkish,
    English,
}

static LANGUAGE: OnceLock<Language> = OnceLock::new();

pub fn current() -> Language {
    *LANGUAGE.get_or_init(detect)
}

pub fn text<'a>(turkish: &'a str, english: &'a str) -> &'a str {
    match current() {
        Language::Turkish => turkish,
        Language::English => english,
    }
}

fn detect() -> Language {
    if let Ok(value) = std::env::var("SR_LANG") {
        match value.trim().to_ascii_lowercase().as_str() {
            "tr" | "turkish" | "türkçe" => return Language::Turkish,
            "en" | "english" => return Language::English,
            _ => {}
        }
    }

    #[cfg(windows)]
    let locale = std::env::var("LANG")
        .ok()
        .or_else(windows_locale)
        .unwrap_or_default();

    #[cfg(not(windows))]
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .find_map(|key| std::env::var(key).ok().filter(|value| !value.is_empty()))
        .unwrap_or_default();

    language_from_locale(&locale)
}

fn language_from_locale(locale: &str) -> Language {
    if locale.trim().to_ascii_lowercase().starts_with("tr") {
        Language::Turkish
    } else {
        Language::English
    }
}

#[cfg(windows)]
fn windows_locale() -> Option<String> {
    const LOCALE_NAME_MAX_LENGTH: usize = 85;
    let mut buffer = [0u16; LOCALE_NAME_MAX_LENGTH];
    let length = unsafe {
        GetUserDefaultLocaleName(buffer.as_mut_ptr(), LOCALE_NAME_MAX_LENGTH as i32)
    };
    if length <= 1 {
        return None;
    }
    String::from_utf16(&buffer[..length as usize - 1]).ok()
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn GetUserDefaultLocaleName(locale_name: *mut u16, cch_locale_name: i32) -> i32;
}

#[cfg(test)]
mod tests {
    use super::{language_from_locale, Language};

    #[test]
    fn recognizes_turkish_locale_variants() {
        assert_eq!(language_from_locale("tr_TR.UTF-8"), Language::Turkish);
        assert_eq!(language_from_locale("tr-TR"), Language::Turkish);
    }

    #[test]
    fn defaults_other_locales_to_english() {
        assert_eq!(language_from_locale("en_US.UTF-8"), Language::English);
        assert_eq!(language_from_locale("C"), Language::English);
    }
}
