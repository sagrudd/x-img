// SPDX-License-Identifier: MPL-2.0
//! Legacy x-img compatibility entry point.

use std::process::ExitCode;

use x_img_cli::{Invocation, parse_from, run};

fn main() -> ExitCode {
    execute(Invocation::Legacy)
}

fn execute(invocation: Invocation) -> ExitCode {
    if let Some(notice) = invocation.notice() {
        eprintln!("warning: {notice}");
    }
    let cli = match parse_from(invocation, std::env::args_os()) {
        Ok(cli) => cli,
        Err(error) => error.exit(),
    };
    match run(invocation, cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}: {error}", invocation.command_name());
            ExitCode::FAILURE
        }
    }
}
