# 🛰️ RasterView 🌏

[![License](https://img.shields.io/badge/license-AGPL-blue.svg)](LICENSE)

🗺️ Tiny viewer for large GDAL rasters written in rust 🦀


## 📖 About


## ✨ Features

* **GDAL raster support**: open `GTiff`, `ENVI`, `ROIPAC` files and more...
* **Raster information**: display the metadata of your raster.
* **Fast display**: compiled rendering operations and tile caching allows seemless raster exploration. For very large rasters, consider creating overviews using GDAL (`gdaladdo` command) before opening the raster in the application. 
* [**Standard color palettes**](./resources/colormaps/README.md): use perceptually perceptive colormaps for intuitive color rendering. ``#TODO``
* **Cube exploration**: display per pixel profiles along all bands, usefull to explore time series. ``#TODO``

## 🔧 Installation

You need to have a working [GDAL](https://gdal.org/en/stable/) installation on your system (`libgdal`).

This application builds using the Rust compiler. If you don't have rust installed, check the [Rust website](https://rust-lang.org/tools/install/).

```shell
git clone https://github.com/LeoLetellier/RasterView
cd RasterView
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
