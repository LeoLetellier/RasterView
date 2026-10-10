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
- [ ] Add sqrt, log, gamma, histo eq conversions, dB valid only for positive data
- [ ] Add symmetric value to cmap, with phase should be -pi;+pi


## ❌ Out of Scope

* Multi-raster view, layer composition: use QGIS instead
* Projection and background layer maps: only display rasters as a pixel grid
* InSAR specific vizualisation: use InSARViz instead


## 🔧 Installation

**NEW**: GDAL dependencies is now handled by pixi, independently from any system installation.

### Rust

This application builds using the Rust compiler. If you don't have rust installed, check the [Rust website](https://rust-lang.org/tools/install/).

### Pixi

This application builds against GDAL using the pixi package manager, fetching source from conda-forge. If you don't have pixi installed, check the [Pixi Website](https://pixi.prefix.dev/latest/installation/).

### Fetch the repository

```shell
git clone https://github.com/LeoLetellier/RasterView
cd RasterView
```

### Development

```shell
pixi run debug-run
```

### Permanent installation for the current machine

```shell
pixi run release-native
```

This binary bundle GDAL, and is compiled with optimization relative to your current hardware.

The binary will be located in: `./target/release-native`.

### **Advanced**: Permanent installation for the current machine with local GDAL

This works only if GDAL was correctly installed on your system and with a compatible version. You can try with or without the `bindgen` feature.

```shell
cargo remove gdal-sys gdal-src
cargo add gdal -F bindgen
cargo build --release
```

The binary will be located in: `./target/release`.

### Distributed installations / Actions

```shell
pixi run realease-static
```

This binary bundle GDAL, and is compiled with optimization for your OS.

The binary will be located in: `./target/release-static`.
