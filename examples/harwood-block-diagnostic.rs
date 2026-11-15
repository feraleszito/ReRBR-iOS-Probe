use rerbr_probe::harwood;
use std::{path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let Some(maps) = std::env::args_os().nth(1) else {
        eprintln!("Usage: harwood-block-diagnostic <private Maps folder>");
        return ExitCode::FAILURE;
    };
    match harwood::load(&PathBuf::from(maps)) {
        Ok(block) => {
            println!(
                "CPU one-block load PASS: selected {} of {}; GPU upload/rendering UNTESTED",
                block.index, block.total_blocks
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("harwood-block FAIL: {error}");
            ExitCode::FAILURE
        }
    }
}
