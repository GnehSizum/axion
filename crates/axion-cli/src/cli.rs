use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "axion", version, about = "Axion framework command line")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Dev(DevArgs),
    Build(BuildArgs),
    Bundle(BundleArgs),
    Doctor(DoctorArgs),
    New(NewArgs),
    SelfTest(SelfTestArgs),
}

#[derive(Debug, Clone, Args)]
pub struct DevArgs {
    #[arg(long, default_value = "axion.toml")]
    pub manifest_path: PathBuf,

    #[arg(long, default_value_t = false)]
    pub launch: bool,

    #[arg(long, default_value_t = false)]
    pub fallback_packaged: bool,
}

#[derive(Debug, Clone, Args)]
pub struct BuildArgs {
    #[arg(long, default_value = "axion.toml")]
    pub manifest_path: PathBuf,

    #[arg(long)]
    pub output_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct BundleArgs {
    #[arg(long, default_value = "axion.toml")]
    pub manifest_path: PathBuf,

    #[arg(long)]
    pub output_dir: Option<PathBuf>,

    #[arg(long)]
    pub executable: Option<PathBuf>,

    #[arg(long, default_value_t = false)]
    pub build_executable: bool,
}

#[derive(Debug, Clone, Args)]
pub struct DoctorArgs {
    #[arg(long, default_value = "axion.toml")]
    pub manifest_path: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct SelfTestArgs {
    #[arg(long, default_value = "axion.toml")]
    pub manifest_path: PathBuf,

    #[arg(long)]
    pub output_dir: Option<PathBuf>,

    #[arg(long)]
    pub report_path: Option<PathBuf>,

    #[arg(long, default_value_t = false)]
    pub json: bool,

    #[arg(long, default_value_t = false)]
    pub keep_artifacts: bool,
}

#[derive(Debug, Clone, Args)]
pub struct NewArgs {
    #[arg(default_value = "axion-app")]
    pub name: String,

    #[arg(long)]
    pub path: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = NewTemplate::Vanilla)]
    pub template: NewTemplate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum NewTemplate {
    Vanilla,
}
