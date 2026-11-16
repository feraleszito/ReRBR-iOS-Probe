//! Read-only research diagnostic for one NGP car pack, never a runtime support-enabling command.
use rerbr_sim::ngp_candidate;
use std::path::Path;

fn main() -> Result<(), String> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: ngp-pack-audit PACK")?;
    println!("RESEARCH ONLY / NO DLL EXECUTION / NO RUNTIME ENABLEMENT");
    let evidence = ngp_candidate::inspect_car_pack(Path::new(&path))?;
    println!(
        "PACK sha256={:?} title={:?}",
        evidence.sha256, evidence.title
    );
    if let Some(about) = &evidence.about {
        for line in about.lines() {
            println!("  {line}");
        }
    }
    for surface in &evidence.surfaces {
        println!(
            "SURFACE {} CarId={:?} / ORIGINAL NGP CRC UNVERIFIED",
            surface.surface, surface.car_id
        );
        println!(
            "  common SHA256={} surface SHA256={:?} options SHA256={:?}",
            surface.common_sha256, surface.surface_sha256, surface.options_sha256
        );
        println!(
            "  NGP specs readable={} missing keys={}",
            surface.ngp_specs.is_some(),
            surface.missing_ngp_keys.len()
        );
    }
    println!("A readable pack and stored CarIds do not certify the loaded-value CRC or physics behavior.");
    Ok(())
}
