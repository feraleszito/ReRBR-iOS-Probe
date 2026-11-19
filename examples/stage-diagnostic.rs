//! Read-only CPU stage diagnostic & full multi-stage compatibility auditor.
//!
//! Usage:
//!   stage-diagnostic MAPS [ROOT]
//!   stage-diagnostic --audit-all <RBR_ROOT> [--csv <path>] [--json <path>] [--sim-boot] [--stem <stem>...]

use std::path::{Path, PathBuf};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--audit-all") {
        return run_audit_all(&args[1..]);
    }

    let maps = args.first().ok_or("usage: stage-diagnostic MAPS [ROOT] | stage-diagnostic --audit-all <RBR_ROOT> [--csv <out.csv>] [--json <out.json>] [--sim-boot] [--stem <stem>...]")?;
    let stage = rerbr_probe::harwood::stage::load(Path::new(maps))?;
    if let Some(root) = args.get(1) {
        println!(
            "AUDIO CPU AUDIT: {:?}",
            rerbr_sim::audio_audit::inspect(Path::new(root))
        );
    }
    println!("CPU STAGE PASS: {:?}", stage.report);
    for (id, info) in stage.render.texture_info() {
        println!(
            "PRIVATE TEXTURE {id}: {} -> {}",
            info.request, info.resolved
        );
    }
    let camera = stage.render.camera;
    let vp = camera.view_projection(1.55);
    let visible = stage
        .render
        .mesh
        .draws
        .iter()
        .filter(|d| {
            rerbr_probe::harwood::stage::visible(
                rerbr_probe::harwood::stage::draw_bounds(&stage.render.mesh, d),
                vp,
                camera.eye,
            )
        })
        .count();
    println!(
        "CPU spawn culling: {visible}/{} draws; no GPU/image/device result",
        stage.report.draws
    );
    Ok(())
}

fn run_audit_all(args: &[String]) -> Result<(), String> {
    let rbr_root = PathBuf::from(args.first().ok_or("missing <RBR_ROOT> after --audit-all")?);
    let mut csv_path: Option<PathBuf> = None;
    let mut json_path: Option<PathBuf> = None;
    let mut sim_boot = false;
    let mut presets_only = false;
    let mut filter_stems: Vec<String> = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--csv" => {
                i += 1;
                csv_path = Some(PathBuf::from(
                    args.get(i).ok_or("missing path after --csv")?,
                ));
            }
            "--json" => {
                i += 1;
                json_path = Some(PathBuf::from(
                    args.get(i).ok_or("missing path after --json")?,
                ));
            }
            "--sim-boot" => sim_boot = true,
            "--presets-only" => presets_only = true,
            "--stem" => {
                i += 1;
                filter_stems.push(args.get(i).ok_or("missing stem after --stem")?.to_string());
            }
            other => return Err(format!("unknown flag: {other}")),
        }
        i += 1;
    }

    let rows = if presets_only {
        let maps_dir = rbr_root.join("Maps");
        let rsf_names = rerbr_probe::harwood::stage_audit::load_rsf_stage_names(&rbr_root);
        rerbr_content::StageDescriptor::presets()
            .iter()
            .map(|preset| {
                rerbr_probe::harwood::stage_audit::audit_stem(
                    &rbr_root,
                    &maps_dir,
                    &preset.stem,
                    Some(preset),
                    &rsf_names,
                    sim_boot,
                )
            })
            .collect()
    } else if filter_stems.is_empty() {
        rerbr_probe::harwood::stage_audit::audit_all_maps(&rbr_root, sim_boot)
    } else {
        let maps_dir = rbr_root.join("Maps");
        let rsf_names = rerbr_probe::harwood::stage_audit::load_rsf_stage_names(&rbr_root);
        let presets = rerbr_content::StageDescriptor::presets();
        filter_stems
            .iter()
            .map(|stem| {
                let preset = presets.iter().find(|p| p.stem.eq_ignore_ascii_case(stem));
                rerbr_probe::harwood::stage_audit::audit_stem(
                    &rbr_root, &maps_dir, stem, preset, &rsf_names, sim_boot,
                )
            })
            .collect()
    };

    for r in &rows {
        println!(
            "{:<12} | {:<22} | {:<7} | blks={}/{} (far={} emp={} rej={}) | route={}/{} col={}/{} gap={:.2}m | dls=S:{:?} F:{:?} sp:{} pn:{} | tex={}/{} ram={:.1}MiB | load={:?} sim={:?} | {} | {:?}",
            r.stem,
            r.stage_name,
            r.surface,
            r.near_loaded_blocks,
            r.lbs_total_blocks,
            r.far_only_blocks,
            r.empty_blocks,
            r.rejected_blocks,
            r.supported_points,
            r.route_points,
            r.collision_matched_points,
            r.route_points,
            r.max_ground_gap_m,
            r.dls_start_m,
            r.dls_finish_m,
            r.dls_splits,
            r.dls_pacenotes,
            r.textures_loaded,
            r.textures_needed,
            r.resident_bytes as f64 / (1024.0 * 1024.0),
            r.stage_load_ok,
            r.sim_boot_ok,
            r.status.as_str(),
            r.errors,
        );
        if !r.anomalies.is_empty() {
            for a in &r.anomalies {
                println!(
                    "   ANOMALY blk#{:<3} {}: near_tris={} far_draws={} bad={} skip={} views={:?} chunks={:?}",
                    a.block_id,
                    a.kind,
                    a.near_triangles,
                    a.far_draws,
                    a.bad_triangles,
                    a.skipped_draws,
                    a.view_slots,
                    a.chunks
                );
            }
        }
    }

    if let Some(path) = csv_path {
        let csv = rerbr_probe::harwood::stage_audit::to_csv(&rows);
        std::fs::write(&path, csv).map_err(|e| format!("write {}: {e}", path.display()))?;
        println!("WROTE CSV: {}", path.display());
    }
    if let Some(path) = json_path {
        let json = rerbr_probe::harwood::stage_audit::to_json(&rows);
        std::fs::write(&path, json).map_err(|e| format!("write {}: {e}", path.display()))?;
        println!("WROTE JSON: {}", path.display());
    }
    Ok(())
}
