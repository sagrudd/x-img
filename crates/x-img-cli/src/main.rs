// SPDX-License-Identifier: MPL-2.0
//! Command-line entry point for the x-img scaffold.

use std::{path::PathBuf, process::ExitCode};

use clap::{Args, Parser, Subcommand};
use x_img_core::{ConfigStore, build_info};

/// x-img command-line interface.
#[derive(Debug, Parser)]
#[command(
    name = "x-img",
    version,
    about = "x-img media catalogue workspace scaffold"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Strictly validate and inspect a local versioned configuration file.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    /// Validate a configuration file without modifying it.
    Validate(ConfigPath),
    /// List configured account and website rule identifiers without authority references.
    List(ConfigPath),
    /// Validate a complete candidate then atomically replace the destination file.
    Replace(ReplaceConfig),
}

#[derive(Debug, Args)]
struct ConfigPath {
    /// Local JSON configuration file.
    #[arg(long)]
    path: PathBuf,
}

#[derive(Debug, Args)]
struct ReplaceConfig {
    /// Destination local JSON configuration file.
    #[arg(long)]
    path: PathBuf,
    /// Complete candidate JSON file to validate and atomically install.
    #[arg(long)]
    input: PathBuf,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("x-img: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        None => println!(
            "{} workspace scaffold; no live integrations are enabled.",
            build_info().summary()
        ),
        Some(Command::Config { command }) => run_config(command)?,
    }
    Ok(())
}

fn run_config(command: ConfigCommand) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        ConfigCommand::Validate(arguments) => {
            let config = ConfigStore::new(arguments.path).load()?;
            println!(
                "valid {} configuration {}",
                config.schema_version, config.instance_id
            );
        }
        ConfigCommand::List(arguments) => {
            let config = ConfigStore::new(arguments.path).load()?;
            for account in config.x_accounts {
                println!("x\t{}\t{}", account.account_id, account.handle);
            }
            for account in config.instagram_accounts {
                println!("instagram\t{}\t{}", account.account_id, account.username);
            }
            for policy in config.website_policies {
                println!("website\t{}\t{}", policy.site_id, policy.origin);
            }
        }
        ConfigCommand::Replace(arguments) => {
            let candidate = std::fs::read(arguments.input)?;
            ConfigStore::new(arguments.path).replace_from_json(&candidate)?;
            println!("validated and atomically replaced configuration");
        }
    }
    Ok(())
}
