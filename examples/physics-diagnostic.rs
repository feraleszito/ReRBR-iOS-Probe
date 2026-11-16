use rerbr_sim::{inputs::Paths, run, STACK_BYTES};
use std::{path::PathBuf, process::ExitCode, sync::atomic::AtomicBool};
fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        eprintln!(
            "Usage: physics-diagnostic <private RBR installation> <compatible NGP package root>"
        );
        return ExitCode::FAILURE;
    }
    let install = PathBuf::from(&args[0]);
    let paths = Paths {
        maps: install.join("Maps"),
        install,
        overlay: Some(PathBuf::from(&args[1])),
    };
    let worker = std::thread::Builder::new().name("pure-physics-probe".into()).stack_size(STACK_BYTES).spawn(move || {
        run(&paths, &AtomicBool::new(false), |p| {
            if p.steps == 0 || p.steps % 360 == 0 {
                eprintln!("physics-probe: pass={} phase={} steps={} payload={} metadata={} footprint={:?}", p.pass,p.phase,p.steps,p.payload_bytes,p.metadata_bytes,p.footprint_bytes);
            }
        })
    });
    match worker
        .map_err(|e| e.to_string())
        .and_then(|w| w.join().map_err(|_| "worker panicked".to_string()))
        .and_then(|r| r)
    {
        Ok(report) => {
            println!("CPU pure-physics probe PASS: {report:?}; iOS/Metal UNTESTED");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
