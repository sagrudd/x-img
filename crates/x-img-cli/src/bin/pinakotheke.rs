// SPDX-License-Identifier: MPL-2.0
//! Canonical Pinakotheke command entry point prepared for the v1 cutover.

use std::process::ExitCode;

use x_img_cli::{Invocation, parse_from, run};

fn main() -> ExitCode {
    let invocation = Invocation::Canonical;
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
