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
    GuiSmoke(GuiSmokeArgs),
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

    #[arg(long, default_value_t = false)]
    pub watch: bool,

    #[arg(long, default_value_t = false)]
    pub reload: bool,

    #[arg(long, default_value_t = false)]
    pub open_devtools: bool,

    #[arg(long)]
    pub frontend_command: Option<String>,

    #[arg(long)]
    pub frontend_cwd: Option<PathBuf>,

    #[arg(long)]
    pub dev_server_timeout_ms: Option<u64>,
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

    #[arg(long, default_value_t = false)]
    pub json: bool,

    #[arg(long, default_value_t = false)]
    pub deny_warnings: bool,

    #[arg(long, value_enum)]
    pub max_risk: Option<DoctorRisk>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum DoctorRisk {
    Low,
    Medium,
    High,
}

impl DoctorRisk {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone, Args)]
pub struct GuiSmokeArgs {
    #[arg(long, default_value = "axion.toml")]
    pub manifest_path: PathBuf,

    #[arg(long)]
    pub report_path: Option<PathBuf>,

    #[arg(long)]
    pub cargo_target_dir: Option<PathBuf>,

    #[arg(long, value_name = "KEY=VALUE")]
    pub build_env: Vec<String>,

    #[arg(long, default_value_t = false)]
    pub serial_build: bool,

    #[arg(long)]
    pub timeout_ms: Option<u64>,

    #[arg(long, default_value_t = false)]
    pub quiet: bool,
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
    pub quiet: bool,

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
