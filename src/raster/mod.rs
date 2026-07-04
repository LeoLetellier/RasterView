use crate::app::prop_section;
use anyhow::Result;
use gdal::Dataset;
use gdal::raster::RasterBand;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(crate) struct RasterHandler {
    pub path: PathBuf,
    pub dataset: gdal::Dataset,
    pub dataset_properties: DatasetProperties,
    pub band_properties: Vec<BandProperties>,
}

impl RasterHandler {
    pub(crate) fn new(path: impl AsRef<Path>) -> Result<Self> {
        let dataset = Dataset::open(&path)?;
        let dataset_properties = DatasetProperties::from_dataset(&dataset);
        let mut band_properties = vec![];
        for b in dataset.rasterbands() {
            band_properties.push(BandProperties::from_rasterband(&b?));
        }

        Ok(Self {
            path: path.as_ref().to_path_buf(),
            dataset,
            dataset_properties,
            band_properties,
        })
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    fn update_dataset(&mut self) -> bool {
        let path = self.path();
        if let Ok(ds) = gdal::Dataset::open(path) {
            self.dataset_properties = DatasetProperties::from_dataset(&ds);
            self.band_properties.clear();
            for b in ds.rasterbands() {
                if let Ok(b) = b {
                    self.band_properties
                        .push(BandProperties::from_rasterband(&b));
                }
            }
            self.dataset = ds;
            return true;
        }
        false
    }

    pub fn raster_size(&self) -> (usize, usize) {
        self.dataset.raster_size()
    }
}

#[derive(Debug)]
pub struct DatasetProperties {
    driver: String,
    size: (usize, usize),
    band_nb: usize,
    projection: String,
    geotransform: Option<[f64; 6]>,
    bbox: Option<[f64; 4]>,
}

impl DatasetProperties {
    fn from_dataset(dataset: &gdal::Dataset) -> Self {
        let driver = dataset.driver().short_name();
        let size = dataset.raster_size();
        let band_nb = dataset.raster_count();
        let projection = dataset.projection();
        let geotransform = dataset.geo_transform().ok();
        let bbox = geotransform.map(|gt| {
            [
                gt[0],
                gt[0] + (size.0 as f64) * gt[1],
                gt[3] + (size.1 as f64) * gt[5],
                gt[3],
            ]
        });
        Self {
            driver,
            size,
            band_nb,
            projection,
            geotransform,
            bbox,
        }
    }

    pub fn ui(&self, ui: &mut egui::Ui) {
        prop_section(ui, None, &[["Driver".to_string(), self.driver.clone()]]);
        prop_section(
            ui,
            Some("Size"),
            &[
                ["x".to_string(), self.size.0.to_string()],
                ["y".to_string(), self.size.1.to_string()],
            ],
        );
        prop_section(
            ui,
            None,
            &[["Band nb".to_string(), self.band_nb.to_string()]],
        );
        prop_section(
            ui,
            None,
            &[["Projection".to_string(), self.projection.clone()]],
        );
        if let Some(gt) = self.geotransform {
            prop_section(
                ui,
                Some("Geotransform"),
                &[
                    ["x ul".to_string(), gt[0].to_string()],
                    ["x res".to_string(), gt[1].to_string()],
                    ["x rot".to_string(), gt[2].to_string()],
                    ["y ul".to_string(), gt[3].to_string()],
                    ["x rot".to_string(), gt[4].to_string()],
                    ["x res".to_string(), gt[5].to_string()],
                ],
            );
        }
        if let Some(bb) = self.bbox {
            prop_section(
                ui,
                Some("BBox"),
                &[
                    ["xmin".to_string(), bb[0].to_string()],
                    ["xmax".to_string(), bb[1].to_string()],
                    ["ymin".to_string(), bb[2].to_string()],
                    ["ymax".to_string(), bb[3].to_string()],
                ],
            );
        }
    }
}

#[derive(Debug)]
pub struct BandProperties {
    dtype: String,
    unit: Option<String>,
    ndv: Option<f64>,
    scale: Option<f64>,
    offset: Option<f64>,
    overviews: Vec<[usize; 3]>,
}

impl BandProperties {
    fn from_rasterband(band: &RasterBand) -> Self {
        let dtype = band.band_type().name();
        let unit = band.unit();
        let unit = if unit.is_empty() { None } else { Some(unit) };
        let overviews_nb = band.overview_count().unwrap_or(0) as usize;
        let mut overviews = vec![];
        for k in 0..overviews_nb {
            if let Ok(o) = band.overview(k) {
                let s = o.size();
                overviews.push([k, s.0, s.1]);
            }
        }
        Self {
            dtype,
            unit,
            ndv: band.no_data_value(),
            scale: band.scale(),
            offset: band.offset(),
            overviews,
        }
    }

    pub fn ui(&self, ui: &mut egui::Ui) {
        let mut props = vec![["dtype".to_string(), self.dtype.clone()]];
        if let Some(v) = &self.unit {
            props.push(["unit".to_string(), v.clone()]);
        }
        if let Some(v) = &self.ndv {
            props.push(["ndv".to_string(), v.to_string()]);
        }
        if let Some(v) = &self.scale {
            props.push(["scale".to_string(), v.to_string()]);
        }
        if let Some(v) = &self.offset {
            props.push(["offset".to_string(), v.to_string()]);
        }
        prop_section(ui, Some("Data"), &props);
        for ovr in &self.overviews {
            prop_section(
                ui,
                Some(&format!("overview {}", ovr[0])),
                &[
                    ["x_size".to_string(), ovr[1].to_string()],
                    ["y_size".to_string(), ovr[2].to_string()],
                ],
            );
        }
    }
}
