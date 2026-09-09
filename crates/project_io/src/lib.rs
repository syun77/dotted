use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use pixel_core::{PixelGrid, Rgba8, Size};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{Cursor, Read, Seek, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;
use thiserror::Error;
use training::{ArtworkId, Project, ReferenceId};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

const MAX_ARCHIVE: u64 = 80 * 1024 * 1024;
const MAX_MANIFEST: u64 = 1024 * 1024;
const MAX_ENTRIES: usize = 4;

#[derive(Debug, Error)]
pub enum ProjectIoError {
    #[error("I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid .dotted container: {0}")]
    Invalid(String),
    #[error("JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("image failed: {0}")]
    Image(#[from] image::ImageError),
    #[error("ZIP failed: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("history index failed: {0}")]
    Sql(#[from] rusqlite::Error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportScale {
    Original,
    X2,
    X4,
    X8,
}
impl ExportScale {
    const fn factor(self) -> u32 {
        match self {
            Self::Original => 1,
            Self::X2 => 2,
            Self::X4 => 4,
            Self::X8 => 8,
        }
    }
}

pub fn save_project(path: &Path, project: &Project) -> Result<(), ProjectIoError> {
    validate_project(project)?;
    let parent = path
        .parent()
        .ok_or_else(|| ProjectIoError::Invalid("save destination has no parent".into()))?;
    fs::create_dir_all(parent)?;
    let mut temp = NamedTempFile::new_in(parent)?;
    write_archive(temp.as_file_mut(), project)?;
    temp.as_file_mut().flush()?;
    temp.as_file().sync_all()?;
    temp.persist(path)
        .map_err(|e| ProjectIoError::Io(e.error))?;
    sync_parent(parent)?;
    Ok(())
}

fn write_archive<W: Write + Seek>(writer: W, project: &Project) -> Result<(), ProjectIoError> {
    let mut zip = ZipWriter::new(writer);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o600);
    let mut manifest = serde_json::to_value(project)?;
    manifest
        .as_object_mut()
        .ok_or_else(|| ProjectIoError::Invalid("project is not an object".into()))?
        .insert("format_version".into(), json!(1));
    let artworks = manifest["artworks"]
        .as_array_mut()
        .ok_or_else(|| ProjectIoError::Invalid("artworks missing".into()))?;
    for (value, artwork) in artworks.iter_mut().zip(&project.artworks) {
        let path = artwork_path(
            artwork.id,
            artwork.document.layer_id.0,
            artwork.document.frame_id.0,
        );
        let png = encode_png(artwork.document.grid(), 1)?;
        let hash = sha256(&png);
        let doc = value["document"]
            .as_object_mut()
            .ok_or_else(|| ProjectIoError::Invalid("document missing".into()))?;
        doc.remove("grid");
        doc.insert("cel".into(), json!({"path": path, "sha256": hash}));
        zip.start_file(&path, options)?;
        zip.write_all(&png)?;
    }
    let references = manifest["references"]
        .as_array_mut()
        .ok_or_else(|| ProjectIoError::Invalid("references missing".into()))?;
    for (value, reference) in references.iter_mut().zip(&project.references) {
        let path = reference_path(reference.id);
        let png = encode_png(&reference.image, 1)?;
        let hash = sha256(&png);
        let obj = value
            .as_object_mut()
            .ok_or_else(|| ProjectIoError::Invalid("reference missing".into()))?;
        obj.remove("image");
        obj.insert(
            "embedded_image".into(),
            json!({"size": reference.image.size(), "path": path, "sha256": hash}),
        );
        zip.start_file(&path, options)?;
        zip.write_all(&png)?;
    }
    let bytes = serde_json::to_vec_pretty(&manifest)?;
    if bytes.len() as u64 > MAX_MANIFEST {
        return Err(ProjectIoError::Invalid("manifest exceeds 1 MiB".into()));
    }
    zip.start_file("manifest.json", options)?;
    zip.write_all(&bytes)?;
    zip.finish()?;
    Ok(())
}

pub fn load_project(path: &Path) -> Result<Project, ProjectIoError> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > MAX_ARCHIVE {
        return Err(ProjectIoError::Invalid("archive exceeds 80 MiB".into()));
    }
    let file = File::open(path)?;
    let mut zip = ZipArchive::new(file)?;
    if zip.len() > MAX_ENTRIES {
        return Err(ProjectIoError::Invalid("too many ZIP entries".into()));
    }
    let mut names = HashSet::new();
    for i in 0..zip.len() {
        let entry = zip.by_index(i)?;
        let name = entry.name().to_owned();
        if !names.insert(name.clone()) || !valid_path(&name) || entry.is_dir() {
            return Err(ProjectIoError::Invalid(format!(
                "invalid ZIP entry: {name}"
            )));
        }
    }
    if !names.contains("manifest.json") {
        return Err(ProjectIoError::Invalid("manifest missing".into()));
    }
    let mut bytes = Vec::new();
    zip.by_name("manifest.json")?
        .take(MAX_MANIFEST + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MANIFEST {
        return Err(ProjectIoError::Invalid("manifest exceeds 1 MiB".into()));
    }
    let mut manifest: Value = serde_json::from_slice(&bytes)?;
    if manifest.get("format_version") != Some(&json!(1)) {
        return Err(ProjectIoError::Invalid("unsupported format version".into()));
    }
    manifest
        .as_object_mut()
        .expect("checked object")
        .remove("format_version");
    let artworks = manifest["artworks"]
        .as_array_mut()
        .ok_or_else(|| ProjectIoError::Invalid("artworks missing".into()))?;
    for artwork in artworks {
        let cel = artwork["document"]
            .as_object_mut()
            .and_then(|d| d.remove("cel"))
            .ok_or_else(|| ProjectIoError::Invalid("cel missing".into()))?;
        let expected = cel["path"]
            .as_str()
            .ok_or_else(|| ProjectIoError::Invalid("cel path missing".into()))?;
        let id: ArtworkId = serde_json::from_value(artwork["id"].clone())?;
        let layer = artwork["document"]["layer_id"]
            .as_str()
            .ok_or_else(|| ProjectIoError::Invalid("layer id missing".into()))?;
        let frame = artwork["document"]["frame_id"]
            .as_str()
            .ok_or_else(|| ProjectIoError::Invalid("frame id missing".into()))?;
        if expected
            != artwork_path(
                id,
                layer
                    .parse()
                    .map_err(|_| ProjectIoError::Invalid("invalid layer id".into()))?,
                frame
                    .parse()
                    .map_err(|_| ProjectIoError::Invalid("invalid frame id".into()))?,
            )
        {
            return Err(ProjectIoError::Invalid("non-canonical cel path".into()));
        }
        let png = read_entry(&mut zip, expected)?;
        verify_hash(&png, &cel["sha256"])?;
        let size: Size = serde_json::from_value(artwork["document"]["size"].clone())?;
        let grid = decode_png(&png, Some(size))?;
        artwork["document"]
            .as_object_mut()
            .expect("document object")
            .insert("grid".into(), serde_json::to_value(grid)?);
    }
    let references = manifest["references"]
        .as_array_mut()
        .ok_or_else(|| ProjectIoError::Invalid("references missing".into()))?;
    for reference in references {
        let embedded = reference
            .as_object_mut()
            .and_then(|o| o.remove("embedded_image"))
            .ok_or_else(|| ProjectIoError::Invalid("embedded reference missing".into()))?;
        let id: ReferenceId = serde_json::from_value(reference["id"].clone())?;
        let expected = embedded["path"]
            .as_str()
            .ok_or_else(|| ProjectIoError::Invalid("reference path missing".into()))?;
        if expected != reference_path(id) {
            return Err(ProjectIoError::Invalid(
                "non-canonical reference path".into(),
            ));
        }
        let png = read_entry(&mut zip, expected)?;
        verify_hash(&png, &embedded["sha256"])?;
        let size: Size = serde_json::from_value(embedded["size"].clone())?;
        let grid = decode_png(&png, Some(size))?;
        reference
            .as_object_mut()
            .expect("reference object")
            .insert("image".into(), serde_json::to_value(grid)?);
    }
    let expected: HashSet<_> = std::iter::once("manifest.json".to_owned())
        .chain(project_paths(&manifest)?)
        .collect();
    if names != expected {
        return Err(ProjectIoError::Invalid("unexpected ZIP entries".into()));
    }
    let project: Project = serde_json::from_value(manifest)?;
    validate_project(&project)?;
    Ok(project)
}

fn project_paths(value: &Value) -> Result<Vec<String>, ProjectIoError> {
    let mut paths = Vec::new();
    for a in value["artworks"]
        .as_array()
        .ok_or_else(|| ProjectIoError::Invalid("artworks missing".into()))?
    {
        let id: ArtworkId = serde_json::from_value(a["id"].clone())?;
        let layer = a["document"]["layer_id"]
            .as_str()
            .ok_or_else(|| ProjectIoError::Invalid("layer id missing".into()))?
            .parse()
            .map_err(|_| ProjectIoError::Invalid("invalid layer id".into()))?;
        let frame = a["document"]["frame_id"]
            .as_str()
            .ok_or_else(|| ProjectIoError::Invalid("frame id missing".into()))?
            .parse()
            .map_err(|_| ProjectIoError::Invalid("invalid frame id".into()))?;
        paths.push(artwork_path(id, layer, frame));
    }
    for r in value["references"]
        .as_array()
        .ok_or_else(|| ProjectIoError::Invalid("references missing".into()))?
    {
        paths.push(reference_path(serde_json::from_value(r["id"].clone())?));
    }
    Ok(paths)
}
fn read_entry<R: Read + Seek>(
    zip: &mut ZipArchive<R>,
    name: &str,
) -> Result<Vec<u8>, ProjectIoError> {
    let mut data = Vec::new();
    zip.by_name(name)?
        .take(MAX_ARCHIVE + 1)
        .read_to_end(&mut data)?;
    if data.len() as u64 > MAX_ARCHIVE {
        return Err(ProjectIoError::Invalid(
            "expanded entry exceeds limit".into(),
        ));
    }
    Ok(data)
}
fn valid_path(name: &str) -> bool {
    !name.starts_with('/')
        && !name.contains("..")
        && !name.contains('\\')
        && (name == "manifest.json"
            || name.starts_with("artworks/")
            || name.starts_with("references/"))
}
fn artwork_path(id: ArtworkId, layer: uuid::Uuid, frame: uuid::Uuid) -> String {
    format!(
        "artworks/{}/{}/{}.png",
        id.0.hyphenated(),
        layer.hyphenated(),
        frame.hyphenated()
    )
}
fn reference_path(id: ReferenceId) -> String {
    format!("references/{}.png", id.0.hyphenated())
}
fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn verify_hash(bytes: &[u8], value: &Value) -> Result<(), ProjectIoError> {
    if value.as_str() != Some(&sha256(bytes)) {
        Err(ProjectIoError::Invalid("PNG hash mismatch".into()))
    } else {
        Ok(())
    }
}

fn encode_png(grid: &PixelGrid, factor: u32) -> Result<Vec<u8>, ProjectIoError> {
    let size = grid.size();
    let w = size
        .width
        .checked_mul(factor)
        .ok_or_else(|| ProjectIoError::Invalid("scaled width overflow".into()))?;
    let h = size
        .height
        .checked_mul(factor)
        .ok_or_else(|| ProjectIoError::Invalid("scaled height overflow".into()))?;
    let mut image = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let p = grid.pixels()[(y / factor * size.width + x / factor) as usize];
            image.put_pixel(x, y, Rgba(p.bytes()));
        }
    }
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image).write_to(&mut out, ImageFormat::Png)?;
    Ok(out.into_inner())
}
fn decode_png(bytes: &[u8], expected: Option<Size>) -> Result<PixelGrid, ProjectIoError> {
    let image = image::load_from_memory_with_format(bytes, ImageFormat::Png)?.into_rgba8();
    let size = Size::new(image.width(), image.height())
        .map_err(|e| ProjectIoError::Invalid(e.to_string()))?;
    if expected.is_some_and(|e| e != size) {
        return Err(ProjectIoError::Invalid("PNG dimensions mismatch".into()));
    }
    let pixels = image
        .pixels()
        .map(|p| Rgba8::new(p.0[0], p.0[1], p.0[2], p.0[3]))
        .collect();
    PixelGrid::from_pixels(size, pixels).map_err(|e| ProjectIoError::Invalid(e.to_string()))
}

