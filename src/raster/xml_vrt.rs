use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use gdal::errors::Result;
use gdal::vsi::create_mem_file;

use crate::raster::GeoTransform;
use gdal::config::set_config_option;

#[derive(Debug, Default, PartialEq)]
pub(crate) struct VrtParameters {
    pub(crate) array_path: PathBuf,
    pub(crate) size: (usize, usize),
    pub(crate) band_nb: usize,
    pub(crate) image_offset: usize,
    pub(crate) pixel_offset: usize,
    pub(crate) interleave: VrtInterleave,
    pub(crate) byte_order: VrtByteOrder,
    pub(crate) data_type: VrtDataType,
    pub(crate) ndv: Option<f64>,
    pub(crate) crs: Option<String>,
    pub(crate) geotransform: Option<GeoTransform>,
}

#[derive(Debug, Default, PartialEq)]
pub(crate) enum VrtInterleave {
    #[default]
    Bsq,
    Bip,
    Bil,
}

#[derive(Debug, Default, PartialEq)]
pub(crate) enum VrtByteOrder {
    #[default]
    Lsb,
    Msb,
}

impl VrtByteOrder {
    pub(crate) fn as_gdal_str(&self) -> &'static str {
        match self {
            VrtByteOrder::Lsb => "LSB",
            VrtByteOrder::Msb => "MSB",
        }
    }
}

#[derive(Debug, Default, PartialEq)]
pub(crate) enum VrtDataType {
    Byte,
    UInt16,
    Int16,
    UInt32,
    Int32,
    #[default]
    Float32,
    Float64,
    CFloat32,
}

impl VrtDataType {
    fn size_bytes(&self) -> usize {
        match self {
            VrtDataType::Byte => 1,
            VrtDataType::UInt16 | VrtDataType::Int16 => 2,
            VrtDataType::UInt32 | VrtDataType::Int32 | VrtDataType::Float32 => 4,
            VrtDataType::Float64 | VrtDataType::CFloat32 => 8,
        }
    }

    pub(crate) fn as_gdal_str(&self) -> &'static str {
        match self {
            VrtDataType::Byte => "Byte",
            VrtDataType::UInt16 => "UInt16",
            VrtDataType::Int16 => "Int16",
            VrtDataType::UInt32 => "UInt32",
            VrtDataType::Int32 => "Int32",
            VrtDataType::Float32 => "Float32",
            VrtDataType::Float64 => "Float64",
            VrtDataType::CFloat32 => "CFloat32",
        }
    }
}

/// Per-band (ImageOffset, PixelOffset, LineOffset) in bytes, given the
/// interleave scheme. PixelOffset and LineOffset are constant across
/// bands for a given interleave; only ImageOffset shifts per band.
fn band_offsets(p: &VrtParameters, band_index: usize) -> (usize, usize, usize) {
    let (w, h) = p.size;
    let dsize = p.data_type.size_bytes();
    let pixel_offset = if p.pixel_offset == 0 {
        dsize
    } else {
        p.pixel_offset
    };

    match p.interleave {
        VrtInterleave::Bsq => (
            p.image_offset + band_index * w * h * dsize,
            pixel_offset,
            w * dsize,
        ),
        VrtInterleave::Bip => (
            p.image_offset + band_index * dsize,
            p.band_nb * dsize,
            w * p.band_nb * dsize,
        ),
        VrtInterleave::Bil => (
            p.image_offset + band_index * w * dsize,
            pixel_offset,
            p.band_nb * w * dsize,
        ),
    }
}

fn build_vrt_xml(p: &VrtParameters) -> String {
    let (width, height) = p.size;
    let dtype = p.data_type.as_gdal_str();
    let byte_order = p.byte_order.as_gdal_str();
    let path = p.array_path.display();

    let srs_elem = p
        .crs
        .as_ref()
        .map(|crs| format!("\n  <SRS>{crs}</SRS>"))
        .unwrap_or_default();

    let geotransform_elem = p
        .geotransform
        .map(|gt| {
            format!(
                "\n  <GeoTransform>{}, {}, {}, {}, {}, {}</GeoTransform>",
                gt.x_off, gt.x_res, gt.x_rot, gt.y_off, gt.y_rot, gt.y_res,
            )
        })
        .unwrap_or_default();

    let mut bands = String::new();
    for b in 0..p.band_nb {
        let (image_offset, pixel_offset, line_offset) = band_offsets(p, b);
        let ndv_elem = p
            .ndv
            .map(|v| format!("\n    <NoDataValue>{v}</NoDataValue>"))
            .unwrap_or_default();

        bands.push_str(&format!(
            r#"
  <VRTRasterBand dataType="{dtype}" band="{band}" subClass="VRTRawRasterBand">
    <SourceFilename relativeToVRT="0">{path}</SourceFilename>
    <ImageOffset>{image_offset}</ImageOffset>
    <PixelOffset>{pixel_offset}</PixelOffset>
    <LineOffset>{line_offset}</LineOffset>
    <ByteOrder>{byte_order}</ByteOrder>{ndv_elem}
  </VRTRasterBand>"#,
            band = b + 1,
        ));
    }

    format!(
        r#"<VRTDataset rasterXSize="{width}" rasterYSize="{height}">{srs_elem}{geotransform_elem}{bands}
</VRTDataset>"#,
    )
}

static VRT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Builds the VRT and writes it into GDAL's in-memory filesystem, returning
/// a `PathBuf` (e.g. `/vsimem/raw_view_3.vrt`) that behaves like any normal
/// path to `Dataset::open` — so existing code that takes a path can consume
/// a headerless raw array without modification.
pub(crate) fn simulate_vrt_path(p: &VrtParameters) -> Result<PathBuf> {
    let xml = build_vrt_xml(p);
    let id = VRT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let vsi_path = PathBuf::from(format!("/vsimem/raw_view_{id}.vrt"));

    create_mem_file(&vsi_path, xml.into_bytes())?;
    Ok(vsi_path)
}

/// Temporarily allows GDAL's VRTRawRasterBand to dereference sources inside
/// `dir`. Restriction is lifted automatically when the returned guard drops.
pub(crate) fn allow_raw_source(dir: &Path) -> Result<()> {
    set_config_option(
        "GDAL_VRT_RAWRASTERBAND_ALLOWED_SOURCE",
        &dir.to_string_lossy(),
    )
}
