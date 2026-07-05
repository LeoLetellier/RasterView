use anyhow::Result;
use gdal::{Dataset, raster::ResampleAlg};
use std::path::Path;

pub mod ui;

#[derive(Debug)]
pub struct RasterHandler(Dataset);

// #[derive(Debug)]
// pub(crate) struct RasterHandler {
//     pub path: PathBuf,
//     pub dataset: gdal::Dataset,
//     // Transform the two below into impl direct instead of fully typed in memory
//     pub dataset_properties: DatasetProperties,
//     pub band_properties: Vec<BandProperties>,
// }

impl RasterHandler {
    // pub fn load_tile(&self, id: &TileId, raster_bbox: &PixelBox, bands: &[usize]) -> TilePayload {
    //     let x_off = raster_bbox.xmin() as isize;
    //     let y_off = raster_bbox.ymin() as isize;
    //     let native_w = (raster_bbox.xmax() - raster_bbox.xmin()) as usize;
    //     let native_h = (raster_bbox.ymax() - raster_bbox.ymin()) as usize;

    //     // id.downscaling est maintenant garanti puissance de 2 (1, 2, 4, 8...)
    //     let out_w = (native_w / id.downscaling).max(1);
    //     let out_h = (native_h / id.downscaling).max(1);

    //     let mut values = Vec::with_capacity(out_w * out_h * bands.len());

    //     for &band_idx in bands {
    //         let band = self
    //             .dataset
    //             .rasterband(band_idx + 1)
    //             .expect("band index out of range");

    //         let buffer = self.read_band_at_scale(
    //             &band,
    //             x_off,
    //             y_off,
    //             native_w,
    //             native_h,
    //             out_w,
    //             out_h,
    //             id.downscaling,
    //         );

    //         values.extend_from_slice(&buffer.data());
    //     }

    //     TilePayload {
    //         values,
    //         width: out_w as u32,
    //         height: out_h as u32,
    //         bands: bands.len() as u8,
    //     }
    // }

    /// Read a portion of a specific band.
    ///
    /// Let `resampling_alg` to `None` to use Nearest Neighbor resampling (fastest)
    fn read_band_at_scale(
        &self,
        band: &gdal::raster::RasterBand,
        x_off: isize,
        y_off: isize,
        native_w: usize,
        native_h: usize,
        out_w: usize,
        out_h: usize,
        target_downscaling: usize,
        resampling_alg: Option<ResampleAlg>,
    ) -> gdal::raster::Buffer<f32> {
        let overview_count = band.overview_count().unwrap_or(0);

        // Raster Size
        let (band_full_w, band_full_h) = band.size();

        if overview_count > 0 {
            let mut best: Option<(usize, gdal::raster::RasterBand)> = None;

            // Search for a suitable overview
            for idx in 0..overview_count {
                if let Ok(ov) = band.overview(idx as usize) {
                    let (ov_w, _ov_h) = ov.size();
                    let factor = (band_full_w / ov_w.max(1)).max(1);

                    if factor <= target_downscaling {
                        let is_better = best
                            .as_ref()
                            .map(|(best_factor, _)| factor > *best_factor)
                            .unwrap_or(true);
                        if is_better {
                            best = Some((factor, ov));
                        }
                    }
                }
            }

            // Then read from nearest overview
            if let Some((factor, overview_band)) = best {
                let ov_x_off = x_off / factor as isize;
                let ov_y_off = y_off / factor as isize;
                let ov_w = (native_w / factor).max(1);
                let ov_h = (native_h / factor).max(1);

                if let Ok(buffer) = overview_band.read_as::<f32>(
                    (ov_x_off, ov_y_off),
                    (ov_w, ov_h),
                    (out_w, out_h),
                    resampling_alg,
                ) {
                    return buffer;
                }
            }
        }

        // If no overview just read from whole raster
        band.read_as::<f32>(
            (x_off, y_off),
            (native_w, native_h),
            (out_w, out_h),
            resampling_alg,
        )
        .expect("failed to read raster band")
    }

    fn read_as_time_series(
        &self,
        band_range: Option<(usize, usize)>,
        x_target: usize,
        y_target: usize,
        padding_width: usize,
    ) -> Vec<gdal::raster::Buffer<f32>> {
        todo!()
    }

    // /// Lit la série temporelle pour un pixel (x, y) et une bande/variable donnée.
    // ///
    // /// PLACEHOLDER: je ne sais pas comment ton cube encode le temps (bandes
    // /// successives ? sous-datasets ? fichiers séparés ?). Je pars du principe
    // /// le plus simple -- une bande GDAL par pas de temps pour cette variable --
    // /// mais dis-moi la vraie structure pour que je corrige `time_step_count`
    // /// et `band_index_for` en conséquence.
    // pub fn load_series(&self, x: usize, y: usize, band: usize) -> SeriesPayload {
    //     let n_time_steps = self.time_step_count();
    //     let mut values = Vec::with_capacity(n_time_steps);

