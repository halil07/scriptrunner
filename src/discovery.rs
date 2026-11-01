//! Script keşfi.
//!
//! scripts klasöründe (alt klasörler dahil — örn. python/, nodejs/, rust/):
//!  - .py/.pyw  -> Python (python -u <dosya>)
//!  - .js/.mjs/.cjs -> Node.js (node <dosya>)
//!  - .rs       -> Rust (rustc ile derle-çalıştır, cache'lenir)
//!  - klasör + script.toml -> özel manifest:
//!
//!      name         = "araclarim"
//!      description  = "..."
//!      command      = "cargo run --manifest-path {{dir}}\\Cargo.toml -- {{args}}"
//!      default_args = "--verbose"
//!      cwd          = "."          # script klasörüne göreli (isteğe bağlı)
//!
//! `{{args}}` verdiğiniz argümanlarla genişletilir (yoksa sona eklenir),
//! `{{dir}}` script klasörünün tam yoludur.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::config::AppConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Python,
    Node,
    Rust,
    Custom,
}

impl Lang {
    pub fn label(self) -> &'static str {
        match self {
            Lang::Python => "python",
            Lang::Node => "node",
            Lang::Rust => "rust",
            Lang::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Script {
    /// Anahtar: scripts klasörüne göre göreli ad ("hello.py", "deploy" ...)
    pub key: String,
    pub name: String,
    pub description: String,
    pub lang: Lang,
    /// Çalıştırılan dosya (tek dosyalar) veya klasör (manifest'li)
    pub path: PathBuf,
    /// Yalnızca manifest'li scriptler: komut şablonu
    pub command: Option<String>,
    pub default_args: String,
    /// Manifest cwd bilgisi (klasöre göreli ya da mutlak)
    pub cwd: Option<String>,
    /// Yorumlayıcı/komut kullanılabilir mi?
    pub available: bool,
    /// Uygun değilse kısa sebep
    pub missing_reason: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
struct Manifest {
    name: Option<String>,
    description: Option<String>,
    command: Option<String>,
    #[serde(rename = "default_args")]
    default_args: Option<String>,
    cwd: Option<String>,
}

fn rel_key(base: &Path, p: &Path) -> String {
    let rel = p.strip_prefix(base).unwrap_or(p);
    rel.to_string_lossy().replace('\\', "/")
}

const MAX_SCAN_DEPTH: usize = 8;

/// İçinde script aranmayacak klasörler (ağırlıklı olarak büyük direktoryalar).
const SKIP_DIRS: [&str; 1] = ["node_modules"];

/// scripts klasörünü alt klasörlerdeki (dili başına klasör düzeni: python/,
/// nodejs/, rust/ vb.) scriptleri bulmak için özyineleli tarar.
pub fn scan(cfg: &AppConfig) -> Vec<Script> {
    let mut out: Vec<Script> = Vec::new();
    let dir = cfg.scripts_dir.clone();
    scan_dir(&dir, &dir, cfg, 0, &mut out);
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

fn scan_dir(dir: &Path, base: &Path, cfg: &AppConfig, depth: usize, out: &mut Vec<Script>) {
    if depth >= MAX_SCAN_DEPTH {
        return; // sağlamlık payı: beklenmedik döngülere/kurma derinliklerine karşı
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if file_name.starts_with('.') || file_name.starts_with('_') {
            continue; // gizli/yardımcı dosya ve klasörleri yut
        }

        if path.is_dir() {
            let manifest_path = path.join("script.toml");
            if manifest_path.is_file() {
                let text = match std::fs::read_to_string(&manifest_path) {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let m: Manifest = toml::from_str(&text).ok().unwrap_or_default();
                let has_cmd = m.command.is_some();
                out.push(Script {
                    key: rel_key(base, &path),
                    name: m.name.unwrap_or_else(|| file_name.clone()),
                    description: m.description.unwrap_or_default(),
                    lang: Lang::Custom,
                    path: path.clone(),
                    command: m.command,
                    default_args: m.default_args.unwrap_or_default(),
                    cwd: m.cwd,
                    available: has_cmd,
                    missing_reason: if has_cmd {
                        None
                    } else {
                        Some("script.toml içinde 'command' eksik".into())
                    },
                });
                continue; // manifest klasörünün içine inilmez
            }
            if SKIP_DIRS.contains(&file_name.to_ascii_lowercase().as_str()) {
                continue;
            }
            scan_dir(&path, base, cfg, depth + 1, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let lang = match ext.to_ascii_lowercase().as_str() {
                "py" | "pyw" => Some(Lang::Python),
                "js" | "mjs" | "cjs" => Some(Lang::Node),
                "rs" => Some(Lang::Rust),
                _ => None,
            };
            if let Some(lang) = lang {
                let bin = match lang {
                    Lang::Python => &cfg.python,
                    Lang::Node => &cfg.node,
                    _ => &cfg.rustc,
                };
                let exists = crate::util::find_in_path(bin).is_some();
                out.push(Script {
                    key: rel_key(base, &path),
                    name: path
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| file_name.clone()),
                    description: String::new(),
                    lang,
                    path: path.clone(),
                    command: None,
                    default_args: String::new(),
                    cwd: None,
                    available: exists,
                    missing_reason: (!exists).then(|| format!("'{bin}' bulunamadı - config.toml'a tam yolu yazın")),
                });
            }
        }
    }
}

/// CLI'dan anahtarla script bulur (büyük/küçük harf duyarsız).
pub fn find_key<'a>(scripts: &'a [Script], key: &str) -> Option<&'a Script> {
    scripts
        .iter()
        .find(|s| s.key.eq_ignore_ascii_case(key))
}

/// Keşfi doğrulamak için küçük bir test.
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn test_cfg(tmp: std::path::PathBuf) -> AppConfig {
        AppConfig {
            root: tmp.clone(),
            scripts_dir: tmp.join("scripts"),
            cache_dir: tmp.join("cache"),
            python: "python".into(),
            node: "node".into(),
            rustc: "rustc".into(),
        }
    }

    #[test]
    fn scan_empty() {
        let tmp = std::env::temp_dir().join(format!("sr_test_{}", std::process::id()));
        fs::create_dir_all(tmp.join("scripts")).unwrap();
        let cfg = test_cfg(tmp.clone());
        assert!(scan(&cfg).is_empty());
        fs::remove_dir_all(tmp).ok();
    }

    /// Dil klasörleri (python/, nodejs/, rust/) ve iç içe alt klasörler taranmalı.
    #[test]
    fn scan_alt_klasorleri_bulur() {
        let tmp = std::env::temp_dir().join(format!("sr_test_sub_{}", std::process::id()));
        fs::create_dir_all(tmp.join("scripts/python")).unwrap();
        fs::create_dir_all(tmp.join("scripts/nodejs")).unwrap();
        fs::create_dir_all(tmp.join("scripts/tools/rust")).unwrap();
        fs::create_dir_all(tmp.join("scripts/node_modules/x")).unwrap();
        fs::write(tmp.join("scripts/python/hello.py"), "print(1)").unwrap();
        fs::write(tmp.join("scripts/nodejs/hello.js"), "console.log(1)").unwrap();
        fs::write(tmp.join("scripts/tools/rust/main.rs"), "fn main() {}").unwrap();
        fs::write(tmp.join("scripts/node_modules/x/a.py"), "print(2)").unwrap();

        let cfg = test_cfg(tmp.clone());
        let keys: Vec<String> = scan(&cfg).iter().map(|s| s.key.clone()).collect();
        assert!(keys.contains(&"python/hello.py".into()), "{keys:?}");
        assert!(keys.contains(&"nodejs/hello.js".into()), "{keys:?}");
        assert!(keys.contains(&"tools/rust/main.rs".into()), "{keys:?}");
        assert!(!keys.contains(&"node_modules/x/a.py".into()), "{keys:?}");
        fs::remove_dir_all(tmp).ok();
    }

    /// Manifest klasörleri alt klasörden de bulunur ve içine inilmez.
    #[test]
    fn scan_manifest_alt_klasorde() {
        let tmp = std::env::temp_dir().join(format!("sr_test_man_{}", std::process::id()));
        let man = tmp.join("scripts/deploy Tools").join("Alt Klasör");
        fs::create_dir_all(&man).unwrap();
        fs::write(man.join("script.toml"), "name = \"arkas\"\ncommand = 'python {{dir}}\\x.py {{args}}'\n").unwrap();
        fs::write(man.join("x.py"), "print(1)").unwrap();

        let cfg = test_cfg(tmp.clone());
        let scripts = scan(&cfg);
        let s = scripts.iter().find(|s| s.key == "deploy Tools/Alt Klasör").expect("manifest bulunamadı");
        assert_eq!(s.name, "arkas");
        assert!(s.available);
        fs::remove_dir_all(tmp).ok();
    }
}
