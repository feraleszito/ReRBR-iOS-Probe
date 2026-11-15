fn main() {
    let Some(root) = std::env::args_os().nth(1) else {
        eprintln!("usage: car-chase-diagnostic <private install root>");
        std::process::exit(2)
    };
    let root = std::path::Path::new(&root);
    match rerbr_probe::car::prepare(root, &root.join("Maps")) {
        Ok(p) => println!(
            "CPU Corolla+route PASS: {} required car files, {:?}, {:.2}m; GPU/Metal UNTESTED",
            p.car.files.len(),
            p.car.geometry.stats(None),
            p.path.length()
        ),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1)
        }
    }
}
