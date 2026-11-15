//! Read-only CPU diagnostic. Pass a Maps folder, never an output directory.

use rerbr_probe::asset_probe;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(maps) = std::env::args_os().nth(1) else {
        eprintln!("Usage: harwood-diagnostic <Maps folder>");
        return ExitCode::FAILURE;
    };
    match asset_probe::inspect(Path::new(&maps)) {
        Ok(report) => {
            println!("{report}");
            if report.complete() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("harwood: FAIL: {error}");
            ExitCode::FAILURE
        }
    }
}
