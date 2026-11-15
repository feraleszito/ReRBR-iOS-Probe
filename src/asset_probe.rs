//! Read-only Harwood Forest audit, using the original reRBR parsers.
//!
//! Texture request rules follow rbr-render/src/stage/textures.rs and texture
//! references follow stage/mesh.rs. This module does not build or upload meshes.

use rerbr_asset_formats::{dds, ini, lbs, rbz, stage_textures};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

pub const STAGE: &str = "track-20_O";
pub const EXPECTED_BLOCKS: usize = 282;
const MIB: u64 = 1024 * 1024;

/// Runs once on a background thread after GPU creation. Missing, partial or
/// unreadable private data never changes the scene or prevents app startup.
pub(crate) fn start_from_documents(bc_supported: bool, bc_enabled: bool, max_dimension: u32) {
    static START: std::sync::Once = std::sync::Once::new();
    START.call_once(|| {
        eprintln!("harwood: adapter BC supported={bc_supported}; device BC enabled={bc_enabled}; texture limit={max_dimension}");
        eprintln!("harwood: capability check only; no asset GPU upload or BC feature request");
        let Some(documents) = std::env::var_os("RERBR_PROBE_DOCUMENTS") else {
            eprintln!("harwood: SKIP: native Documents path was not supplied");
            return;
        };
        let maps = PathBuf::from(documents).join("Harwood").join("Maps");
        if !maps.is_dir() {
            eprintln!("harwood: SKIP: copy private files to Documents/Harwood/Maps, then relaunch");
            return;
        }
        if let Err(error) = std::thread::Builder::new().name("harwood-audit".into()).spawn(move || {
            match inspect(&maps) {
                Ok(report) => {
                    eprintln!("{report}");
                    let issues = report.gpu_issues(bc_supported, max_dimension);
                    for issue in &issues {
                        eprintln!("harwood: GPU capability warning: {issue}");
                    }
                    if report.complete() && issues.is_empty() {
                        eprintln!("harwood: parsed data fits advertised capabilities; upload/rendering UNTESTED");
                    }
                }
                Err(error) => eprintln!("harwood: FAIL: {error}"),
            }
        }) {
            eprintln!("harwood: could not start diagnostic thread: {error}");
        }
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TextureKind {
    Diffuse,
    Specular,
}

impl TextureKind {
    fn prefix(self) -> &'static str {
        match self {
            Self::Diffuse => "Texture",
            Self::Specular => "SpecularTexture",
        }
    }
}

#[derive(Debug)]
pub struct TextureRecord {
    pub kind: TextureKind,
    pub index: u32,
    pub required: bool,
    pub result: Result<TextureInfo, String>,
}

#[derive(Debug)]
pub struct TextureInfo {
    pub request: String,
    pub resolved: String,
    pub width: u32,
    pub height: u32,
    pub mip_count: u32,
    pub format: dds::Format,
}

#[derive(Debug)]
pub struct Report {
    pub blocks: usize,
    pub visibility_cells: usize,
    pub object_items: usize,
    pub declared_diffuse: u32,
    pub declared_specular: u32,
    pub textures: Vec<TextureRecord>,
    pub issues: Vec<String>,
}

impl Report {
    /// Required references and expected Harwood geometry must pass. Unused INI
    /// entries are also audited, but an unused missing entry remains a warning.
    pub fn complete(&self) -> bool {
        self.blocks == EXPECTED_BLOCKS
            && self.visibility_cells == EXPECTED_BLOCKS
            && self.issues.is_empty()
            && self
                .textures
                .iter()
                .filter(|t| t.required)
                .all(|t| t.result.is_ok())
    }

    pub fn redirected_to_n(&self) -> usize {
        self.textures
            .iter()
            .filter_map(|t| t.result.as_ref().ok())
            .filter(|t| t.resolved.starts_with("track-20_n_textures\\"))
            .count()
    }

    /// A capability audit, not a texture upload or proof of rendered correctness.
    pub fn gpu_issues(&self, bc_supported: bool, max_dimension: u32) -> Vec<String> {
        let images: Vec<_> = self
            .textures
            .iter()
            .filter(|t| t.required)
            .filter_map(|t| t.result.as_ref().ok())
            .collect();
        let mut issues = Vec::new();
        if !bc_supported && images.iter().any(|t| t.format != dds::Format::Rgba8) {
            issues.push("required DDS images need TEXTURE_COMPRESSION_BC".into());
        }
        for image in images {
            if image.width.max(image.height) > max_dimension {
                issues.push(format!(
                    "{}: {}x{} exceeds adapter limit {max_dimension}",
                    image.resolved, image.width, image.height
                ));
            }
        }
        issues
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let required: Vec<_> = self.textures.iter().filter(|t| t.required).collect();
        let required_ok = required.iter().filter(|t| t.result.is_ok()).count();
        let declared_ok = self.textures.iter().filter(|t| t.result.is_ok()).count();
        writeln!(f, "harwood: {STAGE}; dry/new; CPU audit only")?;
        writeln!(
            f,
            "harwood: blocks={} (expected {EXPECTED_BLOCKS}), visibility={}, object items={}",
            self.blocks, self.visibility_cells, self.object_items
        )?;
        writeln!(
            f,
            "harwood: INI diffuse={}, specular={}; required DDS={required_ok}/{}; declared DDS={declared_ok}/{}; redirected to N={}",
            self.declared_diffuse, self.declared_specular, required.len(), self.textures.len(),
            self.redirected_to_n()
        )?;
        for texture in &self.textures {
            let key = format!("{}{}", texture.kind.prefix(), texture.index);
            let usage = if texture.required {
                "required"
            } else {
                "unused"
            };
            match &texture.result {
                Ok(image) => writeln!(
                    f,
                    "harwood: {key} [{usage}] {} -> {} | {:?} {}x{} mips={}",
                    image.request,
                    image.resolved,
                    image.format,
                    image.width,
                    image.height,
                    image.mip_count
                )?,
                Err(error) => writeln!(f, "harwood: {key} [{usage}] ERROR: {error}")?,
            }
        }
        for issue in &self.issues {
            writeln!(f, "harwood: ERROR: {issue}")?;
        }
        write!(
            f,
            "harwood: required asset diagnostic {}",
            if self.complete() { "PASS" } else { "FAIL" }
        )
    }
}

/// Reads only files in the supplied Maps folder. No extraction or output files.
pub fn inspect(maps: &Path) -> Result<Report, String> {
    let files = Files::new(maps)?;
    // Drop the LBS buffer and borrowed parse tree before reading texture images.
    let (blocks, visibility_cells, object_items, used) = {
        let bytes = read_limited(files.find("track-20_O.lbs")?, 96 * MIB)?;
        let stage = lbs::parse(&bytes).map_err(|e| format!("{STAGE}.lbs: {e}"))?;
        references(&stage)?
    };
    let ini_bytes = read_limited(files.find("track-20_O.ini")?, 2 * MIB)?;
    let sections = ini::parse(&String::from_utf8_lossy(&ini_bytes)).map_err(|e| e.to_string())?;
    let info = section(&sections, "TextureInfo").ok_or("missing [TextureInfo]")?;
    let declared_diffuse = count(info, "NumTextures")?;
    let declared_specular = count(info, "NumSpecularTextures")?;
    let mut report = Report {
        blocks,
        visibility_cells,
        object_items,
        declared_diffuse,
        declared_specular,
        textures: Vec::new(),
        issues: Vec::new(),
    };
    if count(info, "NumShadowTextures")? != 0 {
        report
            .issues
            .push("this Harwood diagnostic expects no shadow textures".into());
    }
    let bytes = read_limited(files.find("TextureFilenameMap.dat")?, 8 * MIB)?;
    let remap = stage_textures::FilenameMap::parse(&bytes).map_err(|e| e.to_string())?;
    let mut archives = Archives {
        files: &files,
        open: BTreeMap::new(),
    };
    for (kind, count) in [
        (TextureKind::Diffuse, declared_diffuse),
        (TextureKind::Specular, declared_specular),
    ] {
        for index in 0..count {
            report.textures.push(TextureRecord {
                kind,
                index,
                required: used.contains(&(kind, index)),
                result: texture(&mut archives, &remap, &sections, info, kind, index),
            });
        }
    }
    for (kind, index) in used {
        let count = match kind {
            TextureKind::Diffuse => declared_diffuse,
            TextureKind::Specular => declared_specular,
        };
        if index >= count {
            report.issues.push(format!(
                "{}{} is referenced but not declared",
                kind.prefix(),
                index
            ));
        }
    }
    Ok(report)
}

type References = BTreeSet<(TextureKind, u32)>;

fn references(stage: &lbs::Lbs<'_>) -> Result<(usize, usize, usize, References), String> {
    let mut used = BTreeSet::new();
    let (mut blocks, mut cells, mut objects, mut geometry_chunks) = (0, 0, 0, 0);
    for chunk in &stage.chunks {
        match chunk {
            lbs::Chunk::Geometry(geometry) => {
                geometry_chunks += 1;
                blocks += geometry.blocks.len();
                for draw in geometry.blocks.iter().flat_map(|b| &b.chunks) {
                    used.extend(draw.textures.iter().map(|&i| (TextureKind::Diffuse, i)));
                    if matches!(draw.view, 6 | 8) && draw.specular {
                        used.insert((TextureKind::Specular, draw.specular_texture));
                    }
                }
            }
            lbs::Chunk::Visibility(v) => cells += v.entries.len(),
            lbs::Chunk::Objects(o) => {
                for item in o.lanes.iter().flat_map(|l| l.runs.iter().flatten()) {
                    objects += 1;
                    used.extend(item.texture_index().map(|i| (TextureKind::Diffuse, i)));
                }
            }
            lbs::Chunk::Interactive(o) => {
                // stage/mesh.rs uses texture0; texture1 is audited conservatively too.
                for draw in o.objects.iter().flat_map(|o| &o.draws) {
                    used.extend(
                        [draw.texture0, draw.texture1]
                            .into_iter()
                            .flatten()
                            .map(|i| (TextureKind::Diffuse, i)),
                    );
                }
            }
            lbs::Chunk::Sky(sky) => {
                for item in &sky.items {
                    used.extend(
                        [item.texture0, item.texture1]
                            .into_iter()
                            .flatten()
                            .map(|i| (TextureKind::Diffuse, i)),
                    );
                }
            }
            _ => {}
        }
    }
    if geometry_chunks != 1 {
        return Err(format!(
            "expected one geometry chunk, found {geometry_chunks}"
        ));
    }
    Ok((blocks, cells, objects, used))
}

fn section<'a>(sections: &'a [ini::Section], name: &str) -> Option<&'a ini::Section> {
    sections.iter().find(|s| s.name.eq_ignore_ascii_case(name))
}

