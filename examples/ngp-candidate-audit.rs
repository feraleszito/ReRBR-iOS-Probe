//! Read-only research diagnostic, never a runtime support-enabling command.
use rerbr_sim::ngp_candidate;
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if !(args.len() == 2 || args.len() == 3) {
        return Err("usage: ngp-candidate-audit REFERENCE.dll CANDIDATE.dll [physics.rbz]".into());
    }
    println!("RESEARCH ONLY / NO DLL EXECUTION / NO RUNTIME ENABLEMENT");
    let comparison = ngp_candidate::compare_binaries(&args[0], &args[1])?;
    print!("{}", comparison.render());
    let bytes = std::fs::read(&args[1]).map_err(|e| e.to_string())?;
    match ngp_candidate::validate_runtime(&bytes) {
        Ok(()) => println!("Runtime binary policy: existing baseline DLL; vehicle CRC and physics parity are separate checks"),
        Err(reason) => println!("Runtime binary policy: {reason}"),
    }
    if let Some(archive) = args.get(2) {
        let evidence = ngp_candidate::inspect_archive(archive)?;
        println!(
            "ARCHIVE sha256={} revision={:?}",
            evidence.sha256, evidence.physics_revision
        );
        for car in &evidence.vehicles {
            println!(
                "VEHICLE {} CarId={:?} / ORIGINAL NGP CRC UNVERIFIED",
                car.folder, car.car_id
            );
            for surface in &car.surfaces {
                println!(
                    "  {}: CarId={:?} NGP specs readable={} missing keys={} common SHA256={} surface SHA256={:?} options SHA256={:?}",
                    surface.surface, surface.car_id, surface.ngp_specs.is_some(), surface.missing_ngp_keys.len(),
                    surface.common_sha256, surface.surface_sha256, surface.options_sha256
                );
            }
        }
        println!("Archive reads verify ZIP entry CRCs only; no vehicle is certified genuine by this diagnostic.");
    }
    Ok(())
}
