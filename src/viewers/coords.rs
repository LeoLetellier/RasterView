use egui_plot::PlotPoint;
use num_traits::NumCast;
use std::ops::{Div, Sub};

/// Trait for an axis-aligned bounding box (AABB) with four extents.
///
/// A `Bbox<T>` describes a 2D rectangular region aligned to the x/y axes via:
/// - `xmin` / `xmax` for the x extents
/// - `ymin` / `ymax` for the y extents
///
/// This trait also provides common operations:
/// - `center()`: the midpoint of the x/y extents
/// - `intersection()`: the overlapping region (returns `None` when empty)
/// - `union()`: the minimal box that contains both inputs
pub trait Bbox<T>: From<[T; 4]>
where
    T: Sub<Output = T> + Div<Output = T> + NumCast + Copy,
{
    /// Returns the minimum x-boundary of the bounding box.
    ///
    /// In other words, this is the left extent on the x axis.
    fn xmin(&self) -> T;

    /// Returns the maximum x-boundary of the bounding box.
    ///
    /// In other words, this is the right extent on the x axis.
    fn xmax(&self) -> T;

    /// Returns the minimum y-boundary of the bounding box.
    ///
    /// In other words, this is the bottom extent on the y axis.
    fn ymin(&self) -> T;

    /// Returns the maximum y-boundary of the bounding box.
    ///
    /// In other words, this is the top extent on the y axis.
    fn ymax(&self) -> T;

    /// Returns the center point of the bounding box.
    ///
    /// The center is computed as:
    /// - x: `(xmin + xmax) / 2`
    /// - y: `(ymin + ymax) / 2`
    fn center(&self) -> PlotPoint;

    /// Returns the width of the bounding box.
    ///
    /// Computed as xmax - xmin
    fn width(&self) -> T {
        self.xmax() - self.xmin()
    }

    /// Returns the height of the bounding box
    ///
    /// Computed as ymax - ymin
    fn height(&self) -> T {
        self.ymax() - self.ymin()
    }

    /// Returns (width, height) downsampled by `downsample` halvings
    /// (0 = full res, 1 = half res, 2 = quarter res, ...).
    ///
    /// Use that for gdal readband usage
    fn size_with_downsampling(&self, downsample: usize) -> (T, T) {
        let factor_usize = 1usize << downsample;
        let factor: T = NumCast::from(factor_usize).expect("downsample factor should fit in T");

        (self.width() / factor, self.height() / factor)
    }

    /// Returns the intersection of this bounding box with another.
    ///
    /// The intersection is the axis-aligned region where both boxes overlap.
    ///
    /// # Return value
    /// - `Some(bbox)` if the intersection is non-empty
    /// - `None` if the boxes do not intersect
    fn intersection(&self, other: &impl Bbox<T>) -> Option<Self>
    where
        T: Copy + PartialOrd,
    {
        let xmin = partial_max(self.xmin(), other.xmin());
        let xmax = partial_min(self.xmax(), other.xmax());
        let ymin = partial_max(self.ymin(), other.ymin());
        let ymax = partial_min(self.ymax(), other.ymax());
        if xmin > xmax || ymin > ymax {
            None
        } else {
            Some(Self::from([xmin, xmax, ymin, ymax]))
        }
    }

    /// Returns the union of this bounding box with another.
    ///
    /// The union is the smallest axis-aligned bounding box that contains both inputs.
    fn union(&self, other: &impl Bbox<T>) -> Self
    where
        T: Copy + PartialOrd,
    {
        let xmin = partial_min(self.xmin(), other.xmin());
        let xmax = partial_max(self.xmax(), other.xmax());
        let ymin = partial_min(self.ymin(), other.ymin());
        let ymax = partial_max(self.ymax(), other.ymax());
        Self::from([xmin, xmax, ymin, ymax])
    }
}

fn partial_min<T: PartialOrd>(a: T, b: T) -> T {
    if a < b { a } else { b }
}

fn partial_max<T: PartialOrd>(a: T, b: T) -> T {
    if a > b { a } else { b }
}

/// An axis-aligned bounding box in pixel coordinates (`usize`).
///
/// The extents are stored in the order `[xmin, xmax, ymin, ymax]`.
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct PixelBox {
    xmin: usize,
    xmax: usize,
    ymin: usize,
    ymax: usize,
}

