import sys
import time

print("Hello from Python", sys.version.split()[0])
for a in sys.argv[1:]:
    print("  argüman:", a)

print("ilerleme...")
for i in range(3):
    print(f"  adım {i+1}/3")
    time.sleep(0.3)
print("✔ bitti (exit 0)")
