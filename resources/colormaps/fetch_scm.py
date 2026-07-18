#!/usr/bin/env python3
"""
fetch_scm.py

Downloads Fabio Crameri's "Scientific colour maps" package from Zenodo,
extracts every .cpt (GMT colour palette table) file, and saves each as

    ./scm/<cmapname>_<type>.cpt

where <type> is one of: sequential, divergent, cyclic, other
("other" covers multi-sequential and categorical palettes, plus
 anything not in the classification table below).

Source: Crameri, F. (2018). Scientific colour maps. Zenodo.
        https://doi.org/10.5281/zenodo.1243862
Release used: v8.0.1, Zenodo record 8409685
        https://zenodo.org/records/8409685

Just run it, no arguments needed:
    python fetch_scm.py
"""

import shutil
import sys
import tempfile
import urllib.request
import zipfile
from pathlib import Path

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------

ZIP_URL = (
    "https://zenodo.org/records/8409685/files/ScientificColourMaps8.zip?download=1"
)
OUTPUT_DIR = Path("./scm")

# ---------------------------------------------------------------------------
# Colour map classification
#
# The Zenodo archive doesn't tag files by type itself, so this table is
# built from Crameri's own documentation (fabiocrameri.ch/colourmaps,
# the Scientific colour maps user guide, and the misuse-of-colour paper).
# "other" is used for multi-sequential and categorical palettes, and as
# the fallback for anything not listed (e.g. new maps added in a future
# release -- check the warning printed at the end and update this table
# if needed).
# ---------------------------------------------------------------------------

SEQUENTIAL = {
    "acton",
    "bamako",
    "batlow",
    "batlowK",
    "batlowW",
    "bilbao",
    "buda",
    "davos",
    "devon",
    "grayC",
    "hawaii",
    "imola",
    "lajolla",
    "lapaz",
    "nuuk",
    "oslo",
    "tokyo",
    "turku",
}

DIVERGENT = {
    "bam",
    "berlin",
    "broc",
    "cork",
    "lisbon",
    "managua",
    "roma",
    "tofino",
    "vanimo",
    "vik",
}

CYCLIC = {
    "bamO",
    "brocO",
    "corkO",
    "romaO",
    "vikO",
}

# Multi-sequential and categorical palettes -> "other"
OTHER = {
    "oleron",
    "bukavu",
    "fes",  # multi-sequential
    "batlowS",  # categorical
}

_CLASSIFICATION = {}
_CLASSIFICATION.update({n: "sequential" for n in SEQUENTIAL})
_CLASSIFICATION.update({n: "divergent" for n in DIVERGENT})
_CLASSIFICATION.update({n: "cyclic" for n in CYCLIC})
_CLASSIFICATION.update({n: "other" for n in OTHER})

# Case-insensitive lookup
_CLASSIFICATION_LOWER = {k.lower(): v for k, v in _CLASSIFICATION.items()}


def classify(cmap_name: str):
    """Return (type, matched_exactly). type falls back to 'other'."""
    key = cmap_name.lower()
    if key in _CLASSIFICATION_LOWER:
        return _CLASSIFICATION_LOWER[key], True

    # Heuristic fallback for maps not yet in the table above:
    # "*O" suffix marks the cyclic variant of a diverging map,
    # "*S" suffix marks a categorical (discrete) variant -> "other".
    if key.endswith("o") and key[:-1] in _CLASSIFICATION_LOWER:
        return "cyclic", False
    if key.endswith("s") and key[:-1] in _CLASSIFICATION_LOWER:
        return "other", False

    return "other", False


# ---------------------------------------------------------------------------
# Download / extract
# ---------------------------------------------------------------------------


def download(url: str, dest: Path) -> None:
    print(f"Downloading {url}")
    req = urllib.request.Request(url, headers={"User-Agent": "fetch_scm.py"})
    with urllib.request.urlopen(req) as response, open(dest, "wb") as out_file:
        total = response.getheader("Content-Length")
        total = int(total) if total else None
        downloaded = 0
        chunk_size = 1024 * 256
        while True:
            chunk = response.read(chunk_size)
            if not chunk:
                break
            out_file.write(chunk)
            downloaded += len(chunk)
            if total:
                pct = downloaded / total * 100
                print(
                    f"\r  {downloaded / 1e6:8.1f} / {total / 1e6:.1f} MB ({pct:5.1f}%)",
                    end="",
                    flush=True,
                )
            else:
                print(f"\r  {downloaded / 1e6:8.1f} MB", end="", flush=True)
    print()


def extract_cpt_files(zip_path: Path, extract_dir: Path):
    print(f"Extracting .cpt files from {zip_path.name}")
    cpt_paths = []
    with zipfile.ZipFile(zip_path) as zf:
        for member in zf.namelist():
            if member.lower().endswith(".cpt"):
                target = extract_dir / Path(member).name
                with zf.open(member) as src, open(target, "wb") as dst:
                    shutil.copyfileobj(src, dst)
                cpt_paths.append(target)
    return cpt_paths


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main() -> int:
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory() as tmp:
        tmp_dir = Path(tmp)
        zip_path = tmp_dir / "ScientificColourMaps8.zip"

        try:
            download(ZIP_URL, zip_path)
        except Exception as exc:
            print(f"Download failed: {exc}", file=sys.stderr)
            return 1

        extract_dir = tmp_dir / "extracted"
        extract_dir.mkdir()
        cpt_files = extract_cpt_files(zip_path, extract_dir)

        if not cpt_files:
            print("No .cpt files found in the archive.", file=sys.stderr)
            return 1

        print(f"Found {len(cpt_files)} .cpt files. Renaming...")
        unclassified = []
        written = 0
        seen = set()

        for cpt_path in sorted(cpt_files):
            cmap_name = cpt_path.stem  # filename without .cpt
            cmap_type, exact = classify(cmap_name)
            if not exact:
                unclassified.append(cmap_name)

            new_name = f"{cmap_name}_{cmap_type}.cpt"
            if new_name in seen:
                new_name = f"{cmap_name}_{cmap_type}_dup.cpt"
            seen.add(new_name)

            shutil.copy(cpt_path, OUTPUT_DIR / new_name)
            written += 1

        print(f"Wrote {written} files to {OUTPUT_DIR.resolve()}")
        if unclassified:
            print(
                "\nWarning: these colour maps were not in the "
                "classification table and were guessed/defaulted "
                "to 'other'. Check them and update the table if needed:"
            )
            for name in sorted(set(unclassified)):
                print(f"  - {name}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
