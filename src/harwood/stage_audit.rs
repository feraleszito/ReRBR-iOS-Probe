//! Full stage compatibility auditor: inspects `.lbs` geometry blocks, far-LOD
//! blocks, driveline/COL alignment, `.dls` start/splits/finish, textures,
//! resident GPU budget, and (optionally) authentic 720 Hz `rerbr-sim` boot.
//! Exports machine-readable CSV and JSON reports without hiding errors.

use super::mesh;
use crate::asset_probe;
use rerbr_asset_formats::lbs;
use rerbr_content::{StageDescriptor, PHYSICS_STAGE_EXTENSIONS};
use rerbr_render_core::{coords, Vec3};
use rerbr_sim::rbr_formats::{
    cars,
    markers::{self, TAG_END, TAG_FINISH, TAG_SPLIT, TAG_START},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    path::Path,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageCompatStatus {
    /// Geometry, textures, driveline/COL agreement, `.dls` markers, budget, and sim boot all pass.
    FullyPlayable,
    /// Geometry, textures, and COL checks pass, but sim boot was not run or failed / non-preset.
    GeometryValidNotPlayable,
    /// Stage parses and loads partially (e.g. non-fatal far-only mismatch, budget or COL gap).
    PartiallyCompatible,
    /// Missing files, rejected/unsupported blocks, texture failures, or parse errors.
    Blocked,
}

impl StageCompatStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FullyPlayable => "FULLY_PLAYABLE",
            Self::GeometryValidNotPlayable => "GEOMETRY_OK_NOT_PLAYABLE",
            Self::PartiallyCompatible => "PARTIAL",
            Self::Blocked => "BLOCKED",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BlockAnomaly {
    pub block_id: usize,
    pub kind: String,
    pub near_triangles: usize,
    pub far_draws: usize,
    pub bad_triangles: usize,
    pub skipped_draws: usize,
    pub view_slots: Vec<(usize, usize)>,
    /// `(list, view, shadow, specular, prim_count, textures_len)`
    pub chunks: Vec<(u8, usize, bool, bool, u32, usize)>,
}

#[derive(Debug, Clone)]
pub struct StageAuditRow {
    pub stem: String,
    pub stage_id: String,
    pub stage_name: String,
    pub map_id: Option<u32>,
    pub variant: String,
    pub country: String,
    pub surface: String,
    pub is_preset: bool,
    pub preset_expected_blocks: Option<usize>,
    pub lbs_total_blocks: usize,
    pub near_loaded_blocks: usize,
    pub far_only_blocks: usize,
    pub empty_blocks: usize,
    pub rejected_blocks: usize,
    pub anomalies: Vec<BlockAnomaly>,
    pub missing_files: Vec<String>,
    pub has_start_location: bool,
    pub route_points: usize,
    pub supported_points: usize,
    pub collision_matched_points: usize,
    pub max_ground_gap_m: f32,
    pub dls_start_m: Option<f32>,
    pub dls_finish_m: Option<f32>,
    pub dls_end_m: Option<f32>,
    pub dls_splits: usize,
    pub dls_pacenotes: usize,
    pub textures_needed: usize,
    pub textures_loaded: usize,
    pub resident_bytes: usize,
    pub exceeds_budget: bool,
    pub stage_load_ok: Option<bool>,
    pub stage_load_error: Option<String>,
    pub sim_boot_ok: Option<bool>,
    pub sim_boot_error: Option<String>,
    pub status: StageCompatStatus,
    pub errors: Vec<String>,
}

pub fn parse_track_stem(stem: &str) -> (Option<u32>, String) {
    let lower = stem.to_ascii_lowercase();
    let Some(rest) = lower.strip_prefix("track-") else {
        return (None, String::new());
    };
    let mut parts = rest.splitn(2, '_');
    let map = parts.next().and_then(|n| n.parse::<u32>().ok());
    let variant = parts.next().unwrap_or("").to_ascii_uppercase();
    (map, variant)
}

fn country_for_map(map: Option<u32>) -> &'static str {
    match map {
        Some(10..=19) => "Finland",
        Some(20..=29) => "United Kingdom",
        Some(30..=39) => "Australia",
        Some(40..=49) => "France",
        Some(50..=59) => "Japan",
        Some(60..=69) => "United States",
        Some(70..=89) => "Rally School",
        Some(_) => "Custom/Other",
        None => "Utility",
    }
}

fn surface_for_map(map: Option<u32>) -> &'static str {
    match map {
        Some(10..=18) => "Snow",
        Some(41..=49 | 73 | 85 | 86) => "Tarmac",
        Some(20..=25 | 31..=36 | 51..=56 | 61..=66 | 71..=89) => "Gravel",
        _ => "Unknown",
    }
}

#[allow(dead_code)]
fn weather_letter_code(variant: &str) -> Option<u32> {
    match variant {
        "M" => Some(0),
        "N" => Some(1),
        "E" => Some(2),
        "O" => Some(3),
        "S" => Some(4),
        _ => None,
    }
}

