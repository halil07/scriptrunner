pub fn text() -> String {
    format!(
        r#"scriptrunner {version} — terminalden script bul ve çalıştır

KULLANIM
  scriptrunner                 Ana ekranı (TUI) açar
  scriptrunner -t, --tui       Ana ekranı açar
  scriptrunner -h, --help      Bu yardımı gösterir
  scriptrunner help            Bu yardımı gösterir
  scriptrunner -l, --list      Bulunan scriptleri ve kullanılabilirliklerini listeler
  scriptrunner -d, --dir       Script klasörünün yolunu yazdırır
  scriptrunner -o, --open      Script klasörünü açar (Windows); diğer sistemlerde yolu yazdırır
  scriptrunner run <anahtar> [argümanlar...]
                               Scripti TUI olmadan çalıştırır; scriptin çıkış kodunu döndürür

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
  Script klasörü: işletim sisteminin uygulama verisi dizinindeki scriptrunner/scripts
  Tam yolu görmek için: scriptrunner --dir
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
  config.toml, işletim sisteminin uygulama verisi dizinindeki scriptrunner klasöründedir.
  Derleme cache'i işletim sisteminin kullanıcı cache dizininde tutulur.
  SCRIPTRUNNER_CONFIG ortam değişkeniyle config/script kök dizini değiştirilebilir.
  -d / --dir script klasörünün yolunu gösterir; -o / --open klasörü açar.
  İlk TUI açılışında config.toml oluşturma sorulur, yorumlayıcılar denetlenir;
  eksik varsa devam etmeden önce onay istenir.

SINIRLAR
  - Aynı anda yalnızca bir iş çalışır; ikinci işi başlatmadan önce ilkini durdurun.
  - Scriptler stdin'den etkileşimli girdi alamaz.
  - Tek .rs dosyaları desteklenir; harici crate kullanan Rust projeleri için manifestte
    cargo gibi bir komut tanımlayın.
  - Uygulama script düzenlemez/silmez ve scriptleri sandbox içinde çalıştırmaz.
    Yalnızca güvendiğiniz scriptleri ve manifest komutlarını çalıştırın.
"#,
        version = env!("CARGO_PKG_VERSION")
    )
}
