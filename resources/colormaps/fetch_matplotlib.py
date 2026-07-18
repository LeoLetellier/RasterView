#!/usr/bin/env python3
"""
fetch_matplotlib_cpt.py

Generates GMT-style .cpt (colour palette table) files from Matplotlib's
built-in colormaps -- no download needed, everything is sampled locally
from the matplotlib installation -- and saves each as

    ./matplotlib/<cmapname>_<type>.cpt

where <type> is one of: sequential, divergent, cyclic, other.

Classification follows Matplotlib's own documented colormap categories
(https://matplotlib.org/stable/users/explain/colors/colormaps.html):
"other" covers the miscellaneous group (qualitative/rainbow-like maps
that aren't perceptually uniform sequential, diverging, or cyclic).

Just run it, no arguments needed:
    python fetch_matplotlib_cpt.py
"""

import os
import sys

import numpy as np
import matplotlib as mpl

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------

OUTPUT_DIR = "./matplotlib"
N_STEPS = 256  # number of color steps in each CPT
Z_MIN = 0.0
Z_MAX = 1.0

# ---------------------------------------------------------------------------
# Colour map classification
# (per matplotlib's own colormap reference: Perceptually Uniform Sequential,
#  Sequential, Sequential(2), Diverging, Cyclic, Qualitative/Miscellaneous)
# ---------------------------------------------------------------------------

SEQUENTIAL = {
    "viridis",
    "plasma",
    "inferno",
    "magma",
    "cividis",
    "Greys",
    "Purples",
    "Blues",
    "Greens",
    "Oranges",
    "Reds",
    "YlOrBr",
    "YlOrRd",
    "OrRd",
    "PuRd",
    "RdPu",
    "BuPu",
    "GnBu",
    "PuBu",
    "YlGnBu",
    "PuBuGn",
    "BuGn",
    "YlGn",
    "gray",
    "bone",
    "pink",
    "spring",
    "summer",
    "autumn",
    "winter",
    "cool",
    "Wistia",
    "hot",
    "afmhot",
    "gist_heat",
    "copper",
}

DIVERGENT = {
    "PiYG",
    "PRGn",
    "BrBG",
    "PuOr",
    "RdGy",
    "RdBu",
    "RdYlBu",
    "RdYlGn",
    "Spectral",
    "coolwarm",
    "bwr",
    "seismic",
    "berlin",
    "managua",
    "vanimo",
}

CYCLIC = {
    "twilight",
    "twilight_shifted",
    "hsv",
}

OTHER = {
    "flag",
    "prism",
    "ocean",
    "gist_earth",
    "terrain",
    "gist_stern",
    "gnuplot",
    "gnuplot2",
    "CMRmap",
    "cubehelix",
    "brg",
    "gist_rainbow",
    "rainbow",
    "jet",
    "turbo",
    "nipy_spectral",
    "gist_ncar",
}

_CLASSIFICATION = {}
_CLASSIFICATION.update({n: "sequential" for n in SEQUENTIAL})
_CLASSIFICATION.update({n: "divergent" for n in DIVERGENT})
_CLASSIFICATION.update({n: "cyclic" for n in CYCLIC})
_CLASSIFICATION.update({n: "other" for n in OTHER})

ALL_MAPS = sorted(_CLASSIFICATION)


# ---------------------------------------------------------------------------
# CPT generation
# ---------------------------------------------------------------------------


def write_cpt(
    dest: str,
    cmap_name: str,
    n: int = N_STEPS,
    zmin: float = Z_MIN,
    zmax: float = Z_MAX,
) -> None:
    """Sample a matplotlib colormap into n steps and write it as a GMT CPT."""
    cmap = mpl.colormaps[cmap_name]

    dz = (zmax - zmin) / n
    ts = zmin + np.arange(n) * dz
    rgb = cmap((ts - zmin) / (zmax - zmin))[:, :3]  # Nx3 in [0, 1]
    rgb255 = np.clip(np.round(rgb * 255), 0, 255).astype(int)

    with open(dest, "w", encoding="utf-8") as f:
        f.write(f"# CPT generated from matplotlib cmap={cmap_name}\n")
        for i in range(n):
            z1 = zmin + i * dz
            z2 = zmin + (i + 1) * dz
            r1, g1, b1 = rgb255[i]
            r2, g2, b2 = rgb255[i + 1] if i + 1 < n else rgb255[i]
            # CPT line format: z1  R1 G1 B1   z2  R2 G2 B2
            f.write(
                f"{z1:.6f} {r1:3d} {g1:3d} {b1:3d} {z2:.6f} {r2:3d} {g2:3d} {b2:3d}\n"
            )
        f.write("B 0 0 0\n")
        f.write("F 255 255 255\n")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main() -> int:
    os.makedirs(OUTPUT_DIR, exist_ok=True)

    print(f"Generating {len(ALL_MAPS)} matplotlib colour maps as .cpt files ...")

    written = 0
    failed = []

    for i, name in enumerate(ALL_MAPS, start=1):
        cmap_type = _CLASSIFICATION[name]
        dest = os.path.join(OUTPUT_DIR, f"{name}_{cmap_type}.cpt")

        print(f"  [{i:2d}/{len(ALL_MAPS)}] {name:<16s} -> {os.path.basename(dest)}")
        try:
            write_cpt(dest, name)
        except Exception as exc:
            print(f"      failed: {exc}", file=sys.stderr)
            failed.append(name)
            continue

        written += 1

    print(f"\nWrote {written}/{len(ALL_MAPS)} files to {os.path.abspath(OUTPUT_DIR)}")
    if failed:
        print("Failed to generate:", ", ".join(failed), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
