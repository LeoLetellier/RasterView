#!/usr/bin/env python3
import numpy as np
import matplotlib as mpl
import os

# Common default in Matplotlib
CMAP_NAMES_SEQUENTIAL = [
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
]
CMAP_NAMES_DIVERGENT = [
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
]
CMAP_NAMES_CYCLIC = ["twilight", "twilight_shifted", "hsv"]
CMAP_NAMES_OTHER = [
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
]
N = 256  # number of color steps in the CPT


def write_cpt(filename, cmap_name, n=256, zmin=0.0, zmax=1.0):
    cmap = mpl.colormaps[cmap_name]
    # CPT uses fixed intervals: [z, z+dz]
    dz = (zmax - zmin) / n

    # Sample RGBA from the colormap
    ts = zmin + (np.arange(n) + 0.0) * dz
    rgb = cmap((ts - zmin) / (zmax - zmin))[:, :3]  # Nx3 in [0,1]

    # Convert to 0-255 integers
    rgb255 = np.clip(np.round(rgb * 255), 0, 255).astype(int)

    with open(filename, "w", encoding="utf-8") as f:
        # Optional header
        f.write(f"# CPT generated from matplotlib cmap={cmap_name}\n")

        for i in range(n):
            z1 = zmin + i * dz
            z2 = zmin + (i + 1) * dz
            r1, g1, b1 = rgb255[i]
            r2, g2, b2 = rgb255[min(i + 1, n - 1)] if i + 1 < n else rgb255[i]

            # CPT line format:
            # z1  R1 G1 B1   z2  R2 G2 B2
            f.write(
                f"{z1:.6f} {r1:3d} {g1:3d} {b1:3d} {z2:.6f} {r2:3d} {g2:3d} {b2:3d}\n"
            )

        # Optional background/foreground lines (comment out if you don't want them)
        f.write("B 0 0 0\n")
        f.write("F 255 255 255\n")


def main():
    root = "./matplotlib/"
    if not os.path.exists(root):
        os.mkdir(root)
    for cmap in CMAP_NAMES_SEQUENTIAL:
        out = f"{root}/{cmap}_sequential.cpt"
        write_cpt(out, cmap, n=N, zmin=0.0, zmax=1.0)
        print(f"Wrote {out}")
    for cmap in CMAP_NAMES_DIVERGENT:
        out = f"{root}/{cmap}_divergent.cpt"
        write_cpt(out, cmap, n=N, zmin=0.0, zmax=1.0)
        print(f"Wrote {out}")
    for cmap in CMAP_NAMES_CYCLIC:
        out = f"{root}/{cmap}_cyclic.cpt"
        write_cpt(out, cmap, n=N, zmin=0.0, zmax=1.0)
        print(f"Wrote {out}")
    for cmap in CMAP_NAMES_OTHER:
        out = f"{root}/{cmap}_other.cpt"
        write_cpt(out, cmap, n=N, zmin=0.0, zmax=1.0)
        print(f"Wrote {out}")


if __name__ == "__main__":
    main()