pub fn export_png(path: &Path, grid: &PixelGrid, scale: ExportScale) -> Result<(), ProjectIoError> {
    let bytes = encode_png(grid, scale.factor())?;
    fs::write(path, bytes)?;
    Ok(())
}
pub fn import_reference(path: &Path) -> Result<PixelGrid, ProjectIoError> {
    if fs::metadata(path)?.len() > 64 * 1024 * 1024 {
        return Err(ProjectIoError::Invalid("reference exceeds 64 MiB".into()));
    }
    let bytes = fs::read(path)?;
    let format = image::guess_format(&bytes)?;
    if !matches!(format, ImageFormat::Png | ImageFormat::Jpeg) {
        return Err(ProjectIoError::Invalid(
            "only static PNG and JPEG are supported".into(),
        ));
    }
    let image = image::load_from_memory_with_format(&bytes, format)?.into_rgba8();
    if image.width() > 4096
        || image.height() > 4096
        || u64::from(image.width()) * u64::from(image.height()) > 16_777_216
    {
        return Err(ProjectIoError::Invalid(
            "reference dimensions exceed limit".into(),
        ));
    }
    let size = Size::new(image.width(), image.height())
        .map_err(|e| ProjectIoError::Invalid(e.to_string()))?;
    PixelGrid::from_pixels(
        size,
        image
            .pixels()
            .map(|p| Rgba8::new(p[0], p[1], p[2], p[3]))
            .collect(),
    )
    .map_err(|e| ProjectIoError::Invalid(e.to_string()))
}