impl From<[usize; 4]> for PixelBox {
    /// Constructs a `PixelBox` from `[xmin, xmax, ymin, ymax]`.
    fn from([xmin, xmax, ymin, ymax]: [usize; 4]) -> Self {
        Self {
            xmin,
            xmax,
            ymin,
            ymax,
        }
    }
}

impl Bbox<usize> for PixelBox {
    /// Minimum x-boundary (left extent).
    fn xmin(&self) -> usize {
        self.xmin
    }

    /// Maximum x-boundary (right extent).
    fn xmax(&self) -> usize {
        self.xmax
    }

    /// Minimum y-boundary (bottom extent).
    fn ymin(&self) -> usize {
        self.ymin
    }

    /// Maximum y-boundary (top extent).
    fn ymax(&self) -> usize {
        self.ymax
    }

    fn center(&self) -> PlotPoint {
        let xmin = self.xmin as f64;
        let xmax = self.xmax as f64;
        let ymin = self.ymin as f64;
        let ymax = self.ymax as f64;
        PlotPoint::new((xmin + xmax) / 2.0, (ymin + ymax) / 2.0)
    }
}

// /// Downsample version of a pixelbox
// ///
// /// Should only use `.width()` and `.height()` on it for gdal usage
// type DownPixelBox = PixelBox;

// /// Handle all info needed to fetch and position a tile
// ///
// /// need band id, pixelbbox from full size, downsample
// ///
// /// viewport is in raster full size
// struct RawTile {
//     band: usize,
//     full_pbox: PixelBox,
//     downsample: usize,
// }

// impl RawTile {
//     pub fn to_downsample_pixelbox(&self) -> DownPixelBox {
//         let factor = 1usize << self.downsample; // 2^downsample

//         let down_width = self.full_pbox.width() / factor;
//         let down_height = self.full_pbox.height() / factor;

//         DownPixelBox::from([0, down_width, 0, down_height])
//     }
// }

/// An axis-aligned bounding box in geographic/continuous coordinates (`f64`).
///
/// The extents are stored in the order `[xmin, xmax, ymin, ymax]`.
#[derive(Debug, Clone, Copy)]
pub struct GeoBox {
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
}

impl From<[f64; 4]> for GeoBox {
    /// Constructs a `GeoBox` from `[xmin, xmax, ymin, ymax]`.
    fn from([xmin, xmax, ymin, ymax]: [f64; 4]) -> Self {
        Self {
            xmin,
            xmax,
            ymin,
            ymax,
        }
    }
}

impl Bbox<f64> for GeoBox {
    /// Minimum x-boundary.
    fn xmin(&self) -> f64 {
        self.xmin
    }

    /// Maximum x-boundary.
    fn xmax(&self) -> f64 {
        self.xmax
    }

    /// Minimum y-boundary.
    fn ymin(&self) -> f64 {
        self.ymin
    }

    /// Maximum y-boundary.
    fn ymax(&self) -> f64 {
        self.ymax
    }

    fn center(&self) -> PlotPoint {
        PlotPoint::new((self.xmin + self.xmax) / 2.0, (self.ymin + self.ymax) / 2.0)
    }
}

/// GDAL geotransform definition
///
/// > A geotransform is an affine transformation from the image coordinate space (row, column), also known as (pixel, line) to the georeferenced coordinate space (projected or geographic coordinates).
///
/// [GDAL documentation](https://gdal.org/en/stable/tutorials/geotransforms_tut.html)
pub struct GeoTransform {
    /// x-coordinate of the upper-left corner of the upper-left pixel
    x_off: f64,
    /// w-e pixel resolution / pixel width
    x_res: f64,
    /// row rotation (typically zero)
    x_rot: f64,
    /// y-coordinate of the upper-left corner of the upper-left pixel
    y_off: f64,
    /// column rotation (typically zero)
    y_rot: f64,
    /// n-s pixel resolution / pixel height (negative value for a north-up image)
    y_res: f64,
}

impl GeoTransform {
    /// X and Y offsets of the geotransform
    ///
    /// This is the position of the upper-left corner
    pub fn offsets(&self) -> PlotPoint {
        PlotPoint::new(self.x_off, self.y_off)
    }