    //     for t in 0..n_time_steps {
    //         let band_index = self.band_index_for(band, t);
    //         let raster_band = self
    //             .dataset
    //             .rasterband(band_index)
    //             .expect("band index out of range");

    //         let buffer: gdal::raster::Buffer<f32> = raster_band
    //             .read_as::<f32>((x as isize, y as isize), (1, 1), (1, 1), None)
    //             .expect("failed to read pixel");

    //         values.push(buffer.data()[0]);
    //     }

    //     SeriesPayload { values }
    // }

    // /// Nombre de pas de temps -- placeholder, à corriger selon ta structure réelle.
    // fn time_step_count(&self) -> usize {
    //     self.dataset.raster_count() as usize // FAUX si plusieurs variables partagent les bandes
    // }

    // /// Placeholder: convertit (variable, pas de temps) en index de bande GDAL 1-based.
    // fn band_index_for(&self, band: usize, time_index: usize) -> usize {
    //     band * self.time_step_count() + time_index + 1
    // }

    pub(crate) fn new(path: impl AsRef<Path>) -> Result<Self> {
        let dataset = Dataset::open(&path)?;
        // let dataset_properties = DatasetProperties::from_dataset(&dataset);
        // let mut band_properties = vec![];
        // for b in dataset.rasterbands() {
        //     band_properties.push(BandProperties::from_rasterband(&b?));
        // }

        Ok(Self(dataset))
    }

    fn update_dataset(&mut self, path: &Path) -> Result<&Self> {
        gdal::Dataset::open(path)?;
        Ok(self)
    }

    pub fn raster_size(&self) -> (usize, usize) {
        self.0.raster_size()
    }
}

// #[derive(Debug)]
// pub struct DatasetProperties {
//     driver: String,
//     size: (usize, usize),
//     band_nb: usize,
//     projection: String,
//     geotransform: Option<[f64; 6]>,
//     bbox: Option<[f64; 4]>,
// }

// impl DatasetProperties {
//     fn from_dataset(dataset: &gdal::Dataset) -> Self {
//         let driver = dataset.driver().short_name();
//         let size = dataset.raster_size();
//         let band_nb = dataset.raster_count();
//         let projection = dataset.projection();
//         let geotransform = dataset.geo_transform().ok();
//         let bbox = geotransform.map(|gt| {
//             [
//                 gt[0],
//                 gt[0] + (size.0 as f64) * gt[1],
//                 gt[3] + (size.1 as f64) * gt[5],
//                 gt[3],
//             ]
//         });
//         Self {
//             driver,
//             size,
//             band_nb,
//             projection,
//             geotransform,
//             bbox,
//         }
//     }

//     pub fn ui(&self, ui: &mut egui::Ui) {
//         prop_section(ui, None, &[["Driver".to_string(), self.driver.clone()]]);
//         prop_section(
//             ui,
//             Some("Size"),
//             &[
//                 ["x".to_string(), self.size.0.to_string()],
//                 ["y".to_string(), self.size.1.to_string()],
//             ],
//         );
//         prop_section(
//             ui,
//             None,
//             &[["Band nb".to_string(), self.band_nb.to_string()]],
//         );
//         prop_section(
//             ui,
//             None,
//             &[["Projection".to_string(), self.projection.clone()]],
//         );
//         if let Some(gt) = self.geotransform {
//             prop_section(
//                 ui,
//                 Some("Geotransform"),
//                 &[
//                     ["x ul".to_string(), gt[0].to_string()],
//                     ["x res".to_string(), gt[1].to_string()],
//                     ["x rot".to_string(), gt[2].to_string()],
//                     ["y ul".to_string(), gt[3].to_string()],
//                     ["x rot".to_string(), gt[4].to_string()],
//                     ["x res".to_string(), gt[5].to_string()],
//                 ],
//             );
//         }
//         if let Some(bb) = self.bbox {
//             prop_section(
//                 ui,
//                 Some("BBox"),
//                 &[
//                     ["xmin".to_string(), bb[0].to_string()],
//                     ["xmax".to_string(), bb[1].to_string()],
//                     ["ymin".to_string(), bb[2].to_string()],
//                     ["ymax".to_string(), bb[3].to_string()],
//                 ],
//             );
//         }
//     }
// }

// #[derive(Debug)]
// pub struct BandProperties {
//     dtype: String,
//     unit: Option<String>,
//     ndv: Option<f64>,
//     scale: Option<f64>,
//     offset: Option<f64>,
//     overviews: Vec<[usize; 3]>,
// }

