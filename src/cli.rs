use crate::{RasterView, viewers::cmap::ColorMap};
use anyhow::Result;
use clap::{CommandFactory, Parser, error::ErrorKind};
use eframe::egui::Context;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, allow_negative_numbers = true)]
pub(crate) struct Cli {
    /// Raster file to open
    file: Option<PathBuf>,

    /// Band to display (1-based)
    #[arg(short, long, requires = "file", value_parser = parse_band)]
    band: Option<usize>,

    /// Colormap name
    #[arg(short, long, requires = "file", value_parser = parse_cpt)]
    cpt: Option<String>,

    /// Minimum value of the color range
    #[arg(long, requires = "file")]
    vmin: Option<f32>,

    /// Maximum value of the color range
    #[arg(long, requires = "file")]
    vmax: Option<f32>,
}

fn parse_band(s: &str) -> Result<usize, String> {
    let n: usize = s
        .parse()
        .map_err(|_| format!("`{s}` is not a valid band number"))?;
    if n >= 1 {
        Ok(n)
    } else {
        Err("band must be >= 1".into())
    }
}

fn parse_cpt(s: &str) -> Result<String, String> {
    let possible_cmaps = [
        s.to_owned(),
        format!("matplotlib/{s}"),
        format!("scm/{s}"),
        format!("cmocean/{s}"),
    ];

    possible_cmaps
        .into_iter()
        .find(|name| ColorMap::from_name(name).is_some())
        .ok_or_else(|| format!("unknown colormap `{s}`"))
}

impl Cli {
    /// Parse and validate; prints a clap-style error and exits on failure.
    pub(crate) fn parse_validated() -> Self {
        let cli = Self::parse();
        if let Err(msg) = cli.ranging_values() {
            Self::command()
                .error(ErrorKind::ValueValidation, msg)
                .exit();
        }
        cli
    }

    /// Resolve vmin/vmax into a range.
    fn ranging_values(&self) -> Result<Option<(f32, f32)>, String> {
        let range = match (self.vmin, self.vmax) {
            (None, None) => return Ok(None),
            (Some(min), Some(max)) => (min, max),
            (Some(v), None) | (None, Some(v)) => {
                if !v.is_finite() || v == 0.0 {
                    return Err("a single vmin/vmax must be finite and non-zero".into());
                }
                if v < 0.0 {
                    (v, -v) // only vmin given, negative: symmetric
                } else {
                    (-v, v) // positive: symmetric
                }
            }
        };
        if !range.0.is_finite() || !range.1.is_finite() || range.0 >= range.1 {
            return Err("vmin must be lower than vmax".into());
        }
        Ok(Some(range))
    }

    pub(crate) fn update_app(&self, app: &mut RasterView, ctx: Context) -> anyhow::Result<()> {
        // Nothing to do without a file (clap already enforces that the other args need one)
        let Some(file) = &self.file else {
            return Ok(());
        };

        // Load first: it may reset the view mode, so apply the settings after
        app.update_path_force(file, ctx)?;

        let view_mode = &mut app.viewer.view_mode;

        if let Some(band) = self.band {
            view_mode.panchro_band = band; // adjust if this field is 0-based (band - 1)
        }

        if let Some(name) = &self.cpt {
            // already validated in parse_cpt
            if let Some(cm) = ColorMap::from_name(name) {
                view_mode.color_interpretation.colormap = cm;
            }
        }

        if let Some(range) = self.ranging_values().map_err(|e| anyhow::anyhow!(e))? {
            view_mode.color_interpretation.with_ranging_values(range);
            view_mode.ranging_mode = crate::viewers::ColorRanging::Manual;
        }

        Ok(())
    }
}