    /// X and Y resolution of the pixels
    ///
    /// This is the width and height of the pixel
    pub fn resolutions(&self) -> PlotPoint {
        PlotPoint::new(self.x_res, self.y_res)
    }

    /// X and Y rotations of the pixels
    pub fn rotations(&self) -> PlotPoint {
        PlotPoint::new(self.x_rot, self.y_rot)
    }

    /// New geotransform with no rotation
    ///
    /// To add a rotation, use `with_rotation()` instead
    pub fn new(x_off: f64, x_res: f64, y_off: f64, y_res: f64) -> GeoTransform {
        GeoTransform {
            x_off,
            x_res,
            x_rot: 0.0,
            y_off,
            y_rot: 0.0,
            y_res,
        }
    }

    /// New geotransform with rotation
    pub fn with_rotation(
        x_off: f64,
        x_res: f64,
        x_rot: f64,
        y_off: f64,
        y_res: f64,
        y_rot: f64,
    ) -> GeoTransform {
        GeoTransform {
            x_off,
            x_res,
            x_rot,
            y_off,
            y_rot,
            y_res,
        }
    }

    /// Pixel/line -> geo coordinates.
    ///
    /// X_geo = x_off + x_pixel * x_res + y_line * x_rot
    /// Y_geo = y_off + x_pixel * y_rot + y_line * y_res
    #[inline]
    pub fn pixel_to_geo(&self, x_pixel: f64, y_line: f64) -> (f64, f64) {
        let x_geo = self.x_off + x_pixel * self.x_res + y_line * self.x_rot;
        let y_geo = self.y_off + x_pixel * self.y_rot + y_line * self.y_res;
        (x_geo, y_geo)
    }

    /// Geo -> pixel/line coordinates (inverse of the 2x2 linear system).
    ///
    /// Returns `None` if the transform is degenerate (determinant ~ 0),
    /// which would otherwise produce NaN/inf.
    #[inline]
    pub fn geo_to_pixel(&self, x_geo: f64, y_geo: f64) -> Option<(f64, f64)> {
        let det = self.x_res * self.y_res - self.x_rot * self.y_rot;
        if det.abs() < f64::EPSILON {
            return None;
        }

        let dx = x_geo - self.x_off;
        let dy = y_geo - self.y_off;

        let x_pixel = (self.y_res * dx - self.x_rot * dy) / det;
        let y_line = (self.x_res * dy - self.y_rot * dx) / det;
        Some((x_pixel, y_line))
    }
}

impl From<[f64; 6]> for GeoTransform {
    fn from(value: [f64; 6]) -> Self {
        GeoTransform {
            x_off: value[0],
            x_res: value[1],
            x_rot: value[2],
            y_off: value[3],
            y_rot: value[4],
            y_res: value[5],
        }
    }
}

impl GeoTransform {
    /// Converts a pixel-space bounding box into the corresponding geo bounding box.
    ///
    /// Transforms all four corners (not just two) so this stays correct even
    /// when `x_rot`/`y_rot` are non-zero -- with rotation, the top-left and
    /// bottom-right pixel corners don't necessarily map to the geo min/max corners.
    pub fn pixel_box_to_geo_box(&self, px_box: &PixelBox) -> GeoBox {
        let corners = [
            (px_box.xmin() as f64, px_box.ymin() as f64),
            (px_box.xmax() as f64, px_box.ymin() as f64),
            (px_box.xmin() as f64, px_box.ymax() as f64),
            (px_box.xmax() as f64, px_box.ymax() as f64),
        ];

        let mut xmin = f64::INFINITY;
        let mut xmax = f64::NEG_INFINITY;
        let mut ymin = f64::INFINITY;
        let mut ymax = f64::NEG_INFINITY;

        for (px, py) in corners {
            let (gx, gy) = self.pixel_to_geo(px, py);
            xmin = xmin.min(gx);
            xmax = xmax.max(gx);
            ymin = ymin.min(gy);
            ymax = ymax.max(gy);
        }

        GeoBox::from([xmin, xmax, ymin, ymax])
    }

