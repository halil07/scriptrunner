# scriptrunner (`sr`)

OpenCode tarzı terminal UI'lı (TUI), Rust ile yazılmış bir script çalıştırıcı.
**Ctrl+P** ile script seçersin, argüman girersin, çalışır. AI yok — statik, hızlı ve padletsiz.

## Ekran görüntüleri

### Ana ekran

![sr ana ekranı](examples/screenshots/main.png)

### Script listesi

![sr script listesi](examples/screenshots/script_list.png)

### Çalışan script

![sr çalışan script çıktısı](examples/screenshots/run.png)

## Konumlar

| İşletim sistemi | Scriptler ve config | Rust derleme cache'i |
|---|---|---|
| Windows | `%APPDATA%\scriptrunner\` | `%LOCALAPPDATA%\scriptrunner\cache\` |
| Linux | `~/.local/share/scriptrunner/` | `~/.cache/scriptrunner/cache/` |
| macOS | `~/Library/Application Support/scriptrunner/` | `~/Library/Caches/scriptrunner/cache/` |

`SCRIPTRUNNER_CONFIG` ortam değişkeniyle kök dizini değiştirebilirsin.
Komut satırı program adı `sr`'dir. Script klasörünün gerçek yolunu `sr --dir` ile görebilirsin.

## Script tanımlama

`scripts` klasörüne dosya bırak, o kadar. Alt klasörler de taranır — dilleri
klasörlere ayırabilirsin (`scripts\python\`, `scripts\nodejs\`, `scripts\rust\` …):

- `ornek.py` / `ornek.pyw` → `python -u ornek.py <args>`
- `ornek.js` / `.mjs` / `.cjs` → `node ornek.js <args>`
- `ornek.rs` → `rustc` ile cache'e derlenir → derlenen program çalıştırılır
  (tek dosyada std dışı kütüphane yok; cargo projesi için manifest kullan)

Alt klasörlere koyulan scriptin anahtarı klasör yolunu içerir:
`scripts\python\hello.py` → `python/hello.py`, çalıştırma: `sr run python/hello.py`.
Script kendi klasöründe çalışır (göreli dosyalar yanındakilere gider).

Hazır örnekler: `examples/` klasöründe dil başına klasörler halinde durur
(`examples/python/hello.py`, `examples/nodejs/hello.js`, `examples/rust/hello.rs`,
`examples/manifest/`). Bırakmak: `sr --open` → klasörü aç → kopyala.

### Manifest (özel komutlar)

bir klasör içine `script.toml` (örnek: `examples/manifest/`):

```toml
name         = "deploy"
description  = "prod'a deploy eder"
command      = 'pwsh -NoProfile -File {{dir}}\deploy.ps1 {{args}}'
default_args = "--target prod"
# cwd       = ".."        # klasöre göreli çalışma dizini (isteğe bağlı)
```

- `{{dir}}` → klasörün tam yolu
- `{{args}}` → paletten girdiğin argümanlar (yoksa sona eklenir)
- Tırnaklı argümanlar destekli: `add "Hello World"`

## Tuşlar

| Tuş | İş |
|---|---|
| `↑↓` / `j k` + `Enter` | ana ekrandaki menüden eylem seçme |
| `Ctrl+P` | script listesini aç/kapat |
| `↑↓` / `j k` | script listesinde gezinme |
| `Tab` | seçiliyi varsayılan argümanlarla anında çalıştır |
| `Enter` | seçili eylemi çalıştır / argüman girişine geç |
| `Ctrl+U` / `Ctrl+W` | satır temizle / kelime sil |
| `Ctrl+R` | son işi tekrar çalıştır |
| `Ctrl+C` | çalışan işi durdur (yoksa çık) |
| `Backspace` | biten iş görünümünden ana ekrana dön |
| `PgUp/PgDn, ↑↓, Home/End` | çıktı kaydırma |
| `q` | çıkış (palet kapalıyken) |

Çıktı: stdout beyaz, stderr kırmızı. Uzun satırlar kırpılır (`…`).

## CLI modları

```
sr                           TUI
sr -l, --list                script listesi
sr -d, --dir                 script klasör yolunu yaz
sr -o, --open                Gezgin'de aç
sr run python/hello.py a b  TUI olmadan doğrudan çalıştır (exit code döner)
```

## Kurulum ve indirme

GitHub **Releases** bölümünde Windows, Linux ve macOS için hazır ikililer bulunur:

- Windows x64: `sr-windows-x86_64.exe`
- Linux x64: `sr-linux-x86_64`
- Linux ARM64: `sr-linux-aarch64`
- macOS Intel: `sr-macos-x86_64`
- macOS Apple Silicon: `sr-macos-aarch64`

Windows'ta `.exe` dosyasını indirip istediğiniz klasöre taşıyın. Örnek:

```powershell
New-Item -ItemType Directory -Force "$env:LOCALAPPDATA\Programs\sr"
Move-Item "$env:USERPROFILE\Downloads\sr-windows-x86_64.exe" `
  "$env:LOCALAPPDATA\Programs\sr\sr.exe"
& "$env:LOCALAPPDATA\Programs\sr\sr.exe" --help
```