pub fn save_recovery(
    root: &Path,
    generation: uuid::Uuid,
    revision: u64,
    project: &Project,
) -> Result<PathBuf, ProjectIoError> {
    let dir = root.join(generation.hyphenated().to_string());
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{revision}.dotted"));
    save_project(&path, project)?;
    for entry in fs::read_dir(&dir)? {
        let candidate = entry?.path();
        if candidate != path && candidate.extension().is_some_and(|e| e == "dotted") {
            let _ = fs::remove_file(candidate);
        }
    }
    Ok(path)
}

pub struct HistoryIndex {
    connection: Connection,
}
impl HistoryIndex {
    pub fn open(path: &Path) -> Result<Self, ProjectIoError> {
        let connection = Connection::open(path)?;
        connection.execute_batch("CREATE TABLE IF NOT EXISTS projects(path TEXT PRIMARY KEY, project_id TEXT NOT NULL, title TEXT NOT NULL, updated_at TEXT NOT NULL, subject TEXT NOT NULL, width INTEGER NOT NULL, height INTEGER NOT NULL, color_limit INTEGER NOT NULL, stage TEXT NOT NULL, status TEXT NOT NULL);")?;
        Ok(Self { connection })
    }
    pub fn record(&self, path: &Path, project: &Project) -> Result<(), ProjectIoError> {
        let s = project.session();
        self.connection.execute(
            "INSERT OR REPLACE INTO projects VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                path.to_string_lossy(),
                project.id.0.to_string(),
                project.title,
                project.updated_at.to_rfc3339(),
                s.subject,
                s.constraints.size.width,
                s.constraints.size.height,
                s.constraints.color_limit,
                format!("{:?}", s.stage),
                format!("{:?}", s.status)
            ],
        )?;
        Ok(())
    }
    pub fn list(&self) -> Result<Vec<HistoryEntry>, ProjectIoError> {
        let mut stmt = self.connection.prepare("SELECT path,title,subject,updated_at,stage,status FROM projects ORDER BY updated_at DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok(HistoryEntry {
                path: PathBuf::from(r.get::<_, String>(0)?),
                title: r.get(1)?,
                subject: r.get(2)?,
                updated_at: r.get(3)?,
                stage: r.get(4)?,
                status: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}
#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub path: PathBuf,
    pub title: String,
    pub subject: String,
    pub updated_at: String,
    pub stage: String,
    pub status: String,
}