    /// Converts a geo bounding box into the corresponding pixel-space bounding box.
    ///
    /// Returns `None` if the transform is degenerate (see `geo_to_pixel`).
    /// Fractional pixel coordinates are floored/ceiled outward so the returned
    /// `PixelBox` fully covers the requested geo area, and clamped at 0 since
    /// `PixelBox` uses `usize`.
    pub fn geo_box_to_pixel_box(&self, geo_box: &GeoBox) -> Option<PixelBox> {
        let corners = [
            (geo_box.xmin(), geo_box.ymin()),
            (geo_box.xmax(), geo_box.ymin()),
            (geo_box.xmin(), geo_box.ymax()),
            (geo_box.xmax(), geo_box.ymax()),
        ];

        let mut xmin = f64::INFINITY;
        let mut xmax = f64::NEG_INFINITY;
        let mut ymin = f64::INFINITY;
        let mut ymax = f64::NEG_INFINITY;

        for (gx, gy) in corners {
            let (px, py) = self.geo_to_pixel(gx, gy)?;
            xmin = xmin.min(px);
            xmax = xmax.max(px);
            ymin = ymin.min(py);
            ymax = ymax.max(py);
        }

        let xmin = xmin.floor().max(0.0) as usize;
        let xmax = xmax.ceil().max(0.0) as usize;
        let ymin = ymin.floor().max(0.0) as usize;
        let ymax = ymax.ceil().max(0.0) as usize;

        Some(PixelBox::from([xmin, xmax, ymin, ymax]))
    }
}

// The following tests have been generated by Claude Sonnet 5
#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn pixel_to_geo_north_up_no_rotation() {
        // Classic north-up raster: origin at (100, 50), 2.0 units/pixel wide,
        // -1.5 units/pixel tall (y decreases going down), no rotation.
        let gt = GeoTransform::new(100.0, 2.0, 50.0, -1.5);

        // Upper-left pixel (0,0) should map exactly to the origin.
        let (x, y) = gt.pixel_to_geo(0.0, 0.0);
        assert!(approx_eq(x, 100.0, 1e-9));
        assert!(approx_eq(y, 50.0, 1e-9));

        // Pixel (10, 4) -> geo
        let (x, y) = gt.pixel_to_geo(10.0, 4.0);
        assert!(approx_eq(x, 100.0 + 10.0 * 2.0, 1e-9));
        assert!(approx_eq(y, 50.0 + 4.0 * -1.5, 1e-9));
    }

    #[test]
    fn geo_to_pixel_north_up_no_rotation() {
        let gt = GeoTransform::new(100.0, 2.0, 50.0, -1.5);

        let (px, py) = gt
            .geo_to_pixel(120.0, 44.0)
            .expect("non-degenerate transform");
        assert!(approx_eq(px, 10.0, 1e-9));
        assert!(approx_eq(py, 4.0, 1e-9));
    }

    #[test]
    fn round_trip_pixel_geo_pixel_no_rotation() {
        let gt = GeoTransform::new(-180.0, 0.0833, 90.0, -0.0833);

        for &(px, py) in &[(0.0, 0.0), (1234.5, 678.9), (-50.0, 3000.0)] {
            let (gx, gy) = gt.pixel_to_geo(px, py);
            let (px2, py2) = gt.geo_to_pixel(gx, gy).expect("non-degenerate transform");
            assert!(
                approx_eq(px, px2, 1e-6),
                "px round-trip failed: {} vs {}",
                px,
                px2
            );
            assert!(
                approx_eq(py, py2, 1e-6),
                "py round-trip failed: {} vs {}",
                py,
                py2
            );
        }
    }

    #[test]
    fn round_trip_geo_pixel_geo_with_rotation() {
        // Non-zero rotation/shear terms.
        // with_rotation(x_off, x_res, x_rot, y_off, y_res, y_rot)
        let gt = GeoTransform::with_rotation(500.0, 1.8, 0.3, 200.0, -1.2, -0.2);

        for &(gx, gy) in &[(500.0, 200.0), (750.3, 120.7), (300.0, 400.0)] {
            let (px, py) = gt.geo_to_pixel(gx, gy).expect("non-degenerate transform");
            let (gx2, gy2) = gt.pixel_to_geo(px, py);
            assert!(
                approx_eq(gx, gx2, 1e-6),
                "x round-trip failed: {} vs {}",
                gx,
                gx2
            );
            assert!(
                approx_eq(gy, gy2, 1e-6),
                "y round-trip failed: {} vs {}",
                gy,
                gy2
            );
        }
    }

    #[test]
    fn pixel_to_geo_with_rotation() {
        // with_rotation(x_off, x_res, x_rot, y_off, y_res, y_rot)
        let gt = GeoTransform::with_rotation(0.0, 1.0, 0.5, 0.0, 1.0, 0.25);

        // X_geo = x_off + x_pixel * x_res + y_line * x_rot
        // Y_geo = y_off + x_pixel * y_rot + y_line * y_res
        let (x, y) = gt.pixel_to_geo(10.0, 4.0);
        assert!(approx_eq(x, 0.0 + 10.0 * 1.0 + 4.0 * 0.5, 1e-9)); // 12.0
        assert!(approx_eq(y, 0.0 + 10.0 * 0.25 + 4.0 * 1.0, 1e-9)); // 6.5
    }

    #[test]
    fn geo_to_pixel_degenerate_transform_returns_none() {
        // det = x_res * y_res - x_rot * y_rot == 0
        // Pick x_res=2, y_res=1, x_rot=1, y_rot=2 -> det = 2*1 - 1*2 = 0
        let gt = GeoTransform::with_rotation(0.0, 2.0, 1.0, 0.0, 1.0, 2.0);

        assert!(gt.geo_to_pixel(10.0, 10.0).is_none());
    }

    #[test]
    fn geo_to_pixel_zero_transform_returns_none() {
        let gt = GeoTransform::new(0.0, 0.0, 0.0, 0.0);
        assert!(gt.geo_to_pixel(1.0, 1.0).is_none());
    }

    #[test]
    fn negative_pixel_line_values_are_handled() {
        // Some callers may query outside the raster bounds (e.g. clamping logic
        // upstream) -- the transform itself shouldn't panic or misbehave.
        let gt = GeoTransform::new(0.0, 1.0, 0.0, -1.0);
        let (x, y) = gt.pixel_to_geo(-5.0, -3.0);
        assert!(approx_eq(x, -5.0, 1e-9));
        assert!(approx_eq(y, 3.0, 1e-9));
    }
}

