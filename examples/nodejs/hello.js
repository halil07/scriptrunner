console.log("Hello from Node.js", process.version);
const args = process.argv.slice(2);
args.forEach((a, i) => console.log(`  argüman ${i}:`, a));
console.error("bu satır stderr'e gitti (kırmızı görünür)");
console.log("✔ bitti (exit 0)");
