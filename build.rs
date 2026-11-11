use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const LUT_LEN: usize = 256;
const CMAP_SUBDIR: [&str; 2] = ["resources", "colormaps"];
const GENERATED_DIRS: [&str; 3] = ["matplotlib", "cmocean", "scm"];
const DEFAULT_CMAP_NAME: &str = "matplotlib/gray";

/// Relative tolerance (fraction of the full z range) for gaps/overlaps between segments.
const SEGMENT_JOIN_TOLERANCE: f32 = 1e-4;

struct Segment {
    z0: f32,
    c0: [u8; 4],
    z1: f32,
    c1: [u8; 4],
}

struct ParsedCpt {
    segments: Vec<Segment>,
    below: [u8; 4],
    above: [u8; 4],
    nan: [u8; 4],
}

fn colormap_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    CMAP_SUBDIR.iter().fold(manifest_dir, |p, s| p.join(s))
}

fn is_missing_or_empty(dir: &Path) -> bool {
    // read_dir errors if the directory doesn't exist, so this covers both cases
    dir.read_dir()
        .map(|mut d| d.next().is_none())
        .unwrap_or(true)
}

fn launch_fetch(project_dir: &Path, cmap_name: &str) {
    let script = format!("fetch_{cmap_name}.py");

    let status = Command::new("uv")
        .args(["run", "python", &script])
        .current_dir(project_dir)
        .status()
        .expect("failed to spawn `uv` — is it installed and on PATH?");

    if !status.success() {
        panic!(
            "colormap generation script `{script}` failed with status: {:?}",
            status.code()
        );
    }
}

/// Only runs the scripts whose output folder is missing or empty.
fn fetch_cmaps(root: &Path) {
    for name in GENERATED_DIRS {
        if is_missing_or_empty(&root.join(name)) {
            println!(
                "cargo:warning=Colormap folder `{name}` missing or empty, generating via uv..."
            );
            launch_fetch(root, name);
        }
    }
}

fn main() {
    let root = colormap_root();
    fetch_cmaps(&root);

    // Watch only the generated folders, not .venv / .ruff_cache
    for name in GENERATED_DIRS {
        println!("cargo:rerun-if-changed={}", root.join(name).display());
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let mut files = walk_cpt_files(&root);
    files.sort(); // read_dir order is arbitrary: sort for deterministic output / stable offsets

    let mut blob: Vec<u8> = Vec::new();
    let mut registry = String::from("pub(crate) static COLORMAPS: &[ColormapEntry] = &[\n");
    let mut default_idx: Option<usize> = None;

    for (idx, path) in files.iter().enumerate() {
        println!("cargo:rerun-if-changed={}", path.display());

        let category = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| panic!("{}: file name is not valid UTF-8", path.display()));
        let (base_name, cmap_type) = split_type_suffix(path, stem);
        let full_name = format!("{category}/{base_name}");

        // Entries are pushed one per file, in order, so `idx` is the index
        // this entry will have in COLORMAPS. Keep the first match.
        if full_name == DEFAULT_CMAP_NAME && default_idx.is_none() {
            default_idx = Some(idx);
        }

        let text = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        let parsed =
            parse_cpt(&text).unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()));

        let lut = resample(&parsed.segments, LUT_LEN);
        let offset = blob.len() / 4;
        blob.extend_from_slice(&lut);

        registry.push_str(&format!(
            "    ColormapEntry {{ name: \"{full_name}\", cmap_type: ColorMapType::{cmap_type}, \
             offset: {offset}, len: {LUT_LEN}, below: {below:?}, above: {above:?}, nan: {nan:?} }},\n",
            below = parsed.below,
            above = parsed.above,
            nan = parsed.nan,
        ));
    }

    registry.push_str("];\n");

    let default_idx = default_idx.unwrap_or_else(|| {
        panic!(
            "default colormap `{DEFAULT_CMAP_NAME}` not found among {} .cpt files in {}",
            files.len(),
            root.display()
        )
    });
    registry.push_str(&format!(
        "\npub(crate) const DEFAULT_COLORMAP_IDX: usize = {default_idx};\n\
         const _: () = assert!(DEFAULT_COLORMAP_IDX < COLORMAPS.len());\n"
    ));

    fs::write(out_dir.join("colormaps.bin"), &blob).unwrap();
    fs::write(out_dir.join("colormaps_registry.rs"), registry).unwrap();
}

