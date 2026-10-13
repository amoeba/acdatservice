#!/usr/bin/env python3
"""
Upload flattened extracted files to R2.

Usage:
  python3 upload_r2.py <export-dir> <dat-name>

Flattens the asheron-rs export structure (strips Texture/, CharGen/, SpellTable/
subdirectories) and uploads every .bin to:
  treestats-acdats-extracted/<dat-name>/<basename>

Uses parallel HTTP PUT for speed.
"""

import subprocess
import sys
import os
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

MAX_WORKERS = int(os.environ.get("R2_UPLOAD_WORKERS", "16"))

def upload_file(wrangler_path: str, bucket: str, dat_name: str, file_path: Path) -> None:
    """Upload a single .bin file to R2."""
    key = f"{dat_name}/{file_path.name}"
    cmd = [
        wrangler_path, "r2", "object", "put",
        f"{bucket}/{key}",
        "--file", str(file_path),
        "--remote",
    ]
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
    if result.returncode != 0:
        print(f"ERROR: {key}: {result.stderr.strip() or result.stdout.strip()}")
    else:
        print(f"OK: {key}")

def main():
    if len(sys.argv) != 3:
        print("Usage: upload_r2.py <export-dir> <dat-name>")
        sys.exit(1)

    export_dir = Path(sys.argv[1])
    dat_name = sys.argv[2]
    bucket = "treestats-acdats-extracted"

    # Find wrangler
    wrangler = subprocess.run(["which", "npx"], capture_output=True, text=True)
    if wrangler.returncode != 0:
        wrangler_path = "npx"
    else:
        wrangler_path = "npx"

    # Collect all .bin files recursively (flattening subdirectories)
    bin_files = sorted(export_dir.rglob("*.bin"))
    total = len(bin_files)
    print(f"Uploading {total} files to {bucket}/{dat_name}/ using {MAX_WORKERS} workers...")

    done = 0
    errors = 0

    with ThreadPoolExecutor(max_workers=MAX_WORKERS) as pool:
        futures = {
            pool.submit(upload_file, wrangler_path, bucket, dat_name, f): f
            for f in bin_files
        }
        for future in as_completed(futures):
            done += 1
            if done % 500 == 0:
                print(f"Progress: {done}/{total}")

    print(f"\nDone: {done}/{total} files uploaded")

if __name__ == "__main__":
    main()