pub fn load_rsf_stage_names(rbr_root: &Path) -> BTreeMap<u32, String> {
    let path = rbr_root.join("rsfdata/cache/stages_data.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    let mut out = BTreeMap::new();
    // Lightweight flat object scan without external JSON crates.
    for chunk in text.split('{').skip(1) {
        let Some(body) = chunk.split('}').next() else {
            continue;
        };
        let stage_id = extract_json_u32(body, "stage_id");
        let name = extract_json_str(body, "name");
        if let (Some(id), Some(name)) = (stage_id, name) {
            out.entry(id).or_insert(name);
        }
    }
    out
}

fn extract_json_u32(obj: &str, key: &str) -> Option<u32> {
    let needle = format!("\"{key}\"");
    let pos = obj.find(&needle)?;
    let rest = obj[pos + needle.len()..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.trim_start_matches('"');
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn extract_json_str(obj: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let pos = obj.find(&needle)?;
    let rest = obj[pos + needle.len()..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let mut out = String::new();
    let mut esc = false;
    for ch in rest.chars() {
        if esc {
            out.push(ch);
            esc = false;
        } else if ch == '\\' {
            esc = true;
        } else if ch == '"' {
            return Some(out);
        } else {
            out.push(ch);
        }
    }
    None
}

/// Audits one stage stem inside `maps_dir` (under `rbr_root`).
pub fn audit_stem(
    rbr_root: &Path,
    maps_dir: &Path,
    stem: &str,
    preset: Option<&StageDescriptor>,
    rsf_names: &BTreeMap<u32, String>,
    test_sim_boot: bool,
) -> StageAuditRow {
    let (map_id, variant) = parse_track_stem(stem);
    let stage_id = preset
        .map(|p| p.id.clone())
        .unwrap_or_else(|| stem.to_ascii_lowercase());
    let stage_name = preset
        .map(|p| p.name.clone())
        .or_else(|| map_id.and_then(|id| rsf_names.get(&id).cloned()))
        .unwrap_or_else(|| stem.to_string());
    let country = preset
        .map(|p| p.country().to_string())
        .unwrap_or_else(|| country_for_map(map_id).to_string());
    let surface = surface_for_map(map_id).to_string();

    let mut row = StageAuditRow {
        stem: stem.to_string(),
        stage_id,
        stage_name,
        map_id,
        variant: variant.clone(),
        country,
        surface,
        is_preset: preset.is_some(),
        preset_expected_blocks: preset.map(|p| p.terrain_blocks),
        lbs_total_blocks: 0,
        near_loaded_blocks: 0,
        far_only_blocks: 0,
        empty_blocks: 0,
        rejected_blocks: 0,
        anomalies: Vec::new(),
        missing_files: Vec::new(),
        has_start_location: false,
        route_points: 0,
        supported_points: 0,
        collision_matched_points: 0,
        max_ground_gap_m: 0.0,
        dls_start_m: None,
        dls_finish_m: None,
        dls_end_m: None,
        dls_splits: 0,
        dls_pacenotes: 0,
        textures_needed: 0,
        textures_loaded: 0,
        resident_bytes: 0,
        exceeds_budget: false,
        stage_load_ok: None,
        stage_load_error: None,
        sim_boot_ok: None,
        sim_boot_error: None,
        status: StageCompatStatus::Blocked,
        errors: Vec::new(),
    };

    // 1. Check required files for this stem.
    for ext in PHYSICS_STAGE_EXTENSIONS
        .into_iter()
        .chain(std::iter::once("ini"))
    {
        let file = format!("{stem}.{ext}");
        if cars::resolve(maps_dir, &file).is_none() {
            row.missing_files.push(file);
        }
    }
    let tex_rbz = format!("{stem}_textures.rbz");
    if cars::resolve(maps_dir, &tex_rbz).is_none() {
        row.missing_files.push(tex_rbz);
    }
    if cars::resolve(maps_dir, "TextureFilenameMap.dat").is_none() {
        row.missing_files.push("TextureFilenameMap.dat".into());
    }

    // 2. Parse `.dls` markers (start, splits, finish, pacenotes).
    if let Some(dls_path) = cars::resolve(maps_dir, &format!("{stem}.dls")) {
        match std::fs::read(&dls_path)
            .map_err(|e| e.to_string())
            .and_then(|b| markers::parse(&b).map_err(|e| e.to_string()))
        {
            Ok(m) => {
                row.dls_pacenotes = m.pace_notes().count();
                for k in &m.keys {
                    match k.tag {
                        TAG_START => {
                            if row.dls_start_m.is_none() {
                                row.dls_start_m = Some(k.distance);
                            }
                        }
                        TAG_FINISH => {
                            if row.dls_finish_m.is_none() {
                                row.dls_finish_m = Some(k.distance);
                            }
                        }
                        TAG_END => {
                            if row.dls_end_m.is_none() {
                                row.dls_end_m = Some(k.distance);
                            }
                        }
                        TAG_SPLIT => row.dls_splits += 1,
                        _ => {}
                    }
                }
            }
            Err(e) => row.errors.push(format!("DLS parse error: {e}")),
        }
    }

    // 3. Parse `.lbs` and inspect every geometry block in detail.
    let lbs_bytes = match asset_probe::read_stage_stem(maps_dir, stem) {
        Ok(b) => b,
        Err(e) => {
            row.errors.push(format!("LBS read error: {e}"));
            return row;
        }
    };
    let parsed = match lbs::parse(&lbs_bytes) {
        Ok(p) => p,
        Err(e) => {
            row.errors.push(format!("LBS parse error: {e}"));
            return row;
        }
    };

    let mut terrain = mesh::Mesh::default();
    let mut needed_textures = BTreeSet::new();

    for c in &parsed.chunks {
        match c {
            lbs::Chunk::StartLocation(_) => row.has_start_location = true,
            lbs::Chunk::Geometry(g) => {
                for (id, block) in g.blocks.iter().enumerate() {
                    row.lbs_total_blocks += 1;
                    let m = mesh::block_mesh(block);
                    let far_only_block =
                        !block.chunks.iter().any(|c| matches!(c.list, 1 | 2)) && m.far_draws > 0;
                    let view_slots: Vec<_> = block
                        .views
                        .iter()
                        .enumerate()
                        .filter_map(|(s, v)| v.as_ref().map(|b| (s, b.vertex_count)))
                        .collect();
                    let chunks: Vec<_> = block
                        .chunks
                        .iter()
                        .map(|ch| {
                            (
                                ch.list,
                                ch.view,
                                ch.shadow,
                                ch.specular,
                                ch.prim_count,
                                ch.textures.len(),
                            )
                        })
                        .collect();

                    if m.bad_triangles != 0 || m.skipped_draws != 0 {
                        row.rejected_blocks += 1;
                        row.anomalies.push(BlockAnomaly {
                            block_id: id,
                            kind: "REJECTED_UNSUPPORTED_OR_BAD".into(),
                            near_triangles: m.indices.len() / 3,
                            far_draws: m.far_draws,
                            bad_triangles: m.bad_triangles,
                            skipped_draws: m.skipped_draws,
                            view_slots,
                            chunks,
                        });
                    } else if m.indices.is_empty() {
                        row.empty_blocks += 1;
                        row.anomalies.push(BlockAnomaly {
                            block_id: id,
                            kind: "EMPTY_NO_TRIANGLES".into(),
                            near_triangles: 0,
                            far_draws: m.far_draws,
                            bad_triangles: 0,
                            skipped_draws: 0,
                            view_slots,
                            chunks,
                        });
                    } else if let Err(e) = m.bounds() {
                        row.rejected_blocks += 1;
                        row.anomalies.push(BlockAnomaly {
                            block_id: id,
                            kind: format!("INVALID_BOUNDS({e})"),
                            near_triangles: m.indices.len() / 3,
                            far_draws: m.far_draws,
                            bad_triangles: 0,
                            skipped_draws: 0,
                            view_slots,
                            chunks,
                        });
                    } else {
                        if far_only_block {
                            row.far_only_blocks += 1;
                        }
                        for d in &m.draws {
                            for t in d.textures.into_iter().flatten() {
                                needed_textures.insert(t);
                            }
                        }
                        super::stage::append(&mut terrain, m);
                        row.near_loaded_blocks += 1;
                    }
                }
            }
            lbs::Chunk::Sky(s) => {
                for item in &s.items {
                    if matches!((item.flags, item.buffers.stride), (0x15, 24) | (0x35, 32)) {
                        if let Some(t) = item.texture0 {
                            needed_textures.insert(t);
                        }
                        if item.flags == 0x35 {
                            if let Some(t) = item.texture1 {
                                needed_textures.insert(t);
                            }
                        }
                    }
                }
            }
            lbs::Chunk::Objects(o) => {
                for item in o.lanes.iter().flat_map(|l| l.runs.iter().flatten()) {
                    if let Some(t) = item.texture_index() {
                        needed_textures.insert(t);
                    }
                }
            }
            _ => {}
        }
    }

    if row.rejected_blocks > 0 {
        row.errors.push(format!(
            "{} terrain blocks rejected (bad_triangles or skipped_draws)",
            row.rejected_blocks
        ));
    }
    if row.empty_blocks > 0 {
        row.errors.push(format!(
            "{} empty terrain blocks with 0 near/far triangles",
            row.empty_blocks
        ));
    }
    if !row.has_start_location {
        row.errors.push("missing StartLocation chunk".into());
    }

    // 4. Route & COL ground agreement audit.
    let col_ok = if !terrain.indices.is_empty() {
        let triangles = terrain
            .indices
            .chunks_exact(3)
            .map(|t| [t[0], t[1], t[2]].map(|i| Vec3::from(terrain.vertices[i as usize].position)))
            .collect();
        let ground = super::benchmark::route::Ground::new(triangles);
        let col_ground = cars::resolve(maps_dir, &format!("{stem}.col"))
            .ok_or_else(|| format!("missing {stem}.col"))
            .and_then(|p| std::fs::read(p).map_err(|e| e.to_string()))
            .and_then(|bytes| {
                rerbr_sim::rbr_formats::col::parse(&bytes)
                    .map_err(|e| e.to_string())
                    .map(|col| {
                        col.cells
                            .iter()
                            .flat_map(|c| {
                                c.tree.triangles.iter().map(|t| {
                                    t.vertices.map(|i| {
                                        coords::physics_to_stage()
                                            .transform_point3(Vec3::from(c.vertices[i as usize]))
                                    })
                                })
                            })
                            .collect::<Vec<_>>()
                    })
            })
            .and_then(super::benchmark::route::Ground::new);

        match (ground, col_ground) {
            (Ok(ground), Ok(collision)) => {
                for p in parsed
                    .chunks
                    .iter()
                    .filter_map(|c| {
                        if let lbs::Chunk::DrivelinePoints(d) = c {
                            Some(d)
                        } else {
                            None
                        }
                    })
                    .flat_map(|d| &d.points)
                {
                    row.route_points += 1;
                    let p = Vec3::from(*p);
                    if let Some(height) = ground.support_height(p) {
                        row.supported_points += 1;
                        if let Some(col) = collision.support_height(p) {
                            let gap = (col - height).abs();
                            row.max_ground_gap_m = row.max_ground_gap_m.max(gap);
                            if gap <= 0.5 {
                                row.collision_matched_points += 1;
                            }
                        } else {
                            let mut nearest = None;
                            'probe: for r in [0.005_f32, 0.02, 0.1, 0.5, 2.0] {
                                for (dx, dz) in [(r, 0.0), (-r, 0.0), (0.0, r), (0.0, -r)] {
                                    if let Some(h) =
                                        collision.support_height(Vec3::new(p.x + dx, p.y, p.z + dz))
                                    {
                                        nearest = Some((r, (h - height).abs()));
                                        break 'probe;
                                    }
                                }
                            }
                            row.errors.push(format!(
                                "COL miss at route point #{} pos=({:.2},{:.2},{:.2}) terrain_y={:.2} nearest={:?}",
                                row.route_points - 1,
                                p.x,
                                p.y,
                                p.z,
                                height,
                                nearest
                            ));
                        }
                    }
                }
                if row.route_points == 0 {
                    row.errors.push("no DrivelinePoints in LBS".into());
                    false
                } else {
                    if row.supported_points != row.route_points {
                        row.errors.push(format!(
                            "terrain route coverage {}/{}",
                            row.supported_points, row.route_points
                        ));
                    }
                    if row.collision_matched_points != row.route_points {
                        row.errors.push(format!(
                            "visual/COL ground agreement {}/{} (max gap {:.3}m)",
                            row.collision_matched_points, row.route_points, row.max_ground_gap_m
                        ));
                    }
                    row.supported_points == row.route_points
                        && row.collision_matched_points == row.route_points
                }
            }
            (Err(e), _) => {
                row.errors.push(format!("terrain ground build error: {e}"));
                false
            }
            (_, Err(e)) => {
                row.errors.push(format!("COL ground build error: {e}"));
                false
            }
        }
    } else {
        false
    };

    // 5. Texture resolution & resident size estimate.
    row.textures_needed = needed_textures.len();
    match asset_probe::load_diffuse_shared_stem(maps_dir, stem, &needed_textures) {
        Ok(shared) => {
            row.textures_loaded = shared.textures.len();
            let tex_bytes: usize = shared.textures.values().map(|t| t.bytes.len()).sum();
            row.resident_bytes =
                terrain.vertices.len() * 32 + terrain.indices.len() * 4 + tex_bytes;
            row.exceeds_budget = row.resident_bytes > 192 * 1024 * 1024;
            if row.exceeds_budget {
                row.errors.push(format!(
                    "resident_bytes {} exceeds 192MiB budget",
                    row.resident_bytes
                ));
            }
        }
        Err(e) => {
            row.errors.push(format!("texture load error: {e}"));
        }
    }

    // 6. Test actual `stage::load_selected` when a preset matches this stem.
    if let Some(p) = preset {
        match super::stage::load_selected(maps_dir, p) {
            Ok(loaded) => {
                row.stage_load_ok = Some(true);
                row.resident_bytes = loaded.report.resident_bytes;
                row.exceeds_budget = row.resident_bytes > 192 * 1024 * 1024;
            }
            Err(e) => {
                row.stage_load_ok = Some(false);
                row.stage_load_error = Some(e.clone());
                row.errors.push(format!("stage::load_selected failed: {e}"));
            }
        }
    }

    // 7. Optionally boot the actual 720 Hz retail simulation with Corolla WRC.
    if test_sim_boot {
        if let Some(p) = preset {
            let paths = rerbr_sim::inputs::Paths {
                install: rbr_root.to_path_buf(),
                overlay: None,
                maps: maps_dir.to_path_buf(),
            };
            let sel = rerbr_content::Selection {
                vehicle: rerbr_content::VehicleDescriptor::toyota_corolla_wrc(),
                stage: p.clone(),
            };
            // Boot on a dedicated thread with STACK_BYTES so pure-host has ample stack.
            let boot_res = std::thread::Builder::new()
                .stack_size(rerbr_sim::STACK_BYTES)
                .spawn(move || rerbr_sim::drive::Session::boot_selected(&paths, &sel, false))
                .map_err(|e| format!("thread spawn failed: {e}"))
                .and_then(|h| {
                    h.join()
                        .map_err(|_| "physics boot panicked".to_string())?
                        .map(|_| ())
                });
            match boot_res {
                Ok(()) => row.sim_boot_ok = Some(true),
                Err(e) => {
                    row.sim_boot_ok = Some(false);
                    row.sim_boot_error = Some(e.clone());
                    row.errors.push(format!("sim boot failed: {e}"));
                }
            }
        }
    }

    // 8. Final honest classification.
    let geometry_ok = row.rejected_blocks == 0
        && row.empty_blocks == 0
        && row.near_loaded_blocks > 0
        && row.has_start_location
        && row.textures_loaded > 0
        && !row.exceeds_budget;
    let dls_ok = row.dls_start_m.is_some() && row.dls_finish_m.is_some();

    row.status = if !row.missing_files.is_empty() || row.rejected_blocks > 0 || !geometry_ok {
        if row.near_loaded_blocks > 0 && row.missing_files.is_empty() {
            StageCompatStatus::PartiallyCompatible
        } else {
            StageCompatStatus::Blocked
        }
    } else if !col_ok
        || !dls_ok
        || row.stage_load_ok == Some(false)
        || row.sim_boot_ok == Some(false)
    {
        StageCompatStatus::PartiallyCompatible
    } else if row.stage_load_ok == Some(true) && row.sim_boot_ok == Some(true) {
        StageCompatStatus::FullyPlayable
    } else {
        StageCompatStatus::GeometryValidNotPlayable
    };

    row
}

/// Discovers and audits every `track-*.lbs` in `rbr_root/Maps`.
pub fn audit_all_maps(rbr_root: &Path, test_sim_boot: bool) -> Vec<StageAuditRow> {
    let maps_dir = rbr_root.join("Maps");
    let Ok(entries) = std::fs::read_dir(&maps_dir) else {
        return Vec::new();
    };
    let rsf_names = load_rsf_stage_names(rbr_root);
    let presets = StageDescriptor::presets();

    let mut stems: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let lower = name.to_ascii_lowercase();
            if lower.starts_with("track-") && lower.ends_with(".lbs") {
                Some(name[..name.len() - 4].to_string())
            } else {
                None
            }
        })
        .collect();
    stems.sort_by_key(|s| s.to_ascii_lowercase());

    stems
        .into_iter()
        .map(|stem| {
            let preset = presets.iter().find(|p| p.stem.eq_ignore_ascii_case(&stem));
            audit_stem(
                rbr_root,
                &maps_dir,
                &stem,
                preset,
                &rsf_names,
                test_sim_boot,
            )
        })
        .collect()
}

fn csv_escape(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn to_csv(rows: &[StageAuditRow]) -> String {
    let mut out = String::from(
        "stem,stage_id,stage_name,map_id,variant,country,surface,is_preset,preset_expected_blocks,lbs_total_blocks,near_loaded_blocks,far_only_blocks,empty_blocks,rejected_blocks,has_start_location,route_points,supported_points,collision_matched_points,max_ground_gap_m,dls_start_m,dls_finish_m,dls_splits,dls_pacenotes,textures_needed,textures_loaded,resident_bytes,stage_load_ok,sim_boot_ok,status,errors\n",
    );
    for r in rows {
        let _ = writeln!(
            out,
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{},{},{},{},{},{},{},{},{},{},{}",
            csv_escape(&r.stem),
            csv_escape(&r.stage_id),
            csv_escape(&r.stage_name),
            r.map_id.map(|m| m.to_string()).unwrap_or_default(),
            csv_escape(&r.variant),
            csv_escape(&r.country),
            csv_escape(&r.surface),
            r.is_preset,
            r.preset_expected_blocks
                .map(|b| b.to_string())
                .unwrap_or_default(),
            r.lbs_total_blocks,
            r.near_loaded_blocks,
            r.far_only_blocks,
            r.empty_blocks,
            r.rejected_blocks,
            r.has_start_location,
            r.route_points,
            r.supported_points,
            r.collision_matched_points,
            r.max_ground_gap_m,
            r.dls_start_m
                .map(|v| format!("{v:.1}"))
                .unwrap_or_default(),
            r.dls_finish_m
                .map(|v| format!("{v:.1}"))
                .unwrap_or_default(),
            r.dls_splits,
            r.dls_pacenotes,
            r.textures_needed,
            r.textures_loaded,
            r.resident_bytes,
            r.stage_load_ok.map(|b| b.to_string()).unwrap_or_default(),
            r.sim_boot_ok.map(|b| b.to_string()).unwrap_or_default(),
            r.status.as_str(),
            csv_escape(&r.errors.join(" | ")),
        );
    }
    out
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn to_json(rows: &[StageAuditRow]) -> String {
    let mut out = String::from("[\n");
    for (i, r) in rows.iter().enumerate() {
        let anomalies_json: Vec<String> = r
            .anomalies
            .iter()
            .map(|a| {
                format!(
                    "{{\"block_id\":{},\"kind\":{},\"near_triangles\":{},\"far_draws\":{},\"bad_triangles\":{},\"skipped_draws\":{}}}",
                    a.block_id,
                    json_escape(&a.kind),
                    a.near_triangles,
                    a.far_draws,
                    a.bad_triangles,
                    a.skipped_draws
                )
            })
            .collect();
        let errors_json: Vec<String> = r.errors.iter().map(|e| json_escape(e)).collect();
        let missing_json: Vec<String> = r.missing_files.iter().map(|e| json_escape(e)).collect();
        let opt_num = |v: Option<usize>| {
            v.map(|n| n.to_string())
                .unwrap_or_else(|| "null".to_string())
        };
        let opt_u32 = |v: Option<u32>| {
            v.map(|n| n.to_string())
                .unwrap_or_else(|| "null".to_string())
        };
        let opt_f32 = |v: Option<f32>| {
            v.map(|n| format!("{n:.2}"))
                .unwrap_or_else(|| "null".to_string())
        };
        let opt_bool = |v: Option<bool>| {
            v.map(|b| b.to_string())
                .unwrap_or_else(|| "null".to_string())
        };
        let _ = writeln!(
            out,
            "  {{\"stem\":{},\"stage_id\":{},\"stage_name\":{},\"map_id\":{},\"variant\":{},\"country\":{},\"surface\":{},\"is_preset\":{},\"preset_expected_blocks\":{},\"lbs_total_blocks\":{},\"near_loaded_blocks\":{},\"far_only_blocks\":{},\"empty_blocks\":{},\"rejected_blocks\":{},\"has_start_location\":{},\"route_points\":{},\"supported_points\":{},\"collision_matched_points\":{},\"max_ground_gap_m\":{:.3},\"dls_start_m\":{},\"dls_finish_m\":{},\"dls_splits\":{},\"dls_pacenotes\":{},\"textures_needed\":{},\"textures_loaded\":{},\"resident_bytes\":{},\"stage_load_ok\":{},\"sim_boot_ok\":{},\"status\":{},\"missing_files\":[{}],\"anomalies\":[{}],\"errors\":[{}]}}{}",
            json_escape(&r.stem),
            json_escape(&r.stage_id),
            json_escape(&r.stage_name),
            opt_u32(r.map_id),
            json_escape(&r.variant),
            json_escape(&r.country),
            json_escape(&r.surface),
            r.is_preset,
            opt_num(r.preset_expected_blocks),
            r.lbs_total_blocks,
            r.near_loaded_blocks,
            r.far_only_blocks,
            r.empty_blocks,
            r.rejected_blocks,
            r.has_start_location,
            r.route_points,
            r.supported_points,
            r.collision_matched_points,
            r.max_ground_gap_m,
            opt_f32(r.dls_start_m),
            opt_f32(r.dls_finish_m),
            r.dls_splits,
            r.dls_pacenotes,
            r.textures_needed,
            r.textures_loaded,
            r.resident_bytes,
            opt_bool(r.stage_load_ok),
            opt_bool(r.sim_boot_ok),
            json_escape(r.status.as_str()),
            missing_json.join(","),
            anomalies_json.join(","),
            errors_json.join(","),
            if i + 1 < rows.len() { "," } else { "" }
        );
    }
    out.push_str("]\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retail_36_presets_preflight_weather_textures_and_trk_if_pc_present() {
        let rbr_root = Path::new(r"C:\Richard Burns Rally");
        let maps_dir = rbr_root.join("Maps");
        if !maps_dir.join("track-63_O.lbs").is_file() {
            return;
        }
        let data = rerbr_sim::rbr_formats::data::Data::new(rbr_root.to_path_buf(), None);
        let season = rerbr_sim::rbr_gameplay::weather::Season::load(&data)
            .expect("season.dat should load from PC RBR");
        let presets = StageDescriptor::presets();
        assert_eq!(
            presets.len(),
            36,
            "all 36 retail RBR stages must be presets"
        );

        let mut failures = Vec::new();
        for p in &presets {
            let weather_ok = season.rows.iter().any(|r| {
                r.map == p.original_map && r.letter == p.weather_letter && r.wetness == p.wetness
            });
            let available_rows: Vec<_> = season
                .rows
                .iter()
                .filter(|r| r.map == p.original_map)
                .map(|r| (r.letter, r.wetness))
                .collect();
            let trk_ok = cars::resolve(&maps_dir, &format!("{}.trk", p.stem))
                .and_then(|path| std::fs::read(path).ok())
                .and_then(|b| rerbr_sim::rbr_formats::trk::parse(&b).ok())
                .is_some();
            let boot_missing: Vec<_> = p
                .additional_boot_paths()
                .into_iter()
                .filter(|rel| data.resolve(rel).is_none())
                .collect();

            // Check which _textures.rbz archives are referenced by this stage's diffuse textures.
            let mut archives = BTreeSet::new();
            if let Ok(lbs_bytes) = asset_probe::read_stage_stem(&maps_dir, &p.stem) {
                if let Ok(parsed) = lbs::parse(&lbs_bytes) {
                    let mut needed = BTreeSet::new();
                    for c in &parsed.chunks {
                        match c {
                            lbs::Chunk::Geometry(g) => {
                                for blk in &g.blocks {
                                    let m = mesh::block_mesh(blk);
                                    for d in &m.draws {
                                        for t in d.textures.into_iter().flatten() {
                                            needed.insert(t);
                                        }
                                    }
                                }
                            }
                            lbs::Chunk::Objects(o) => {
                                for item in o.lanes.iter().flat_map(|l| l.runs.iter().flatten()) {
                                    if let Some(t) = item.texture_index() {
                                        needed.insert(t);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    if let Ok(shared) =
                        asset_probe::load_diffuse_shared_stem(&maps_dir, &p.stem, &needed)
                    {
                        for tex in shared.textures.values() {
                            if let Some((arch, _)) = tex.info.resolved.split_once('\\') {
                                archives.insert(arch.to_ascii_lowercase());
                            }
                        }
                    }
                }
            }
            eprintln!(
                "PREFLIGHT map={:02} {:<10} {:<18} weather_ok={} (want=({},{}) avail={:?}) trk_ok={} boot_miss={:?} archives={:?}",
                p.original_map,
                p.stem,
                p.name,
                weather_ok,
                p.weather_letter,
                p.wetness,
                available_rows,
                trk_ok,
                boot_missing,
                archives
            );
            if !trk_ok {
                failures.push(format!("TRK parse failed for {}", p.stem));
            }
            if !boot_missing.is_empty() {
                failures.push(format!(
                    "missing boot spectator files for {}: {:?}",
                    p.stem, boot_missing
                ));
            }
            if !weather_ok {
                failures.push(format!(
                    "season.dat has no weather row (letter={}, wetness={}) for map {} ({}): available={:?}",
                    p.weather_letter, p.wetness, p.original_map, p.name, available_rows
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "preflight failures:\n{}",
            failures.join("\n")
        );
    }

    #[test]
    fn diamond_creek_and_retail_landscape_anatomy_if_pc_present() {
        let rbr_root = Path::new(r"C:\Richard Burns Rally");
        let maps_dir = rbr_root.join("Maps");
        if !maps_dir.join("track-63_O.lbs").is_file() {
            return;
        }
        for stem in [
            "track-63_O",
            "track-66_O",
            "track-20_O",
            "track-31_E",
            "track-41_O",
        ] {
            let bytes = asset_probe::read_stage_stem(&maps_dir, stem).unwrap();
            let parsed = lbs::parse(&bytes).unwrap();
            let names = asset_probe::diffuse_names_stem(&maps_dir, stem).unwrap_or_default();
            let start = parsed
                .chunks
                .iter()
                .find_map(|c| match c {
                    lbs::Chunk::StartLocation(s) => Some(Vec3::from(s.position)),
                    _ => None,
                })
                .unwrap_or(Vec3::ZERO);

            eprintln!(
                "================ LANDSCAPE ANATOMY: {stem} (start={start:?}) ================"
            );
            let mut blk_total = 0usize;
            let mut blk_near_only = 0usize;
            let mut blk_mixed = 0usize;
            let mut blk_far_only = 0usize;
            let mut list1_tris = 0usize;
            let mut list2_tris = 0usize;
            let mut list3_tris_on_mixed = 0usize;
            let mut list3_tris_on_far_only = 0usize;
            let mut mixed_list3_extends_box = 0usize;
            let mut draws_le_450 = 0usize;
            let mut draws_450_1500 = 0usize;
            let mut draws_gt_1500 = 0usize;
            let mut far_only_dists = Vec::new();
            let mut obj_items_r0 = 0usize;
            let mut obj_items_r1 = 0usize;
            let mut obj_second_own = 0usize;
            let mut obj_second_same = 0usize;
            let mut obj_flags_05 = 0usize;
            let mut obj_flags_15 = 0usize;

            for c in &parsed.chunks {
                match c {
                    lbs::Chunk::Geometry(g) => {
                        for (id, blk) in g.blocks.iter().enumerate() {
                            blk_total += 1;
                            let has_near = blk.chunks.iter().any(|ch| matches!(ch.list, 1 | 2));
                            let has_far = blk.chunks.iter().any(|ch| ch.list == 3);
                            match (has_near, has_far) {
                                (true, false) => blk_near_only += 1,
                                (true, true) => blk_mixed += 1,
                                (false, true) => blk_far_only += 1,
                                (false, false) => {}
                            }
                            let mut near_min = Vec3::splat(f32::INFINITY);
                            let mut near_max = Vec3::splat(f32::NEG_INFINITY);
                            let mut far_min = Vec3::splat(f32::INFINITY);
                            let mut far_max = Vec3::splat(f32::NEG_INFINITY);
                            for ch in &blk.chunks {
                                let bc = Vec3::from(ch.bounds.center);
                                let be = Vec3::from(ch.bounds.half_extent);
                                let bmin = bc - be;
                                let bmax = bc + be;
                                let d = start.distance(start.clamp(bmin, bmax));
                                match ch.list {
                                    1 => {
                                        list1_tris += ch.prim_count as usize;
                                        near_min = near_min.min(bmin);
                                        near_max = near_max.max(bmax);
                                    }
                                    2 => {
                                        list2_tris += ch.prim_count as usize;
                                        near_min = near_min.min(bmin);
                                        near_max = near_max.max(bmax);
                                    }
                                    3 => {
                                        far_min = far_min.min(bmin);
                                        far_max = far_max.max(bmax);
                                        if has_near {
                                            list3_tris_on_mixed += ch.prim_count as usize;
                                        } else {
                                            list3_tris_on_far_only += ch.prim_count as usize;
                                            far_only_dists.push((
                                                id,
                                                d.round() as u32,
                                                ch.prim_count,
                                                bmin,
                                                bmax,
                                            ));
                                        }
                                    }
                                    _ => {}
                                }
                                if matches!(ch.list, 1 | 2) || !has_near {
                                    if d <= 450.0 {
                                        draws_le_450 += 1;
                                    } else if d <= 1500.0 {
                                        draws_450_1500 += 1;
                                    } else {
                                        draws_gt_1500 += 1;
                                    }
                                }
                            }
                            if has_near && has_far {
                                let ext =
                                    (near_min - far_min).max(far_max - near_max).max_element();
                                if ext > 5.0 {
                                    mixed_list3_extends_box += 1;
                                    if mixed_list3_extends_box <= 5 {
                                        eprintln!(
                                            "  MIXED blk#{id} list3 extends >5m beyond near! ext={ext:.1}m near=({near_min:?}..{near_max:?}) far=({far_min:?}..{far_max:?})"
                                        );
                                    }
                                }
                            }
                        }
                    }
                    lbs::Chunk::Sky(sky) => {
                        let sname = String::from_utf8_lossy(sky.name);
                        let mut sky_tris = 0usize;
                        let mut smin = Vec3::splat(f32::INFINITY);
                        let mut smax = Vec3::splat(f32::NEG_INFINITY);
                        let mut sky_flags = BTreeSet::new();
                        let mut sky_tex = BTreeSet::new();
                        for it in &sky.items {
                            sky_tris += it.buffers.index_count() / 3;
                            sky_flags.insert((it.flags, it.buffers.stride, it.kind));
                            if let Some(t0) = it.texture0 {
                                sky_tex.insert((
                                    t0,
                                    names.get(&t0).cloned().unwrap_or_else(|| "?".into()),
                                ));
                            }
                            for vi in 0..it.buffers.vertex_count {
                                if let Some(p) = it.buffers.position(vi) {
                                    let v = Vec3::from(p);
                                    smin = smin.min(v);
                                    smax = smax.max(v);
                                }
                            }
                        }
                        eprintln!(
                            "  SKY name={sname:?} items={} tris={} flags={:?} bounds=({smin:?}..{smax:?}) tex={:?}",
                            sky.items.len(),
                            sky_tris,
                            sky_flags,
                            sky_tex
                        );
                    }
                    lbs::Chunk::Clouds(cl) => {
                        eprintln!(
                            "  CLOUDS name={:?} items={}",
                            String::from_utf8_lossy(cl.name),
                            cl.items.len()
                        );
                    }
                    lbs::Chunk::Water(w) => {
                        let tris: usize = w
                            .faces
                            .iter()
                            .flat_map(|f| &f.items)
                            .map(|it| it.buffers.indices.len() / 6)
                            .sum();
                        eprintln!("  WATER faces={} tris={tris}", w.faces.len());
                    }
                    lbs::Chunk::Reflection(r) => {
                        eprintln!("  REFLECTION faces={}", r.faces.len());
                    }
                    lbs::Chunk::Objects(o) => {
                        for lane in &o.lanes {
                            obj_items_r0 += lane.runs[0].len();
                            obj_items_r1 += lane.runs[1].len();
                            for it in lane.runs.iter().flatten() {
                                match it.flags {
                                    0x05 => obj_flags_05 += 1,
                                    0x15 => obj_flags_15 += 1,
                                    _ => {}
                                }
                                match it.second_indices {
                                    lbs::SecondIndices::Own(_) => obj_second_own += 1,
                                    lbs::SecondIndices::SameAsFirst => obj_second_same += 1,
                                    lbs::SecondIndices::None => {}
                                }
                            }
                        }
                    }
                    lbs::Chunk::Interactive(i) => {
                        let total_inst: usize = i.objects.iter().map(|o| o.visibility.len()).sum();
                        eprintln!(
                            "  INTERACTIVE objects={} total_instances={total_inst}",
                            i.objects.len()
                        );
                    }
                    lbs::Chunk::Other { kind, data } => {
                        eprintln!("  OTHER chunk kind={kind:?} bytes={}", data.len());
                    }
                    _ => {}
                }
            }
            eprintln!(
                "  BLOCKS: total={blk_total} near_only={blk_near_only} mixed={blk_mixed} far_only={blk_far_only} | mixed_list3_extends_gt5m={mixed_list3_extends_box}"
            );
            eprintln!(
                "  TRIS: list1={list1_tris} list2={list2_tris} list3_mixed={list3_tris_on_mixed} list3_far_only={list3_tris_on_far_only}"
            );
            eprintln!(
                "  TERRAIN DRAWS FROM START: <=450m={draws_le_450}, 450..1500m={draws_450_1500}, >1500m={draws_gt_1500}"
            );
            eprintln!(
                "  OBJECTS: run0={obj_items_r0} run1={obj_items_r1} flags_15={obj_flags_15} flags_05={obj_flags_05} second_same={obj_second_same} second_own={obj_second_own}"
            );
            if !far_only_dists.is_empty() {
                eprintln!(
                    "  FAR-ONLY BLOCKS (id, dist_from_start_m, tris, min, max): {far_only_dists:?}"
                );
            }
        }
    }

    #[test]
    fn audit_sky_water_and_36_splits_if_pc_present() {
        let rbr_root = Path::new(r"C:\Richard Burns Rally");
        let maps_dir = rbr_root.join("Maps");
        if !maps_dir.join("track-63_O.lbs").is_file() {
            return;
        }
        // 1. Inspect SkyItem vertex layouts & textures on Diamond Creek I (track-63_O)
        let bytes = asset_probe::read_stage_stem(&maps_dir, "track-63_O").unwrap();
        let parsed = lbs::parse(&bytes).unwrap();
        let names = asset_probe::diffuse_names_stem(&maps_dir, "track-63_O").unwrap();
        let mut sky_needed = BTreeSet::new();
        for c in &parsed.chunks {
            if let lbs::Chunk::Sky(sky) = c {
                for (i, it) in sky.items.iter().enumerate() {
                    if let Some(t) = it.texture0 {
                        sky_needed.insert(t);
                    }
                    if let Some(t) = it.texture1 {
                        sky_needed.insert(t);
                    }
                    let b = &it.buffers;
                    let mut min_uv0 = [f32::INFINITY; 2];
                    let mut max_uv0 = [f32::NEG_INFINITY; 2];
                    let mut min_uv1 = [f32::INFINITY; 2];
                    let mut max_uv1 = [f32::NEG_INFINITY; 2];
                    let mut min_a = 255u8;
                    let mut max_a = 0u8;
                    let mut up_cw = 0usize;
                    let mut down_cw = 0usize;
                    for v in 0..b.vertex_count {
                        let off = v * b.stride;
                        let a = b.vertices[off + 15];
                        min_a = min_a.min(a);
                        max_a = max_a.max(a);
                        let u0 =
                            f32::from_le_bytes(b.vertices[off + 16..off + 20].try_into().unwrap());
                        let v0 =
                            f32::from_le_bytes(b.vertices[off + 20..off + 24].try_into().unwrap());
                        min_uv0[0] = min_uv0[0].min(u0);
                        min_uv0[1] = min_uv0[1].min(v0);
                        max_uv0[0] = max_uv0[0].max(u0);
                        max_uv0[1] = max_uv0[1].max(v0);
                        if b.stride >= 32 {
                            let u1 = f32::from_le_bytes(
                                b.vertices[off + 24..off + 28].try_into().unwrap(),
                            );
                            let v1 = f32::from_le_bytes(
                                b.vertices[off + 28..off + 32].try_into().unwrap(),
                            );
                            min_uv1[0] = min_uv1[0].min(u1);
                            min_uv1[1] = min_uv1[1].min(v1);
                            max_uv1[0] = max_uv1[0].max(u1);
                            max_uv1[1] = max_uv1[1].max(v1);
                        }
                    }
                    for tri in 0..(b.index_count() / 3) {
                        let (Some(i0), Some(i1), Some(i2)) = (
                            b.index(tri * 3).map(usize::from),
                            b.index(tri * 3 + 1).map(usize::from),
                            b.index(tri * 3 + 2).map(usize::from),
                        ) else {
                            continue;
                        };
                        let (Some(p0), Some(p1), Some(p2)) =
                            (b.position(i0), b.position(i1), b.position(i2))
                        else {
                            continue;
                        };
                        // In LH coords, CW front-face has (p1-p0).cross(p2-p0) pointing toward viewer (+Y for ground)
                        let n = (Vec3::from(p1) - Vec3::from(p0))
                            .cross(Vec3::from(p2) - Vec3::from(p0));
                        if n.y >= 0.0 {
                            up_cw += 1;
                        } else {
                            down_cw += 1;
                        }
                    }
                    if i < 10 || it.flags == 0x35 {
                        eprintln!(
                            "  SKY#{i:02} flags={:#x} kind={} stride={} tris={} t0={:?}({:?}) t1={:?}({:?}) alpha={}..{} uv0=({:.2}..{:.2},{:.2}..{:.2}) uv1=({:.2}..{:.2}) cw_y(+/-)={}/{}",
                            it.flags,
                            it.kind,
                            b.stride,
                            b.index_count() / 3,
                            it.texture0,
                            it.texture0.and_then(|t| names.get(&t)),
                            it.texture1,
                            it.texture1.and_then(|t| names.get(&t)),
                            min_a,
                            max_a,
                            min_uv0[0],
                            max_uv0[0],
                            min_uv0[1],
                            max_uv0[1],
                            min_uv1[0],
                            max_uv1[0],
                            up_cw,
                            down_cw
                        );
                    }
                }
            }
            if let lbs::Chunk::Objects(o) = c {
                let mut own_le = 0usize;
                let mut own_gt = 0usize;
                for it in o.lanes.iter().flat_map(|l| l.runs.iter().flatten()) {
                    if let lbs::SecondIndices::Own(sec) = it.second_indices {
                        if sec.len() / 2 <= it.buffers.index_count() {
                            own_le += 1;
                        } else {
                            own_gt += 1;
                        }
                    }
                }
                eprintln!("  OBJECTS second_indices::Own vs first: len<=first:{own_le}, len>first:{own_gt}");
            }
        }
        let sky_shared =
            asset_probe::load_diffuse_shared_stem(&maps_dir, "track-63_O", &sky_needed);
        eprintln!(
            "  SKY TEXTURE RESOLVE on track-63_O: needed={} result={:?}",
            sky_needed.len(),
            sky_shared.as_ref().map(|s| s.textures.len())
        );

        // 2. Inspect skyboxes.lbs and Clouds.lbs
        for extra in ["skyboxes", "Clouds"] {
            if let Ok(eb) = asset_probe::read_stage_stem(&maps_dir, extra) {
                if let Ok(ep) = lbs::parse(&eb) {
                    let summary: Vec<String> = ep
                        .chunks
                        .iter()
                        .map(|ch| match ch {
                            lbs::Chunk::Sky(s) => format!(
                                "Sky({:?}, items={})",
                                String::from_utf8_lossy(s.name),
                                s.items.len()
                            ),
                            lbs::Chunk::Clouds(s) => format!(
                                "Clouds({:?}, items={})",
                                String::from_utf8_lossy(s.name),
                                s.items.len()
                            ),
                            lbs::Chunk::Geometry(g) => format!("Geometry(blks={})", g.blocks.len()),
                            lbs::Chunk::Other { kind, data } => {
                                format!("Other({kind:?}, {}B)", data.len())
                            }
                            other => format!("{:?}", std::mem::discriminant(other)),
                        })
                        .collect();
                    eprintln!("  EXTERNAL {extra}.lbs chunks={summary:?}");
                }
            }
        }

        // 3. Audit Sky/Water/SkyFlags AND exact DLS start/splits/finish across ALL 36 retail stages!
        let mut all_sky_flags = BTreeSet::new();
        let mut all_water_flags = BTreeSet::new();
        for p in StageDescriptor::presets() {
            let b = asset_probe::read_stage_stem(&maps_dir, &p.stem).unwrap();
            let l = lbs::parse(&b).unwrap();
            let mut sky_items = 0usize;
            let mut sky_tris = 0usize;
            let mut water_items = 0usize;
            let mut obj_items = 0usize;
            for c in &l.chunks {
                match c {
                    lbs::Chunk::Sky(s) => {
                        sky_items += s.items.len();
                        for it in &s.items {
                            sky_tris += it.buffers.index_count() / 3;
                            all_sky_flags.insert((it.flags, it.buffers.stride));
                        }
                    }
                    lbs::Chunk::Water(w) => {
                        for f in &w.faces {
                            water_items += f.items.len();
                            for it in &f.items {
                                all_water_flags.insert((it.flags, it.buffers.vertex_size));
                            }
                        }
                    }
                    lbs::Chunk::Objects(o) => {
                        obj_items += o
                            .lanes
                            .iter()
                            .flat_map(|ln| ln.runs.iter().flatten())
                            .count();
                    }
                    _ => {}
                }
            }
            let dls_bytes =
                std::fs::read(cars::resolve(&maps_dir, &format!("{}.dls", p.stem)).unwrap())
                    .unwrap();
            let mk = markers::parse(&dls_bytes).unwrap();
            let start_m = mk
                .keys
                .iter()
                .find(|k| k.tag == TAG_START)
                .map(|k| k.distance);
            let finish_m = mk
                .keys
                .iter()
                .find(|k| k.tag == TAG_FINISH)
                .map(|k| k.distance);
            let splits: Vec<f32> = mk
                .keys
                .iter()
                .filter(|k| k.tag == TAG_SPLIT)
                .map(|k| k.distance)
                .collect();
            eprintln!(
                "  36-AUDIT map={:02} {:<10} {:<18} | sky={:2}({:5}t) water={:2} obj={:3} | DLS S={:<6} splits({}):{:?} F={:<7}",
                p.original_map,
                p.stem,
                p.name,
                sky_items,
                sky_tris,
                water_items,
                obj_items,
                start_m.map(|v| format!("{v:.0}")).unwrap_or_else(|| "-".into()),
                splits.len(),
                splits.iter().map(|v| v.round() as i32).collect::<Vec<_>>(),
                finish_m.map(|v| format!("{v:.0}")).unwrap_or_else(|| "-".into()),
            );
        }
        eprintln!("  ALL 36 SKY FLAGS/STRIDES: {all_sky_flags:?}");
        eprintln!("  ALL 36 WATER FLAGS/STRIDES: {all_water_flags:?}");
    }

    #[test]
    fn diamond_creek_loads_and_submits_landscape_horizon_and_terrain_draws() {
        let maps_dir = Path::new(r"C:\Richard Burns Rally\Maps");
        if !maps_dir.join("track-63_O.lbs").is_file() {
            return;
        }
        let presets = StageDescriptor::presets();
        for (stem, expected_blocks, min_horizon_tris) in [
            ("track-63_O", 139usize, 12_500usize),
            ("track-66_O", 139usize, 12_500usize),
        ] {
            let preset = presets
                .iter()
                .find(|p| p.stem.eq_ignore_ascii_case(stem))
                .expect("Diamond Creek preset");
            let loaded = crate::harwood::stage::load_selected(maps_dir, preset)
                .unwrap_or_else(|e| panic!("{stem} load_selected failed: {e}"));
            let r = &loaded.report;
            assert_eq!(r.blocks, expected_blocks, "{stem} terrain blocks");
            assert_eq!(r.sky_items_loaded, 30, "{stem} SuperBowl sky items");
            assert_eq!(r.sky_items_withheld, 0, "{stem} withheld sky items");
            assert!(
                r.horizon_landscape_triangles >= min_horizon_tris,
                "{stem} horizon tris={}",
                r.horizon_landscape_triangles
            );
            assert!(
                r.terrain_far_draws >= 6,
                "{stem} far-only terrain draws={}",
                r.terrain_far_draws
            );
            assert!(
                r.resident_bytes <= 192 * 1024 * 1024,
                "{stem} resident bytes {}",
                r.resident_bytes
            );

            // Inspect DDS formats of horizon textures on Diamond Creek I.
            if stem == "track-63_O" {
                let mut horizon_tex_ids = BTreeSet::new();
                for d in &loaded.render.mesh.draws {
                    if d.category == crate::harwood::mesh::DrawCategory::HorizonLandscape {
                        for t in d.textures.into_iter().flatten() {
                            horizon_tex_ids.insert(t);
                        }
                    }
                }
                for id in horizon_tex_ids {
                    if let Some(td) = loaded.render.textures.get(&id) {
                        let header = rerbr_asset_formats::dds::parse(&td.bytes).ok();
                        eprintln!(
                            "  HORIZON TEX {id}: {} -> {:?} {:?}",
                            td.info.request,
                            td.info.resolved,
                            header.map(|h| (h.format, h.width, h.height))
                        );
                    }
                }
            }

            // Simulate spawn camera looking down the road (first route segment from start)
            // and verify GPU draw submission counts by category via `visible_for_category`.
            let start = loaded.render.start;
            let look_target = loaded
                .route_points
                .iter()
                .copied()
                .find(|p| p.distance(start) > 15.0 && p.distance(start) < 60.0)
                .unwrap_or(start + Vec3::Z * 30.0);
            let cam = crate::harwood::Camera {
                eye: start + Vec3::new(0.0, 2.5, 0.0),
                target: look_target + Vec3::new(0.0, 1.5, 0.0),
                up: Vec3::Y,
            };
            let vp = cam.view_projection(1.55);
            let (mut vis_near, mut vis_far, mut vis_horizon, mut vis_obj) = (0, 0, 0, 0);
            let mut vis_horizon_tris = 0usize;
            for d in &loaded.render.mesh.draws {
                let bounds = crate::harwood::stage::draw_bounds(&loaded.render.mesh, d);
                if crate::harwood::stage::visible_for_category(bounds, vp, cam.eye, d.category) {
                    match d.category {
                        crate::harwood::mesh::DrawCategory::TerrainNear => vis_near += 1,
                        crate::harwood::mesh::DrawCategory::TerrainFar => vis_far += 1,
                        crate::harwood::mesh::DrawCategory::HorizonLandscape => {
                            vis_horizon += 1;
                            vis_horizon_tris += d.indices.len() / 3;
                        }
                        crate::harwood::mesh::DrawCategory::SceneryObject => vis_obj += 1,
                    }
                }
            }
            eprintln!(
                "  {stem} SPAWN VISIBLE: near={vis_near}/{} far={vis_far}/{} horizon={vis_horizon}/{} ({vis_horizon_tris}t) obj={vis_obj}/{} | ram={:.1}MiB",
                r.terrain_near_draws,
                r.terrain_far_draws,
                r.horizon_landscape_draws,
                r.object_draws + r.interactive_draws,
                r.resident_bytes as f64 / (1024.0 * 1024.0)
            );
            assert!(vis_near > 0, "{stem} must submit near terrain draws");
            assert!(
                vis_horizon > 0 && vis_horizon_tris > 1_000,
                "{stem} must submit SuperBowl landscape draws (got {vis_horizon}, {vis_horizon_tris}t)"
            );
            assert!(vis_obj > 0, "{stem} must submit scenery object draws");
        }
    }
}