// ---------------------------------------------------------------------------
// File discovery
// ---------------------------------------------------------------------------

fn is_cpt_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("cpt"))
}

/// Recursively collects `.cpt` files. Symlinked directories are not followed
/// (no infinite loops); symlinked files are accepted.
fn walk_cpt_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            let hidden = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with('.'));
            if hidden {
                continue; // .venv, .ruff_cache, ...
            }
            out.extend(walk_cpt_files(&path));
        } else if is_cpt_extension(&path) && path.is_file() {
            // `path.is_file()` follows symlinks, so a link to a file is kept
            out.push(path);
        }
    }
    out
}

fn split_type_suffix(path: &Path, stem: &str) -> (String, &'static str) {
    for (suffix, variant) in [
        ("_sequential", "Sequential"),
        ("_divergent", "Divergent"),
        ("_cyclic", "Cyclic"),
        ("_other", "Other"),
    ] {
        if let Some(base) = stem.strip_suffix(suffix) {
            return (base.to_string(), variant);
        }
    }
    panic!(
        "{}: no recognized _sequential/_divergent/_cyclic/_other suffix",
        path.display()
    );
}

// ---------------------------------------------------------------------------
// CPT parsing
// ---------------------------------------------------------------------------

fn parse_cpt(text: &str) -> Result<ParsedCpt, String> {
    let mut segments = Vec::new();
    // Explicit B / F lines win; otherwise the first / last segment colors are used.
    let mut below: Option<[u8; 4]> = None;
    let mut above: Option<[u8; 4]> = None;
    let mut nan = [0, 0, 0, 0];

    for (lineno, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(comment) = line.strip_prefix('#') {
            // GMT declares the model in a comment: "# COLOR_MODEL = HSV".
            // Parsing HSV as RGB would silently produce wrong colors.
            let c = comment.to_ascii_uppercase();
            if c.contains("COLOR_MODEL") && c.contains("HSV") {
                return Err(format!(
                    "line {}: HSV color model is not supported",
                    lineno + 1
                ));
            }
            continue;
        }

        let mut tokens: Vec<&str> = line.split_whitespace().collect();

        // Optional trailing annotation flag (A/L/U/B), only after a complete
        // segment line (4 or 8 fields + flag), so B/F/N lines are never altered.
        let has_flag = matches!(tokens.len(), 5 | 9)
            && tokens
                .last()
                .is_some_and(|l| l.len() <= 2 && l.chars().all(|c| c.is_ascii_alphabetic()));
        if has_flag {
            tokens.pop();
        }

        let err = |e: String| format!("line {}: {e}", lineno + 1);
        match tokens.first().copied() {
            Some("B") => below = Some(parse_color_tokens(&tokens[1..]).map_err(err)?),
            Some("F") => above = Some(parse_color_tokens(&tokens[1..]).map_err(err)?),
            Some("N") => nan = parse_color_tokens(&tokens[1..]).map_err(err)?,
            _ => segments.push(parse_segment(&tokens).map_err(err)?),
        }
    }

    if segments.is_empty() {
        return Err("no color segments found".into());
    }

    // Each segment must be a proper, finite, increasing range.
    if let Some(s) = segments.iter().find(|s| s.z1 <= s.z0) {
        return Err(format!("segment {}..{} is empty or reversed", s.z0, s.z1));
    }

    segments.sort_by(|a, b| a.z0.total_cmp(&b.z0));

    // Segments must join up: a gap or overlap would make lookups silently wrong.
    let z_min = segments.first().unwrap().z0;
    let z_max = segments.last().unwrap().z1;
    let tolerance = (z_max - z_min) * SEGMENT_JOIN_TOLERANCE;
    for w in segments.windows(2) {
        if (w[1].z0 - w[0].z1).abs() > tolerance {
            return Err(format!(
                "gap or overlap between segments ending at {} and starting at {}",
                w[0].z1, w[1].z0
            ));
        }
    }

    let below = below.unwrap_or(segments.first().unwrap().c0);
    let above = above.unwrap_or(segments.last().unwrap().c1);

    Ok(ParsedCpt {
        segments,
        below,
        above,
        nan,
    })
}

fn parse_z(tok: &str, what: &str) -> Result<f32, String> {
    let v: f32 = tok.parse().map_err(|_| format!("bad {what} '{tok}'"))?;
    if v.is_finite() {
        Ok(v)
    } else {
        Err(format!("non-finite {what} '{tok}'"))
    }
}