// impl BandProperties {
//     fn from_rasterband(band: &RasterBand) -> Self {
//         let dtype = band.band_type().name();
//         let unit = band.unit();
//         let unit = if unit.is_empty() { None } else { Some(unit) };
//         let overviews_nb = band.overview_count().unwrap_or(0) as usize;
//         let mut overviews = vec![];
//         for k in 0..overviews_nb {
//             if let Ok(o) = band.overview(k) {
//                 let s = o.size();
//                 overviews.push([k, s.0, s.1]);
//             }
//         }
//         Self {
//             dtype,
//             unit,
//             ndv: band.no_data_value(),
//             scale: band.scale(),
//             offset: band.offset(),
//             overviews,
//         }
//     }

//     pub fn ui(&self, ui: &mut egui::Ui) {
//         let mut props = vec![["dtype".to_string(), self.dtype.clone()]];
//         if let Some(v) = &self.unit {
//             props.push(["unit".to_string(), v.clone()]);
//         }
//         if let Some(v) = &self.ndv {
//             props.push(["ndv".to_string(), v.to_string()]);
//         }
//         if let Some(v) = &self.scale {
//             props.push(["scale".to_string(), v.to_string()]);
//         }
//         if let Some(v) = &self.offset {
//             props.push(["offset".to_string(), v.to_string()]);
//         }
//         prop_section(ui, Some("Data"), &props);
//         for ovr in &self.overviews {
//             prop_section(
//                 ui,
//                 Some(&format!("overview {}", ovr[0])),
//                 &[
//                     ["x_size".to_string(), ovr[1].to_string()],
//                     ["y_size".to_string(), ovr[2].to_string()],
//                 ],
//             );
//         }
//     }
// }

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use gdal::raster::{Buffer, GdalType};
//     use gdal::{Dataset, DriverManager};

//     /// Crée un dataset GDAL en mémoire (driver MEM) avec un motif connu,
//     /// pour tester le pipeline de lecture sans fichier disque.
//     fn make_test_dataset(width: usize, height: usize) -> Dataset {
//         let driver = DriverManager::get_driver_by_name("MEM").unwrap();
//         let mut dataset = driver
//             .create_with_band_type::<f32, _>("", width, height, 1)
//             .unwrap();

//         dataset
//             .set_geo_transform(&[0.0, 1.0, 0.0, 0.0, 0.0, -1.0]) // north-up, 1 unit/pixel
//             .unwrap();

//         let mut band = dataset.rasterband(1).unwrap();
//         let mut data = vec![0f32; width * height];
//         for y in 0..height {
//             for x in 0..width {
//                 data[y * width + x] = (x + y) as f32; // motif prévisible
//             }
//         }
//         let mut buffer = Buffer::new((width, height), data);
//         band.write((0, 0), (width, height), &mut buffer).unwrap();

//         // Génère de vraies overviews (facteurs 2, 4) pour tester cette branche.
//         dataset.build_overviews("NEAREST", &[2, 4], &[1]).unwrap();

//         dataset
//     }

//     #[test]
//     fn load_tile_native_resolution_matches_pattern() {
//         let dataset = make_test_dataset(100, 100);
//         let dataset_properties = DatasetProperties::from_dataset(&dataset);
//         let raster_handler = RasterHandler {
//             path: "".into(),
//             dataset,
//             dataset_properties,
//             band_properties: vec![],
//         };

//         let id = TileId {
//             downscaling: 1,
//             tile_x: 0,
//             tile_y: 0,
//             time_index: 0,
//         };
//         let raster_bbox = PixelBox::from([0, 10, 0, 10]);

//         let payload = raster_handler.load_tile(&id, &raster_bbox, &[0]);

//         assert_eq!(payload.width, 10);
//         assert_eq!(payload.height, 10);
//         // pixel (0,0) du motif x+y -> valeur attendue 0.0
//         assert_eq!(payload.values[0], 0.0);
//         // pixel (5,5) -> valeur attendue 10.0 (5+5)
//         assert_eq!(payload.values[5 * 10 + 5], 10.0);
//     }

//     #[test]
//     fn load_tile_downscaled_uses_overview() {
//         let dataset = make_test_dataset(100, 100);
//         let dataset_properties = DatasetProperties::from_dataset(&dataset);
//         let raster_handler = RasterHandler {
//             path: "".into(),
//             dataset,
//             dataset_properties,
//             band_properties: vec![],
//         };

//         let id = TileId {
//             downscaling: 4,
//             tile_x: 0,
//             tile_y: 0,
//             time_index: 0,
//         };
//         let raster_bbox = PixelBox::from([0, 40, 0, 40]);

//         let payload = raster_handler.load_tile(&id, &raster_bbox, &[0]);

//         // 40 pixels natifs / downscaling 4 -> 10x10 en sortie
//         assert_eq!(payload.width, 10);
//         assert_eq!(payload.height, 10);
//         // pas de crash, valeurs dans une plage plausible (0 à 80 pour ce motif)
//         assert!(payload.values.iter().all(|&v| (0.0..=80.0).contains(&v)));
//     }
// }
