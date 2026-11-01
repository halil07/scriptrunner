pub fn text() -> String {
    text_for(crate::i18n::current())
}

fn text_for(language: crate::i18n::Language) -> String {
    match language {
        crate::i18n::Language::Turkish => format!(
            r#"sr {version} — terminalden script bul ve çalıştır

KULLANIM
  sr                 Ana ekranı (TUI) açar
  sr -t, --tui       Ana ekranı açar
  sr -h, --help      Bu yardımı gösterir
  sr help            Bu yardımı gösterir
  sr -l, --list      Bulunan scriptleri ve kullanılabilirliklerini listeler
  sr -d, --dir       Script klasörünün yolunu yazdırır
  sr -o, --open      Script klasörünü açar (Windows); diğer sistemlerde yolu yazdırır
  sr run <anahtar> [argümanlar...]
                     Scripti TUI olmadan çalıştırır ve çıkış kodunu döndürür

TUI TUŞLARI
  ↑ / ↓ veya j / k             Menüde ya da listede gezin
  Enter                        Seçimi onayla; scriptte argümanları düzenle
  Ctrl+P                        Script paletini aç/kapat
  Tab                          Seçili scripti varsayılan argümanlarla hemen çalıştır
  Ctrl+U / Ctrl+W               Giriş alanını temizle / önceki kelimeyi sil
  Ctrl+R                        Son çalıştırılan scripti ve argümanlarını tekrarla
  Ctrl+C                        Çalışan scripti durdur; iş yoksa uygulamadan çık
  Esc                          Paleti kapat veya argüman girişinden geri dön
  Backspace                    Biten iş ekranından ana menüye dön
  PgUp / PgDn, ↑ / ↓, Home / End Çıktıyı kaydır
  q                            Ana ekrandan çık

SCRIPT EKLEME
  Script klasörünü görmek için: sr --dir
  Alt klasörler taranır; örneğin python\hello.py anahtarı python/hello.py olur.
  - .py / .pyw        Python ile çalışır (python[3] -u)
  - .js / .mjs / .cjs Node.js ile çalışır
  - .rs               rustc ile derlenip çalışır; derlenmiş dosya cache'te tutulur
  - script.toml       Bir klasörde özel komut tanımlar (örnek aşağıda)

ÖZEL KOMUT (script.toml)
  name = "deploy"
  description = "Uygulamayı yayımla"
  command = 'pwsh -NoProfile -File {{{{dir}}}}\deploy.ps1 {{{{args}}}}'
  default_args = "--target prod"
  cwd = "." # isteğe bağlı; göreli yollar script klasörüne göredir

  {{{{dir}}}}  script klasörünün tam yolu, {{{{args}}}} girilen argümanlardır.
  {{{{args}}}} kullanılmazsa girilen argümanlar komutun sonuna eklenir.

YAPILANDIRMA
  Yorumlayıcılar PATH'te aranır. Bulunamazlarsa config.toml içindeki
  [interpreters] bölümüne python, node veya rustc için tam yolu yazın.
  config.toml, script klasörüyle aynı uygulama verisi alanındadır.
  Derleme cache'i işletim sisteminin kullanıcı cache dizininde tutulur.
  SCRIPTRUNNER_CONFIG ortam değişkeniyle config/script kök dizini değiştirilebilir.
  Arayüz dilini sistem seçer; SR_LANG=tr veya SR_LANG=en ile değiştirebilirsiniz.
  İlk TUI açılışında config.toml oluşturma sorulur ve yorumlayıcılar denetlenir.

SINIRLAR
  - Aynı anda yalnızca bir iş çalışır; ikinci işi başlatmadan önce ilkini durdurun.
  - Scriptler stdin'den etkileşimli girdi alamaz.
  - Tek .rs dosyaları desteklenir; harici crate kullanan Rust projeleri için manifestte
    cargo gibi bir komut tanımlayın.
  - Uygulama script düzenlemez/silmez ve scriptleri sandbox içinde çalıştırmaz.
    Yalnızca güvendiğiniz scriptleri ve manifest komutlarını çalıştırın.
"#,
            version = env!("CARGO_PKG_VERSION")
        ),
        crate::i18n::Language::English => format!(
            r#"sr {version} — find and run scripts from your terminal

USAGE
  sr                 Open the terminal UI (TUI)
  sr -t, --tui       Open the TUI
  sr -h, --help      Show this help
  sr help            Show this help
  sr -l, --list      List discovered scripts and their availability
  sr -d, --dir       Print the scripts directory
  sr -o, --open      Open the scripts directory (Windows); print its path elsewhere
  sr run <key> [args...]
                     Run a script without the TUI and return its exit code

TUI KEYS
  Up / Down or j / k           Move through menus and lists
  Enter                        Confirm a selection or edit script arguments
  Ctrl+P                       Open or close the script palette
  Tab                          Run the selected script with its default arguments
  Ctrl+U / Ctrl+W              Clear the input / delete the previous word
  Ctrl+R                       Rerun the last script with the same arguments
  Ctrl+C                       Stop a running script; quit if nothing is running
  Esc                          Close the palette or return from argument entry
  Backspace                    Return from a completed job to the main menu
  PgUp / PgDn, Up / Down, Home / End  Scroll output
  q                            Quit from the main screen

ADDING SCRIPTS
  Run `sr --dir` to print the scripts directory.
  Subdirectories are scanned; python/hello.py gets the key python/hello.py.
  - .py / .pyw         Run with Python (python[3] -u)
  - .js / .mjs / .cjs  Run with Node.js
  - .rs                Compile with rustc and cache the executable
  - script.toml        Define a custom command in a directory (see below)

CUSTOM COMMAND (script.toml)
  name = "deploy"
  description = "Deploy the application"
  command = 'pwsh -NoProfile -File {{{{dir}}}}\deploy.ps1 {{{{args}}}}'
  default_args = "--target prod"
  cwd = "." # optional; relative paths use the script directory

  {{{{dir}}}} is the full script directory; {{{{args}}}} expands to the supplied arguments.
  If {{{{args}}}} is omitted, supplied arguments are appended to the command.

CONFIGURATION
  Interpreters are searched on PATH. If one is missing, set its full path under
  [interpreters] in config.toml (python, node, or rustc).
  config.toml is stored alongside the scripts directory; the build cache uses the
  operating system's user cache directory.
  Set SCRIPTRUNNER_CONFIG to change the config and scripts root.
  On first TUI launch, sr asks whether to create config.toml and checks interpreters.
  Set SR_LANG=tr or SR_LANG=en to override the detected interface language.

LIMITATIONS
  - Only one job can run at a time; stop it before starting another.
  - Scripts cannot receive interactive input from stdin.
  - Standalone .rs files are supported. Use a manifest command such as cargo for
    Rust projects that depend on external crates.
  - sr does not edit or delete scripts and does not sandbox commands.
    Only run scripts and manifest commands you trust.
"#,
            version = env!("CARGO_PKG_VERSION")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::text_for;
    use crate::i18n::Language;

    #[test]
    fn both_help_languages_use_the_sr_command_name() {
        for language in [Language::Turkish, Language::English] {
            assert!(text_for(language).starts_with("sr "));
        }
    }
}