Linux x64 örneği (ARM64 için dosya adını `sr-linux-aarch64` yapın):

```sh
curl -L https://github.com/halil07/scriptrunner/releases/latest/download/sr-linux-x86_64 -o sr
chmod +x sr
./sr --help
sudo install -m 755 sr /usr/local/bin/sr
```

macOS için işlem aynıdır; işlemci mimarinize göre `sr-macos-x86_64`
veya `sr-macos-aarch64` dosyasını indirin. Örnek (Apple Silicon):

```sh
curl -L https://github.com/halil07/scriptrunner/releases/latest/download/sr-macos-aarch64 -o sr
chmod +x sr
./sr --help
```

Uygulamanın kendisi için Rust kurmanız gerekmez. Script çalıştırmak için ilgili
Python, Node.js veya Rust (`rustc`) yorumlayıcısı PATH'te olmalı ya da
`config.toml` içinde yolu belirtilmelidir. İlk açılışta bu araçlar denetlenir.

Yeni bir GitHub Release yayımlamak için `v` ile başlayan bir sürüm etiketi
gönderin (ör. `v0.1.0`). GitHub Actions testleri çalıştırır, Windows/Linux/macOS
ikili dosyalarını derler ve doğrudan Release'e ekler.
Komut ve uygulama içi kullanım yardımı için `sr --help` komutunu
kullanın veya TUI ana ekranından **Yardım** seçeneğini açın.

## Yorumlayıcı ayarı

PATH'te değilse `config.toml`:

```toml
[interpreters]
python = "C:\\Python314\\python.exe"
node   = "C:\\Program Files\\nodejs\\node.exe"
rustc  = "C:\\Users\\<user>\\.cargo\\bin\\rustc.exe"
```

## Geliştirme (bu makinede)

### Kurulu araçlar
- **Rust**: rustup + `stable-x86_64-pc-windows-gnu` (VS Build Tools gerekmez) → `%USERPROFILE%\.cargo\bin`
- **MinGW linkeri**: WinLibs (winget) → `...\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_*\mingw64\bin`
- İkisi de kullanıcı PATH'inde (registry). Yeni açılan terminal bunları görür; eski bir kabukta PATH'i elle ekleyebilirsin:

```powershell
$mingw = "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin"
$env:PATH = "$mingw;$env:USERPROFILE\.cargo\bin;$env:PATH"
```

### Derleme / çalıştırma / test

```powershell
cd scriptrunnerPath

cargo build            # hızlı deneme derlemesi: target\debug\sr.exe
cargo build --release  # dağıtım derlemesi: target\release\sr.exe
cargo test             # birim testler (util, discovery)
cargo run              # TUI'ı development'tan başlatır (Ctrl+P'yi dene)
```

Release derlemesini terminalinden her yerden çalıştırmak için:

```powershell
Copy-Item target\release\sr.exe "$env:LOCALAPPDATA\Microsoft\WindowsApps\" -Force
```

### İzole development config'i

Gerçek `%APPDATA%\scriptrunner` yerine ayrı bir klasörle dene:

```powershell
$env:SCRIPTRUNNER_CONFIG = "$PWD\.devdata"
cargo run                # scriptlerini .devdata\scripts'e bırak
$env:SCRIPTRUNNER_CONFIG = $null   # devden çıkarken sıfırla
```

TUI'sız hızlı çıktı için:

```powershell
cargo run -- --list
cargo run -- run python/hello.py a "b c"
cargo run -- run rust/hello.rs        # rustc cache'e derler (derleme yalnızca bir kere)
cargo run -- run args-demo -v
```

### Bilinen geliştirme tuhaflıkları

- **`cargo` bulunamıyor (PATH eski)**: rustup/WinLibs kurulumundan *önce* açılmış PowerShell/CMD pencereleri eski PATH taşır. Yeni pencere aç, ya da mevcut pencereyi güncelle:
  ```powershell
  $env:PATH = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
  ```
  Windows Terminal arka planda çalışıyorsa tümüyle kapatıp yeniden aç. (Cmder kullanıyorsan ana penceresini yeniden başlat.)

- **WDAC (Smart App Control)**: bu makinede imzasız süreç zincirinin doğrudan ürettiği exe'yi engeller (os error 4551). Bu yüzden Rust derleme aşaması `cmd /c rustc …` ile sarmalanır (`runner.rs`, `build_stages`).
- **dlltool / as bulunamadı** hatası görürsen yukarıdaki `$env:PATH` satırını unutma.
- TUI çıktısını görmek için gerçek bir terminal gerek (Windows Terminal); log pipe'ında alternatif ekran davranışı yok.

Sınırlar: script stdin beklerse EOF alır (interaktif değildir); ANSI renk çıktısı şimdilik düz metin; aynı anda tek iş.
