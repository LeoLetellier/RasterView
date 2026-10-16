use std::{
    fs, io,
    path::{self, Path, PathBuf},
    time::SystemTime,
};

const VSIMEM_PREFIX: &str = "/vsimem/";

#[derive(Debug)]
pub enum SourceMetadata {
    File(FileMetadata),
    Vrt(VrtMetadata),
}

#[derive(Debug, Clone)]
pub struct VrtMetadata {
    pub vsi_path: String, // e.g. "/vsimem/abc123.vrt"
}

#[derive(Debug, Default)]
pub struct FileMetadata {
    pub path: PathBuf,
    pub name: String,
    pub extension: Option<String>,
    pub size: u64,
    pub is_symlink: bool,
    pub readonly: bool,
    pub hidden: bool,
    pub mode: Option<u32>,
    pub modified: Option<SystemTime>,
    pub created: Option<SystemTime>,
    pub accessed: Option<SystemTime>,
}

impl SourceMetadata {
    pub fn from_path(path: &Path) -> io::Result<Self> {
        // Check this first: fs calls and path::absolute would mangle or reject a /vsimem/ path.
        if let Some(s) = path.to_str()
            && is_vsimem(s) {
                return Ok(Self::Vrt(VrtMetadata {
                    vsi_path: s.replace('\\', "/"),
                }));
            }
        FileMetadata::from_path(path).map(Self::File)
    }
}

fn is_vsimem(s: &str) -> bool {
    // tolerate backslashes in case the path went through a Windows PathBuf
    s.replace('\\', "/").starts_with(VSIMEM_PREFIX)
}

impl FileMetadata {
    pub fn from_path(path: &Path) -> io::Result<Self> {
        let is_symlink = fs::symlink_metadata(path)?.file_type().is_symlink();
        let meta = fs::metadata(path)?; // follows symlinks
        if !meta.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a regular file",
            ));
        }

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        Ok(Self {
            path: path::absolute(path)?,
            extension: path.extension().map(|e| e.to_string_lossy().to_lowercase()),
            size: meta.len(),
            is_symlink,
            readonly: meta.permissions().readonly(),
            hidden: is_hidden(&name, &meta),
            mode: unix_mode(&meta),
            modified: meta.modified().ok(),
            created: meta.created().ok(),
            accessed: meta.accessed().ok(),
            name,
        })
    }
}

#[cfg(unix)]
fn is_hidden(name: &str, _m: &fs::Metadata) -> bool {
    name.starts_with('.')
}

#[cfg(windows)]
fn is_hidden(_name: &str, m: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    m.file_attributes() & 0x2 != 0
}

#[cfg(not(any(unix, windows)))]
fn is_hidden(name: &str, _m: &fs::Metadata) -> bool {
    name.starts_with('.')
}

#[cfg(unix)]
fn unix_mode(m: &fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    Some(m.permissions().mode() & 0o777)
}

#[cfg(not(unix))]
fn unix_mode(_m: &fs::Metadata) -> Option<u32> {
    None
}
