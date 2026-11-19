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
}
