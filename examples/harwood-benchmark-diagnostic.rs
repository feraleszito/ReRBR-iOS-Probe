fn main() {
    let Some(maps) = std::env::args_os().nth(1) else {
        eprintln!("usage: harwood-benchmark-diagnostic <private Maps>");
        std::process::exit(2)
    };
    match rerbr_probe::harwood::benchmark::load(std::path::Path::new(&maps)) {
        Ok(prepared) => println!(
            "CPU route/ground PASS: {:.2}m; GPU/Metal and device benchmark UNTESTED",
            prepared.route.length()
        ),
        Err(error) => {
            eprintln!("BENCHMARK diagnostic FAIL: {error}");
            std::process::exit(1)
        }
    }
}
