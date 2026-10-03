# 🛰️ RasterView 🌏

[![License](https://img.shields.io/badge/license-AGPL-blue.svg)](LICENSE)

🗺️ Tiny viewer for large GDAL rasters written in rust 🦀



## ✨ Features

* **GDAL raster support**: open `GTiff`, `ENVI`, `ROIPAC` files and more... If it is a raster it will open.
* **Raster information**: display the metadata of your raster. Handy.
* **Fast display**: compiled rendering operations and tile caching allows seemless raster exploration. For very large rasters, consider creating overviews using GDAL (`gdaladdo` command) before opening the raster in the application, or convert the file to `COG` format. ``#WIP``
* [**Standard color palettes**](./resources/colormaps/README.md): use perceptually perceptive colormaps for intuitive color rendering. ``#WIP``
* **Cube exploration**: display per pixel profiles along all bands, usefull to explore time series. ``#TODO``
* **Raw rasters**: simulate rasters from 2D arrays on disk, given dimensions. ``#WIP``

## 📖 About


## 📌 TODO

- [ ] separate loaded raster from view parameters
- [x] allow alternatively for loading 2d arrays with user given size and format type
- [ ] Fix view parameters with user inputs
- [ ] Fix value range
- [ ] Fix invert cmap for min max colors
- [ ] Clean parameters export to user
- [ ] Add keybindings
- [ ] Add a decrease resolution parameter to allow load data at a resolution coarser than actually could have displayed
- [ ] Add extended loading strategy, by decreasing loading charge by checking panning/zooming speed and then loading coarser tiles until the view stabilize, and drop all in middle
- [x] Invert y-axis display
- [ ] Add resolution to main dataset metadata display
- [ ] Show all file system info
- [ ] Implement loading spinner and stats waiting
- [ ] Add clap cli cmd
- [ ] Organize parameters panel
- [ ] Make button and forms wrapper
- [ ] Workaround for loading complex dtype (Rust gdal limitation)
- [ ] Save screenshot of current view (view + colorbar, need own module)
- [ ] Add index view mode (multi-band raster)
- [ ] Switch cache call to quadtree to enable coarse filling


## ❌ Out of Scope

* Multi-raster view, layer composition: use QGIS instead
* Projection and background layer maps: only display rasters as a pixel grid
* InSAR specific vizualisation: use InSARViz instead


## 🔧 Installation

You need to have a working [GDAL](https://gdal.org/en/stable/) development installation on your system (`libgdal`).

This application builds using the Rust compiler. If you don't have rust installed, check the [Rust website](https://rust-lang.org/tools/install/).

### Fetch the repository

```shell
git clone https://github.com/LeoLetellier/RasterView
cd RasterView
```

### Local build

```shell
cargo build --release
```

The binary will be located in `./target/release/`.

If you have issues with the local GDAL installation, try using GDAL in a conda environment instead, such as:

```shell
conda create -n gdal311 -c conda-forge gdal=3.11
conda activate gdal311
export GDALHOME=$CONDA_PREFIX
export PKG_CONFIG_PATH="$CONDA_PREFIX/lib/pkgconfig:$PKG_CONFIG_PATH"
cargo clean
```

### Permanent installation

```shell
cargo install --path .
```