fn validate_project(project: &Project) -> Result<(), ProjectIoError> {
    if project.sessions.len() != 1 || project.references.len() != 1 || project.artworks.len() > 2 {
        return Err(ProjectIoError::Invalid("MVP cardinality violated".into()));
    }
    project
        .session()
        .constraints
        .validate()
        .map_err(|e| ProjectIoError::Invalid(e.to_string()))?;
    if project.session().events.len() > 256 {
        return Err(ProjectIoError::Invalid("event limit exceeded".into()));
    }
    for artwork in &project.artworks {
        if artwork.document.size != project.session().constraints.size {
            return Err(ProjectIoError::Invalid(
                "artwork size does not match constraints".into(),
            ));
        }
        if artwork
            .document
            .grid()
            .pixels()
            .iter()
            .any(|p| p.a == 0 && *p != Rgba8::TRANSPARENT)
        {
            return Err(ProjectIoError::Invalid(
                "transparent RGB is not normalized".into(),
            ));
        }
    }
    Ok(())
}
#[cfg(unix)]
fn sync_parent(parent: &Path) -> Result<(), ProjectIoError> {
    File::open(parent)?.sync_all()?;
    Ok(())
}
#[cfg(not(unix))]
fn sync_parent(_parent: &Path) -> Result<(), ProjectIoError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use pixel_core::{Palette, PaletteEntry, PaletteEntryId};
    use training::{Background, Constraints, Reference};
    fn project() -> Project {
        let size = Size::new(16, 16).unwrap();
        let palette = Palette::new(vec![
            PaletteEntry {
                id: PaletteEntryId(uuid::Uuid::new_v4()),
                color: Rgba8::BLACK,
            },
            PaletteEntry {
                id: PaletteEntryId(uuid::Uuid::new_v4()),
                color: Rgba8::new(255, 255, 255, 255),
            },
        ])
        .unwrap();
        Project::new(
            "test".into(),
            "apple".into(),
            Constraints {
                size,
                initial_palette: palette,
                color_limit: 4,
                time_limit_ms: 1000,
                background: Background::Checker,
            },
            Reference {
                id: ReferenceId::new(),
                image: PixelGrid::transparent(size),
                source_title: None,
                creator: None,
                source_url: None,
                usage_note: None,
            },
            Utc::now(),
        )
        .unwrap()
    }
    #[test]
    fn project_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.dotted");
        let p = project();
        save_project(&path, &p).unwrap();
        assert_eq!(load_project(&path).unwrap(), p);
    }
    #[test]
    fn png_exact_scale() {
        let size = Size::new(2, 1).unwrap();
        let grid = PixelGrid::from_pixels(
            size,
            vec![Rgba8::new(1, 2, 3, 127), Rgba8::new(4, 5, 6, 255)],
        )
        .unwrap();
        let bytes = encode_png(&grid, 4).unwrap();
        let decoded = decode_png(&bytes, None).unwrap();
        assert_eq!(decoded.size(), Size::new(8, 4).unwrap());
        assert_eq!(
            decoded.get(pixel_core::Coordinate { x: 0, y: 0 }).unwrap(),
            Rgba8::new(1, 2, 3, 127)
        );
    }
}