fn count(info: &ini::Section, key: &str) -> Result<u32, String> {
    let n = info
        .get(key)
        .ok_or_else(|| format!("missing {key}"))?
        .parse::<u32>()
        .map_err(|e| format!("{key}: {e}"))?;
    if n > 4096 {
        return Err(format!("{key}: excessive count {n}"));
    }
    Ok(n)
}

fn texture(
    archives: &mut Archives<'_>,
    remap: &stage_textures::FilenameMap,
    sections: &[ini::Section],
    info: &ini::Section,
    kind: TextureKind,
    index: u32,
) -> Result<TextureInfo, String> {
    let key = format!("{}{index}", kind.prefix());
    let name = info.get(&key).ok_or_else(|| format!("missing {key}"))?;
    let ground = section(sections, name)
        .and_then(|s| s.get("IsGroundTexture"))
        .is_some_and(|s| s.eq_ignore_ascii_case("true"));
    let requests = match kind {
        TextureKind::Diffuse => vec![stage_textures::texture_request(
            STAGE, name, ground, "dry", "new",
        )],
        TextureKind::Specular => {
            stage_textures::specular_requests(STAGE, name, "dry", "new").to_vec()
        }
    };
    let mut errors = String::new();
    for request in requests {
        let resolved = remap.resolve(&request);
        match archives.image(&resolved) {
            Ok(bytes) => match dds::parse(&bytes) {
                Ok(image) => {
                    return Ok(TextureInfo {
                        request,
                        resolved: normalized(&resolved)?,
                        width: image.width,
                        height: image.height,
                        mip_count: image.mip_count,
                        format: image.format,
                    })
                }
                Err(error) => {
                    let _ = write!(errors, "{resolved}: {error}; ");
                }
            },
            Err(error) => {
                let _ = write!(errors, "{resolved}: {error}; ");
            }
        }
    }
    Err(errors)
}

