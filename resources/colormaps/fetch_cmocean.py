#!/usr/bin/env python3
"""
fetch_cmocean_cpt.py

Downloads Kristen Thyng's cmocean colour palette tables (the "cmocean/*"
family only -- as redistributed in .cpt form by GMT) and saves each as

    ./cmocean/<cmapname>_<type>.cpt

where <type> is one of: sequential, divergent, cyclic, other.

Source: GenericMappingTools/gmt, share/cpt/cmocean/*.cpt
        https://github.com/GenericMappingTools/gmt
Original project: https://matplotlib.org/cmocean/ (Thyng et al., 2016)

Classification is derived from GMT's own master-CPT description table
(src/gmt_cpt_masters.h), which spells the type out directly for most
cmocean maps ("divergent colormap", "cyclic colormap"); the remaining
ones (plain "Perceptually uniform colormap") are sequential, except
'topo' which -- like land/sea relief maps -- is a two-part hinge map
and therefore classified as divergent.

Just run it, no arguments needed:
    python fetch_cmocean_cpt.py
"""

import sys
import urllib.request
from pathlib import Path

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------

# Pinned to the 6.5.0 release tag rather than "master" so the file list and
# classification below stay in sync with what's actually downloaded.
GMT_REF = "6.5.0"
RAW_BASE = f"https://raw.githubusercontent.com/GenericMappingTools/gmt/{GMT_REF}/share/cpt/cmocean"

OUTPUT_DIR = Path("./cmocean")

# ---------------------------------------------------------------------------
# Colour map classification (derived from src/gmt_cpt_masters.h)
# ---------------------------------------------------------------------------

SEQUENTIAL = {
    "algae",
    "amp",
    "deep",
    "dense",
    "gray",
    "haline",
    "ice",
    "matter",
    "rain",
    "solar",
    "speed",
    "tempo",
    "thermal",
    "turbid",
}

DIVERGENT = {
    "balance",
    "curl",
    "delta",
    "diff",
    "oxy",
    "tarn",
    "topo",
}

CYCLIC = {
    "phase",
}

OTHER: set[str] = set()  # no explicitly categorical cmocean maps

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
    req = urllib.request.Request(url, headers={"User-Agent": "fetch_cmocean_cpt.py"})
    with urllib.request.urlopen(req) as response:
        return response.read()


def main() -> int:
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    print(
        f"Fetching {len(ALL_MAPS)} cmocean colour maps from "
        f"GenericMappingTools/gmt@{GMT_REF} ..."
    )

    written = 0
    failed = []

    for i, name in enumerate(ALL_MAPS, start=1):
        cmap_type = _CLASSIFICATION[name]
        url = f"{RAW_BASE}/{name}.cpt"
        dest = OUTPUT_DIR / f"{name}_{cmap_type}.cpt"

        print(f"  [{i:2d}/{len(ALL_MAPS)}] {name:<10s} -> {dest.name}")
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
