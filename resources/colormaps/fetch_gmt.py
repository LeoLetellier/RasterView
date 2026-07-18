#!/usr/bin/env python3
"""
fetch_gmt_cpt.py

Downloads the native GMT master colour palette tables (the "gmt/*" family
only -- NOT SCM, cmocean, matlab, matplotlib, cpt-city, etc.) directly from
the GMT GitHub repository, and saves each as

    ./gmt/<cmapname>_<type>.cpt

where <type> is one of: sequential, divergent, cyclic, other.

Source: GenericMappingTools/gmt, share/cpt/gmt/*.cpt
        https://github.com/GenericMappingTools/gmt

Classification is derived from GMT's own master-CPT description table
(src/gmt_cpt_masters.h): maps whose description says "Cyclic" -> cyclic;
maps flagged with a hinge [H] (two ramps joined at a critical value, e.g.
sea level) or a soft/hard symmetric hinge [S] (e.g. blue-white-red polar
scales) -> divergent; explicitly categorical/qualitative palettes ->
other; everything else -> sequential.

Just run it, no arguments needed:
    python fetch_gmt_cpt.py
"""

import shutil
import sys
import urllib.request
from pathlib import Path

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------

# Pinned to the 6.5.0 release tag rather than "master" so the file list and
# classification below stay in sync with what's actually downloaded. Bump
# this if you want newer GMT master CPTs (check gmt_cpt_masters.h again).
GMT_REF = "6.5.0"
RAW_BASE = f"https://raw.githubusercontent.com/GenericMappingTools/gmt/{GMT_REF}/share/cpt/gmt"

OUTPUT_DIR = Path("./gmt")

# ---------------------------------------------------------------------------
# Colour map classification (derived from src/gmt_cpt_masters.h)
# ---------------------------------------------------------------------------

SEQUENTIAL = {
    "abyss", "bathy", "dem2", "dem3", "drywet", "gebco", "gray", "haxby",
    "ibcso", "nighttime", "ocean", "rainbow", "rust2silver", "seafloor",
    "seis",
}

DIVERGENT = {
    "earth", "etopo1", "geo", "globe", "mag", "no_green", "red2green",
    "relief", "sealand", "split", "srtm", "terra", "topo", "world",
}

CYCLIC = {
    "cyclic",
}

OTHER = {
    "categorical", "paired", "wysiwyg",
}

_CLASSIFICATION = {}
_CLASSIFICATION.update({n: "sequential" for n in SEQUENTIAL})
_CLASSIFICATION.update({n: "divergent" for n in DIVERGENT})
_CLASSIFICATION.update({n: "cyclic" for n in CYCLIC})
_CLASSIFICATION.update({n: "other" for n in OTHER})

ALL_MAPS = sorted(_CLASSIFICATION)


# ---------------------------------------------------------------------------
# Download
# ---------------------------------------------------------------------------

def download(url: str) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": "fetch_gmt_cpt.py"})
    with urllib.request.urlopen(req) as response:
        return response.read()


def main() -> int:
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    print(f"Fetching {len(ALL_MAPS)} native GMT colour maps from "
          f"GenericMappingTools/gmt@{GMT_REF} ...")

    written = 0
    failed = []

    for i, name in enumerate(ALL_MAPS, start=1):
        cmap_type = _CLASSIFICATION[name]
        url = f"{RAW_BASE}/{name}.cpt"
        dest = OUTPUT_DIR / f"{name}_{cmap_type}.cpt"

        print(f"  [{i:2d}/{len(ALL_MAPS)}] {name:<14s} -> {dest.name}")
        try:
            data = download(url)
        except Exception as exc:
            print(f"      failed: {exc}", file=sys.stderr)
            failed.append(name)
            continue

        dest.write_bytes(data)
        written += 1

    print(f"\nWrote {written}/{len(ALL_MAPS)} files to {OUTPUT_DIR.resolve()}")
    if failed:
        print("Failed to download:", ", ".join(failed), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
