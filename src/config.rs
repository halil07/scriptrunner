//! Yol ve yapılandırma yönetimi.
//!
//! Root (Windows): %APPDATA%\scriptrunner
//!   - config.toml        (isteğe bağlı: yorumlayıcı ayarları)
//!   - scripts\           (scriptler buraya bırakılır)
//! Cache (Windows): %LOCALAPPDATA%\scriptrunner\cache  (rustc derlemeleri vb.)

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Deserialize;

pub const APP_ID: &str = "scriptrunner";

#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Yapılandırma kökü (ör. C:\Users\<user>\AppData\Roaming\scriptrunner)
    #[allow(dead_code)]
    pub root: PathBuf,
    pub scripts_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub python: String,
    pub node: String,
    pub rustc: String,
}

#[derive(Deserialize, Default, Debug)]
struct InterpToml {
    #[serde(default)]
    python: Option<String>,
    #[serde(default)]
    node: Option<String>,
    #[serde(default)]
    rustc: Option<String>,
}

#[derive(Deserialize, Default, Debug)]
struct ConfigToml {
    #[serde(default)]
    interpreters: InterpToml,
}

fn default_data_dir() -> PathBuf {
    // dirs::data_dir() Windows'ta %APPDATA% (Roaming) verir.
    dirs::data_dir()
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_ID)
}

fn default_cache_dir() -> PathBuf {
    // Windows'ta %LOCALAPPDATA% ; Linux/macOS'ta ~/.cache
    dirs::cache_dir()
        .unwrap_or_else(|| default_data_dir().join("cache"))
        .join(APP_ID)
        .join("cache")
}

impl AppConfig {
    /// Dizinleri oluşturur, config.toml varsa okur, yoksa örnek bir dosya yazar.
    pub fn load() -> Result<AppConfig> {
        let overridden = std::env::var_os("SCRIPTRUNNER_CONFIG").map(PathBuf::from);
        let root = overridden.unwrap_or_else(default_data_dir);
        let scripts_dir = root.join("scripts");
        let cache_dir = default_cache_dir();

        fs::create_dir_all(&root)?;
        fs::create_dir_all(&scripts_dir)?;
        fs::create_dir_all(&cache_dir.join("rust"))?;

        let cfg_path = root.join("config.toml");
        let mut app = AppConfig {
            root,
            scripts_dir,
            cache_dir,
            python: "python".into(),
            node: "node".into(),
            rustc: "rustc".into(),
        };

        if cfg_path.exists() {
            let text = fs::read_to_string(&cfg_path).unwrap_or_default();
            if let Ok(c) = toml::from_str::<ConfigToml>(&text) {
                if let Some(v) = c.interpreters.python { app.python = v; }
                if let Some(v) = c.interpreters.node { app.node = v; }
                if let Some(v) = c.interpreters.rustc { app.rustc = v; }
            }
        } else {
            fs::write(&cfg_path, sample_config()).ok();
        }

        // rustc PATH'te yoksa bilinen konumu dene (rustup varsayılan kurulum yeri)
        if crate::util::find_in_path(&app.rustc).is_none() {
            if let Some(home) = dirs::home_dir() {
                let cand = home.join(r".cargo\bin\rustc.exe");
                if cand.is_file() {
                    app.rustc = cand.to_string_lossy().into_owned();
                }
            }
        }
        Ok(app)
    }

    #[allow(dead_code)]
    pub fn config_path(&self) -> PathBuf {
        self.root.join("config.toml")
    }
    // (root alanı ilerideki CLI komutları için tutuluyor)
}

/// İlk açılışta yazılacak, yorumlu örnek yapılandırma.
fn sample_config() -> String {
    let py = crate::util::find_in_path("python").is_some();
    let node = crate::util::find_in_path("node").is_some();
    let rust = crate::util::find_in_path("rustc").is_some();
    let note = |ok: bool| if ok { "(PATH'te bulundu)" } else { "(PATH'te bulunamadı - tam yol verin)" };

    format!(
        "# scriptrunner yapılandırma dosyası\n\
         # Yorumlayıcılar varsayılan olarak PATH'te aranır.\n\
         # Bulunmazsa veya özel bir yol kullanmak isterseniz buraya tam yolu yazın.\n\n\
         [interpreters]\n\
         # python = \"C:\\\\Python314\\\\python.exe\"     {}\n\
         # node   = \"C:\\\\Program Files\\\\nodejs\\\\node.exe\"  {}\n\
         # rustc  = \"C:\\\\Users\\\\halil\\\\.cargo\\\\bin\\\\rustc.exe\" {}\n",
        note(py), note(node), note(rust)
    )
}

pub fn shorten(path: &Path, max: usize) -> String {
    let s = path.to_string_lossy().into_owned();
    if s.chars().count() <= max {
        return s;
    }
    // Uzun yollarda ortayı kes: "C:\Users\...\scriptrunner\scripts"
    let chars: Vec<char> = s.chars().collect();
    let half = max / 2;
    let head: String = chars[..half].iter().collect();
    let tail: String = chars[chars.len() - half..].iter().collect();
    format!("…{}{}", head, tail)
}