#[cfg(test)]
mod bbox_tests {
    use super::*;

    // ---------- PixelBox (Bbox<usize>) ----------

    #[test]
    fn pixel_box_accessors() {
        let b = PixelBox::from([10, 50, 20, 80]);
        assert_eq!(b.xmin(), 10);
        assert_eq!(b.xmax(), 50);
        assert_eq!(b.ymin(), 20);
        assert_eq!(b.ymax(), 80);
    }

    #[test]
    fn pixel_box_center() {
        let b = PixelBox::from([0, 100, 0, 50]);
        let c = b.center();
        assert!((c.x - 50.0).abs() < 1e-9);
        assert!((c.y - 25.0).abs() < 1e-9);
    }

    #[test]
    fn pixel_box_center_odd_extent() {
        // (10 + 21) / 2 = 15.5, checks the override isn't truncating via integer division.
        let b = PixelBox::from([10, 21, 0, 0]);
        let c = b.center();
        assert!((c.x - 15.5).abs() < 1e-9);
    }

    #[test]
    fn pixel_box_intersection_overlapping() {
        let a = PixelBox::from([0, 100, 0, 100]);
        let b = PixelBox::from([50, 150, 50, 150]);
        let inter = a.intersection(&b).expect("should overlap");
        assert_eq!(inter.xmin(), 50);
        assert_eq!(inter.xmax(), 100);
        assert_eq!(inter.ymin(), 50);
        assert_eq!(inter.ymax(), 100);
    }

    #[test]
    fn pixel_box_intersection_touching_edges() {
        // Boxes that only touch at an edge still count as a (degenerate) intersection.
        let a = PixelBox::from([0, 50, 0, 50]);
        let b = PixelBox::from([50, 100, 0, 50]);
        let inter = a.intersection(&b).expect("touching edges should intersect");
        assert_eq!(inter.xmin(), 50);
        assert_eq!(inter.xmax(), 50);
    }

