import argparse

p = argparse.ArgumentParser(description="manifest ornegi")
p.add_argument("-v", "--verbose", action="store_true", help="ayrintili mod")
p.add_argument("--name", default="dunya", help="selamlanacak isim")
a = p.parse_args()

if a.verbose:
    print("verbose mod açık")
print(f"Merhaba, {a.name}!")