fn parse_segment(tokens: &[&str]) -> Result<Segment, String> {
    match tokens.len() {
        // z0 r0 g0 b0 z1 r1 g1 b1
        8 => Ok(Segment {
            z0: parse_z(tokens[0], "z0")?,
            c0: rgb_triplet(&tokens[1..4])?,
            z1: parse_z(tokens[4], "z1")?,
            c1: rgb_triplet(&tokens[5..8])?,
        }),
        // z0 color0 z1 color1  (hex / slash / grey)
        4 => Ok(Segment {
            z0: parse_z(tokens[0], "z0")?,
            c0: parse_color_tokens(&tokens[1..2])?,
            z1: parse_z(tokens[2], "z1")?,
            c1: parse_color_tokens(&tokens[3..4])?,
        }),
        n => Err(format!("unexpected token count {n} in color line")),
    }
}

fn parse_color_tokens(tokens: &[&str]) -> Result<[u8; 4], String> {
    match tokens.len() {
        1 => parse_color_token(tokens[0]),
        3 => rgb_triplet(tokens),
        n => Err(format!("unexpected color token count {n}")),
    }
}

/// Parses one 0..=255 component, rounding (not truncating) and clamping.
fn parse_component(tok: &str, what: &str) -> Result<u8, String> {
    let v: f32 = tok.parse().map_err(|_| format!("bad {what} '{tok}'"))?;
    if !v.is_finite() {
        return Err(format!("non-finite {what} '{tok}'"));
    }
    Ok(v.round().clamp(0.0, 255.0) as u8)
}

fn rgb_triplet(tokens: &[&str]) -> Result<[u8; 4], String> {
    let r = parse_component(tokens[0], "r")?;
    let g = parse_component(tokens[1], "g")?;
    let b = parse_component(tokens[2], "b")?;
    Ok([r, g, b, 255])
}

fn parse_color_token(tok: &str) -> Result<[u8; 4], String> {
    if let Some(hex) = tok.strip_prefix('#') {
        // `#fff` would parse as 0x000fff (a dark blue), so require 6 digits.
        if hex.len() != 6 {
            return Err(format!("bad hex color '{tok}' (need 6 hex digits)"));
        }
        let v = u32::from_str_radix(hex, 16).map_err(|_| format!("bad hex color '{tok}'"))?;
        return Ok([
            ((v >> 16) & 0xff) as u8,
            ((v >> 8) & 0xff) as u8,
            (v & 0xff) as u8,
            255,
        ]);
    }
    if tok.contains('/') {
        let parts: Vec<&str> = tok.split('/').collect();
        if parts.len() == 3 {
            return rgb_triplet(&parts);
        }
        return Err(format!("bad slash color '{tok}'"));
    }
    if tok.parse::<f32>().is_ok() {
        let g = parse_component(tok, "grey level")?;
        return Ok([g, g, g, 255]);
    }
    Err(format!(
        "unsupported color '{tok}' (expected #rrggbb, r/g/b, or grey level)"
    ))
}

// ---------------------------------------------------------------------------
// LUT generation
// ---------------------------------------------------------------------------

/// Samples the palette at `n` evenly spaced z values. Segments are half-open
/// `[z0, z1)`, except the last one which is closed, so stepped palettes
/// (`c0 == c1`) pick the right step exactly on a boundary.
fn resample(segments: &[Segment], n: usize) -> Vec<u8> {
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return segments.first().unwrap().c0.to_vec();
    }

    let z_min = segments.first().unwrap().z0;
    let z_max = segments.last().unwrap().z1;
    let mut out = vec![0u8; n * 4];
    let mut seg_idx = 0;

    for i in 0..n {
        let t = i as f32 / (n - 1) as f32;
        let z = z_min + t * (z_max - z_min);
        while seg_idx + 1 < segments.len() && z >= segments[seg_idx].z1 {
            seg_idx += 1;
        }
        let seg = &segments[seg_idx];
        // Segments are validated as non-empty, so span > 0.
        let span = seg.z1 - seg.z0;
        let local_t = ((z - seg.z0) / span).clamp(0.0, 1.0);
        for c in 0..4 {
            let a = seg.c0[c] as f32;
            let b = seg.c1[c] as f32;
            out[i * 4 + c] = (a + (b - a) * local_t).round() as u8;
        }
    }
    out
}
