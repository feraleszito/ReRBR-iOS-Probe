use rerbr_probe::harwood::neighbors;
use std::{path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let Some(maps) = std::env::args_os().nth(1) else {
        eprintln!("Usage: harwood-neighbors-diagnostic <private Maps folder>");
        return ExitCode::FAILURE;
    };
    match neighbors::load(&PathBuf::from(maps)) {
        Ok(set) => {
            println!("{}", set.report);
            println!("CPU connected-region load PASS; GPU/Metal rendering UNTESTED");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("harwood-neighbors FAIL: {error}");
            ExitCode::FAILURE
        }
    }
}
