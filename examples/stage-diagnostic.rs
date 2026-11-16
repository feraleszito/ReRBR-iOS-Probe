//! Read-only CPU inventory of the owner's private full Harwood stage.
fn main() -> Result<(), String> {
    let maps = std::env::args_os()
        .nth(1)
        .ok_or("usage: stage-diagnostic MAPS")?;
    let stage = rerbr_probe::harwood::stage::load(std::path::Path::new(&maps))?;
    if let Some(root) = std::env::args_os().nth(2) {
        println!(
            "AUDIO CPU AUDIT: {:?}",
            rerbr_sim::audio_audit::inspect(std::path::Path::new(&root))
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
