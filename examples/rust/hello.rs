// Tek dosyalik Rust scripti - scriptrunner bunu rustc ile derler ve cache'ler.
use std::env;

fn main() {
    for a in env::args().skip(1) {
        println!("Hello {} from Rust", a);
        println!("  argüman: {}", a);
    }
    println!("✔ bitti (exit 0)");
}
