//! Yol ve yapılandırma yönetimi.
//!
//! Yapılandırma ve scriptler işletim sisteminin uygulama verisi dizinindeki
//! scriptrunner klasöründe; derleme cache'i ise kullanıcı cache dizinindedir.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Deserialize;

pub const APP_ID: &str = "scriptrunner";

fn default_python_command() -> &'static str {
    if cfg!(windows) || crate::util::find_in_path("python").is_some() {
        "python"
    } else {
        "python3"
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    /// İşletim sisteminin uygulama verisi altındaki scriptrunner dizini.
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
    dirs::data_dir()
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_ID)
}

fn config_root() -> PathBuf {
    std::env::var_os("SCRIPTRUNNER_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(default_data_dir)
}

fn default_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| default_data_dir().join("cache"))
        .join(APP_ID)
        .join("cache")
}

impl AppConfig {
    /// Dizinleri oluşturur ve config.toml varsa okur; yoksa örnek bir dosya yazar.
    pub fn load() -> Result<AppConfig> {
        Self::load_with_config_creation(true)
    }

    /// TUI ilk açılışında örnek config dosyasının oluşturulup oluşturulmayacağını seçer.
    pub fn load_with_config_creation(create_config: bool) -> Result<AppConfig> {
        let root = config_root();
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
            python: default_python_command().into(),
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
        } else if create_config {
            fs::write(&cfg_path, sample_config()).ok();
        }

        // rustc PATH'te yoksa rustup'ın varsayılan konumunu dene.
        if crate::util::find_in_path(&app.rustc).is_none() {
            if let Some(home) = dirs::home_dir() {
                #[cfg(windows)]
                let cand = home.join(".cargo").join("bin").join("rustc.exe");
                #[cfg(not(windows))]
                let cand = home.join(".cargo").join("bin").join("rustc");
                if cand.is_file() {
                    app.rustc = cand.to_string_lossy().into_owned();
                }
            }
        }
        Ok(app)
    }

    pub fn config_file_path() -> PathBuf {
        config_root().join("config.toml")
    }

    #[allow(dead_code)]
    pub fn config_path(&self) -> PathBuf {
        self.root.join("config.toml")
    }
    // (root alanı ilerideki CLI komutları için tutuluyor)
}

/// İlk açılışta yazılacak, yorumlu örnek yapılandırma.
fn sample_config() -> String {
    let python = default_python_command();
    let py = crate::util::find_in_path(python).is_some();
    let node = crate::util::find_in_path("node").is_some();
    let rust = crate::util::find_in_path("rustc").is_some();
    let note = |ok: bool| if ok { "(PATH'te bulundu)" } else { "(PATH'te bulunamadı - tam yol verin)" };
    let (python_path, node_path, rustc_path) = if cfg!(windows) {
        (
            r"C:\\Path\\To\\python.exe",
            r"C:\\Program Files\\nodejs\\node.exe",
            r"C:\\Users\\<kullanıcı_adı>\\.cargo\\bin\\rustc.exe",
        )
    } else {
        ("/usr/bin/python3", "/usr/bin/node", "/usr/bin/rustc")
    };

    format!(
        "# scriptrunner yapılandırma dosyası\n\
         # Yorumlayıcılar varsayılan olarak PATH'te aranır.\n\
         # Bulunmazsa veya özel bir yol kullanmak isterseniz buraya tam yolu yazın.\n\n\
         # Örnek yollardaki yer tutucuları kendi kurulumunuza göre değiştirin.\n\
         [interpreters]\n\
         # python = \"{}\"     {}\n\
         # node   = \"{}\"  {}\n\
         # rustc  = \"{}\" {}\n",
        python_path,
        note(py),
        node_path,
        note(node),
        rustc_path,
        note(rust)
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

#[cfg(test)]
mod tests {
    use super::{sample_config, ConfigToml};

    #[test]
    fn sample_config_is_valid_toml() {
        toml::from_str::<ConfigToml>(&sample_config()).expect("örnek config geçerli TOML olmalı");
    }
}