struct Files(BTreeMap<String, PathBuf>);

impl Files {
    fn new(maps: &Path) -> Result<Self, String> {
        let mut files = BTreeMap::new();
        for entry in std::fs::read_dir(maps).map_err(|e| format!("{}: {e}", maps.display()))? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_type().map_err(|e| e.to_string())?.is_file() {
                files.insert(
                    entry.file_name().to_string_lossy().to_ascii_lowercase(),
                    entry.path(),
                );
            }
        }
        Ok(Self(files))
    }

    fn find(&self, name: &str) -> Result<&Path, String> {
        self.0
            .get(&name.to_ascii_lowercase())
            .map(PathBuf::as_path)
            .ok_or_else(|| format!("missing Maps/{name}"))
    }
}

fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    read_stream(file, limit).map_err(|e| format!("{}: {e}", path.display()))
}

fn read_stream(reader: impl Read, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err(format!("file exceeds diagnostic limit {limit} bytes"));
    }
    Ok(bytes)
}

fn normalized(path: &str) -> Result<String, String> {
    if path.len() > 1024
        || path
            .split(['/', '\\'])
            .any(|s| s.is_empty() || s == "." || s == ".." || s.contains(':'))
    {
        return Err("invalid archive path (absolute paths and traversal are not accepted)".into());
    }
    Ok(path.replace('/', "\\").to_ascii_lowercase())
}

struct Archives<'a> {
    files: &'a Files,
    open: BTreeMap<String, rbz::ZipArchive<BufReader<File>>>,
}

impl Archives<'_> {
    fn image(&mut self, request: &str) -> Result<Vec<u8>, String> {
        let request = normalized(request)?;
        let (name, _) =
            stage_textures::split_archive_path(&request).ok_or("no archive in texture path")?;
        if !self.open.contains_key(&name) {
            let archive = rbz::open(self.files.find(&name)?).map_err(|e| e.to_string())?;
            self.open.insert(name.clone(), archive);
        }
        let archive = self.open.get_mut(&name).ok_or("archive was not opened")?;
        // Keep the archive-prefix directory in the entry, as the real RBZs do.
        // The original helper ignores ASCII case and slash direction. Reading to
        // EOF verifies ZIP CRC; nothing is extracted to disk.
        let entry = rbz::by_name(archive, &request).map_err(|e| e.to_string())?;
        if entry.size() > 32 * MIB {
            return Err("inflated DDS exceeds diagnostic limit".into());
        }
        read_stream(entry, 32 * MIB)
    }
}
