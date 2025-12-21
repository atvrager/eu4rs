import argparse
import sys
import zipfile
import json
import io
from pathlib import Path


def inspect_file(path: Path, num_samples: int = 3):
    print(f"inspecting {path}...")

    if path.suffix == ".zip":
        with zipfile.ZipFile(path, "r") as z:
            print(f"Archive contains {len(z.namelist())} files.")
            for name in sorted(z.namelist())[:3]:
                print(f"  - {name} ({z.getinfo(name).file_size} bytes)")

            # Read first file
            first = z.namelist()[0]
            print(f"\nReading samples from {first}:")
            with z.open(first) as f:
                # ZipFile.open returns binary, need to decode
                text_io = io.TextIOWrapper(f, encoding="utf-8")
                for i, line in enumerate(text_io):
                    if i >= num_samples:
                        break
                    print(f"[{i}] {line.strip()[:200]}...")  # Truncate

    elif path.suffix == ".jsonl":
        print(f"Reading samples from {path}:")
        with open(path, "r", encoding="utf-8") as f:
            for i, line in enumerate(f):
                if i >= num_samples:
                    break
                print(f"[{i}] {line.strip()[:200]}...")


def main():
    parser = argparse.ArgumentParser(description="Inspect EU4 training data")
    parser.add_argument("path", type=Path, help="Path to .zip or .jsonl file")
    parser.add_argument(
        "-n", "--num", type=int, default=3, help="Number of samples to print"
    )
    args = parser.parse_args()

    if not args.path.exists():
        print(f"Error: {args.path} not found")
        sys.exit(1)

    inspect_file(args.path, args.num)


if __name__ == "__main__":
    main()
