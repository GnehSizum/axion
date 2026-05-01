mod cli;
mod commands;
mod error;

use clap::Parser;

use crate::cli::{Cli, Command};
use crate::error::AxionCliError;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), AxionCliError> {
    let cli = Cli::parse();
    match cli.command {
        Command::Dev(args) => commands::dev::run(args),
        Command::Build(args) => commands::build::run(args),
        Command::Bundle(args) => commands::bundle::run(args),
        Command::Check(args) => commands::check::run(args),
        Command::Doctor(args) => commands::doctor::run(args),
        Command::GuiSmoke(args) => commands::gui_smoke::run(args),
        Command::New(args) => commands::run_new(args),
        Command::Report(args) => commands::report::run(args),
        Command::Release(args) => commands::release::run(args),
        Command::SelfTest(args) => commands::self_test::run(args),
    }
}