    #[test]
    fn pixel_box_intersection_disjoint_returns_none() {
        let a = PixelBox::from([0, 10, 0, 10]);
        let b = PixelBox::from([20, 30, 20, 30]);
        assert!(a.intersection(&b).is_none());
    }

    #[test]
    fn pixel_box_union() {
        let a = PixelBox::from([0, 10, 0, 10]);
        let b = PixelBox::from([5, 20, 5, 30]);
        let u = a.union(&b);
        assert_eq!(u.xmin(), 0);
        assert_eq!(u.xmax(), 20);
        assert_eq!(u.ymin(), 0);
        assert_eq!(u.ymax(), 30);
    }

    #[test]
    fn pixel_box_union_disjoint_covers_gap() {
        let a = PixelBox::from([0, 10, 0, 10]);
        let b = PixelBox::from([100, 110, 100, 110]);
        let u = a.union(&b);
        assert_eq!(u.xmin(), 0);
        assert_eq!(u.xmax(), 110);
        assert_eq!(u.ymin(), 0);
        assert_eq!(u.ymax(), 110);
    }

    // ---------- GeoBox (Bbox<f64>) ----------

    #[test]
    fn geo_box_accessors() {
        let b = GeoBox::from([-10.0, 10.0, -5.0, 5.0]);
        assert_eq!(b.xmin(), -10.0);
        assert_eq!(b.xmax(), 10.0);
        assert_eq!(b.ymin(), -5.0);
        assert_eq!(b.ymax(), 5.0);
    }

    #[test]
    fn geo_box_center() {
        let b = GeoBox::from([-10.0, 10.0, 0.0, 20.0]);
        let c = b.center();
        assert!((c.x - 0.0).abs() < 1e-9);
        assert!((c.y - 10.0).abs() < 1e-9);
    }

    #[test]
    fn geo_box_intersection_overlapping() {
        let a = GeoBox::from([0.0, 10.0, 0.0, 10.0]);
        let b = GeoBox::from([5.0, 15.0, 5.0, 15.0]);
        let inter = a.intersection(&b).expect("should overlap");
        assert!((inter.xmin() - 5.0).abs() < 1e-9);
        assert!((inter.xmax() - 10.0).abs() < 1e-9);
        assert!((inter.ymin() - 5.0).abs() < 1e-9);
        assert!((inter.ymax() - 10.0).abs() < 1e-9);
    }

    #[test]
    fn geo_box_intersection_disjoint_returns_none() {
        let a = GeoBox::from([0.0, 1.0, 0.0, 1.0]);
        let b = GeoBox::from([2.0, 3.0, 2.0, 3.0]);
        assert!(a.intersection(&b).is_none());
    }

    #[test]
    fn geo_box_intersection_identical_boxes() {
        let a = GeoBox::from([0.0, 10.0, 0.0, 10.0]);
        let b = GeoBox::from([0.0, 10.0, 0.0, 10.0]);
        let inter = a.intersection(&b).expect("identical boxes overlap fully");
        assert_eq!(inter.xmin(), a.xmin());
        assert_eq!(inter.xmax(), a.xmax());
    }

    #[test]
    fn geo_box_union() {
        let a = GeoBox::from([-1.0, 1.0, -1.0, 1.0]);
        let b = GeoBox::from([0.5, 2.0, -3.0, 0.5]);
        let u = a.union(&b);
        assert!((u.xmin() - (-1.0)).abs() < 1e-9);
        assert!((u.xmax() - 2.0).abs() < 1e-9);
        assert!((u.ymin() - (-3.0)).abs() < 1e-9);
        assert!((u.ymax() - 1.0).abs() < 1e-9);
    }
}

#[cfg(test)]
mod geotransform_bbox_tests {
    use super::*;

    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn pixel_box_to_geo_box_north_up_no_rotation() {
        let gt = GeoTransform::new(100.0, 2.0, 50.0, -1.5);
        let px_box = PixelBox::from([0, 10, 0, 4]);

        let geo = gt.pixel_box_to_geo_box(&px_box);

        assert!(approx_eq(geo.xmin(), 100.0, 1e-9));
        assert!(approx_eq(geo.xmax(), 100.0 + 10.0 * 2.0, 1e-9));
        // y_res is negative, so ymax(pixel=0) is at y_off and ymin is lower.
        assert!(approx_eq(geo.ymax(), 50.0, 1e-9));
        assert!(approx_eq(geo.ymin(), 50.0 + 4.0 * -1.5, 1e-9));
    }

    #[test]
    fn geo_box_to_pixel_box_north_up_no_rotation() {
        let gt = GeoTransform::new(100.0, 2.0, 50.0, -1.5);
        let geo_box = GeoBox::from([100.0, 120.0, 44.0, 50.0]);

        let px = gt
            .geo_box_to_pixel_box(&geo_box)
            .expect("non-degenerate transform");

        assert_eq!(px.xmin(), 0);
        assert_eq!(px.xmax(), 10);
        assert_eq!(px.ymin(), 0);
        assert_eq!(px.ymax(), 4);
    }

    #[test]
    fn round_trip_pixel_to_geo_to_pixel() {
        let gt = GeoTransform::new(-180.0, 0.0833, 90.0, -0.0833);
        let px_box = PixelBox::from([100, 200, 50, 150]);

        let geo = gt.pixel_box_to_geo_box(&px_box);
        let px2 = gt
            .geo_box_to_pixel_box(&geo)
            .expect("non-degenerate transform");

        // Allow +/-1 pixel due to floor/ceil outward rounding.
        assert!(px2.xmin() <= px_box.xmin());
        assert!(px2.xmax() >= px_box.xmax());
        assert!(px2.ymin() <= px_box.ymin());
        assert!(px2.ymax() >= px_box.ymax());
    }

    #[test]
    fn pixel_box_to_geo_box_with_rotation_uses_all_corners() {
        // With rotation, top-left/bottom-right pixel corners don't map to
        // geo min/max directly -- verify all four corners are considered.
        let gt = GeoTransform::with_rotation(0.0, 1.0, 0.5, 0.0, 1.0, 0.25);
        let px_box = PixelBox::from([0, 10, 0, 10]);

        let geo = gt.pixel_box_to_geo_box(&px_box);

        // Manually compute all four corners to find the true min/max.
        let corners = [
            gt.pixel_to_geo(0.0, 0.0),
            gt.pixel_to_geo(10.0, 0.0),
            gt.pixel_to_geo(0.0, 10.0),
            gt.pixel_to_geo(10.0, 10.0),
        ];
        let expected_xmin = corners.iter().map(|c| c.0).fold(f64::INFINITY, f64::min);
        let expected_xmax = corners
            .iter()
            .map(|c| c.0)
            .fold(f64::NEG_INFINITY, f64::max);
        let expected_ymin = corners.iter().map(|c| c.1).fold(f64::INFINITY, f64::min);
        let expected_ymax = corners
            .iter()
            .map(|c| c.1)
            .fold(f64::NEG_INFINITY, f64::max);

        assert!(approx_eq(geo.xmin(), expected_xmin, 1e-9));
        assert!(approx_eq(geo.xmax(), expected_xmax, 1e-9));
        assert!(approx_eq(geo.ymin(), expected_ymin, 1e-9));
        assert!(approx_eq(geo.ymax(), expected_ymax, 1e-9));
    }

    #[test]
    fn geo_box_to_pixel_box_degenerate_transform_returns_none() {
        let gt = GeoTransform::with_rotation(0.0, 2.0, 1.0, 0.0, 1.0, 2.0); // det == 0
        let geo_box = GeoBox::from([0.0, 10.0, 0.0, 10.0]);

        assert!(gt.geo_box_to_pixel_box(&geo_box).is_none());
    }

    #[test]
    fn geo_box_to_pixel_box_clamps_negative_to_zero() {
        // A geo box that would map to negative pixel coordinates should be
        // clamped at 0 rather than underflowing usize.
        let gt = GeoTransform::new(0.0, 1.0, 0.0, -1.0);
        let geo_box = GeoBox::from([-50.0, 10.0, -60.0, 5.0]);

        let px = gt
            .geo_box_to_pixel_box(&geo_box)
            .expect("non-degenerate transform");

        assert_eq!(px.xmin(), 0);
        assert_eq!(px.ymin(), 0);
    }
}
