pub mod build;
pub mod bundle;
pub mod check;
mod check_identity;
pub mod dev;
pub mod doctor;
pub mod gui_smoke;
pub mod release;
pub mod report;
pub mod report_util;
pub mod self_test;

use std::path::{Path, PathBuf};

use crate::cli::{CheckArgs, DoctorRisk, NewArgs, NewTemplate};
use crate::error::AxionCliError;

const TEMPLATE_APP_ICON: &[u8] = include_bytes!("../../assets/app.icns");
const TEMPLATE_RUST_TOOLCHAIN: &str = include_str!("../../../../rust-toolchain.toml");
#[cfg(target_os = "macos")]
const TEMPLATE_CARGO_CONFIG: &str = include_str!("../../../../.cargo/config.macos.example.toml");
#[cfg(not(target_os = "macos"))]
const TEMPLATE_CARGO_CONFIG: &str = include_str!("../../../../.cargo/config.example.toml");

pub fn run_new(args: NewArgs) -> Result<(), AxionCliError> {
    let run_check = args.run_check;
    let project = NewProject::new(args)?;
    project.write()?;

    println!("Axion application created");
    println!("name: {}", project.name);
    println!("template: {}", project.template.name());
    println!("template_focus: {}", project.template_summary());
    println!("path: {}", project.root.display());
    println!("next:");
    for step in project.next_steps() {
        println!("  {step}");
    }
    if run_check {
        println!("running initial check:");
        check::run(CheckArgs {
            manifest_path: project.root.join("axion.toml"),
            max_risk: DoctorRisk::Medium,
            bundle: true,
            dev: true,
            report_path: None,
            json: false,
            keep_artifacts: false,
        })?;
        println!(
            "note: run gui-smoke from the Axion checkout with --manifest-path {} and --cargo-target-dir target",
            project.root.join("axion.toml").display()
        );
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct NewProject {
    name: String,
    root: PathBuf,
    axion_root: PathBuf,
    template: NewTemplate,
}

impl NewProject {
    fn new(args: NewArgs) -> Result<Self, AxionCliError> {
        let name = normalize_project_name(&args.name);
        let axion_root = axion_root_for_templates()?;
        let current_dir = std::env::current_dir()?;
        let root = args.path.unwrap_or_else(|| current_dir.join(&name));

        Ok(Self {
            name,
            root,
            axion_root,
            template: args.template,
        })
    }

    fn write(&self) -> Result<(), AxionCliError> {
        if self.root.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("project path already exists: {}", self.root.display()),
            )
            .into());
        }

        std::fs::create_dir_all(self.root.join("src"))?;
        std::fs::create_dir_all(self.root.join("frontend"))?;
        std::fs::create_dir_all(self.root.join("icons"))?;
        std::fs::create_dir_all(self.root.join(".cargo"))?;
        std::fs::write(self.root.join("Cargo.toml"), self.cargo_toml())?;
        std::fs::write(
            self.root.join("rust-toolchain.toml"),
            TEMPLATE_RUST_TOOLCHAIN,
        )?;
        std::fs::write(
            self.root.join(".cargo").join("config.toml"),
            TEMPLATE_CARGO_CONFIG,
        )?;
        std::fs::write(self.root.join(".gitignore"), self.gitignore())?;
        std::fs::write(self.root.join("README.md"), self.readme())?;
        std::fs::write(self.root.join("axion.toml"), self.manifest())?;
        std::fs::write(self.root.join("icons").join("app.icns"), TEMPLATE_APP_ICON)?;
        std::fs::write(self.root.join("src").join("main.rs"), self.main_rs())?;
        std::fs::write(
            self.root.join("frontend").join("index.html"),
            self.index_html(),
        )?;
        std::fs::write(
            self.root.join("frontend").join("style.css"),
            self.style_css(),
        )?;
        std::fs::write(self.root.join("frontend").join("app.js"), self.app_js())?;
        Ok(())
    }

    fn next_steps(&self) -> Vec<String> {
        let manifest = self.root.join("axion.toml").display().to_string();
        vec![
            format!("cd {}", self.root.display()),
            "cargo run -- --plan".to_owned(),
            "cargo run --features servo-runtime".to_owned(),
            format!(
                "from Axion checkout: cargo run -p axion-cli -- check --manifest-path {manifest} --dev --bundle --json --report-path target/axion/reports/check.json"
            ),
            format!(
                "from Axion checkout: cargo run -p axion-cli -- gui-smoke --manifest-path {manifest} --report-path target/axion/reports/gui-smoke.json --timeout-ms 30000 --require-check bridge.bootstrap --require-check app.ping --require-check input.snapshot --require-command app.ping --require-command window.info --require-host-event window.ready --require-window main --cargo-target-dir target --serial-build"
            ),
            format!(
                "from Axion checkout: cargo run -p axion-cli --features servo-runtime -- dev --manifest-path {manifest} --launch --fallback-packaged --watch --reload --restart-on-change --event-log target/axion/reports/dev-events.jsonl --report-path target/axion/reports/dev-report.json"
            ),
            format!(
                "from Axion checkout: cargo run -p axion-cli -- release --manifest-path {manifest} --check-report-path target/axion/reports/check.json --json --report-path target/axion/reports/release.json --bundle-report-path target/axion/reports/bundle.json --archive --archive-path target/axion/reports/bundle.tar"
            ),
            "from Axion checkout: cargo run -p axion-cli -- report target/axion/reports/release.json --output target/axion/reports/release-summary.json".to_owned(),
            "from Axion checkout: cargo run -p axion-cli -- report target/axion/reports/gui-smoke.json --allow-failed --output target/axion/reports/gui-smoke-summary.json".to_owned(),
        ]
    }

    fn template_summary(&self) -> &'static str {
        match self.template {
            NewTemplate::Vanilla => {
                "Bridge, lifecycle, native API, custom command, capability-denial, and bundle demos"
            }
            NewTemplate::NativeApiDemo => {
                "Focused clipboard, shell, app-data filesystem, dialog, app/window API, and diagnostics demo"
            }
        }
    }

    fn template_eyebrow(&self) -> &'static str {
        match self.template {
            NewTemplate::Vanilla => "Axion Vanilla Template",
            NewTemplate::NativeApiDemo => "Axion Native API Demo",
        }
    }

    fn native_api_heading(&self) -> &'static str {
        match self.template {
            NewTemplate::Vanilla => "Native Preview APIs",
            NewTemplate::NativeApiDemo => "Native API Workbench",
        }
    }

    fn gui_smoke_source(&self) -> &'static str {
        match self.template {
            NewTemplate::Vanilla => "axion-vanilla-template",
            NewTemplate::NativeApiDemo => "axion-native-api-demo-template",
        }
    }

    fn native_api_focus_section(&self) -> &'static str {
        match self.template {
            NewTemplate::Vanilla => "",
            NewTemplate::NativeApiDemo => {
                r#"
## Native API Demo Focus

This template is tuned for validating Axion's preview native API surface. The UI highlights app/window metadata, clipboard round trips, shell URL validation, app-data file lifecycle operations, headless dialog responses, capability denial, input compatibility, and the GUI smoke hook used by `axion gui-smoke`. Use the "Run all checks" button in the Native API Workbench card for a manual in-window check pass.
"#
            }
        }
    }

    fn native_api_controls_html(&self) -> &'static str {
        match self.template {
            NewTemplate::Vanilla => "",
            NewTemplate::NativeApiDemo => {
                "\n          <p class=\"hint\">Run the same bridge checks used by GUI smoke and inspect the structured result.</p>\n          <button id=\"run-native-api-checks\" type=\"button\">Run all checks</button>"
            }
        }
    }

    fn readme(&self) -> String {
        include_str!("../../templates/README.md")
            .replace("@TITLE@", &title_case(&self.name))
            .replace("@TEMPLATE@", self.template.name())
            .replace("@SUMMARY@", self.template_summary())
            .replace("@NATIVE_API_FOCUS@", self.native_api_focus_section())
            .replace(
                "@MANIFEST@",
                &self.root.join("axion.toml").display().to_string(),
            )
    }

    fn gitignore(&self) -> String {
        "target/\n.DS_Store\n*.log\n".to_owned()
    }

    fn cargo_toml(&self) -> String {
        format!(
            "[package]\nname = {name:?}\nversion = \"0.6.1\"\nedition = \"2024\"\nrust-version = \"1.88.0\"\n\n[features]\ndefault = []\nservo-runtime = [\"axion-runtime/servo-runtime\"]\n\n[dependencies]\naxion-core = {{ path = {core:?} }}\naxion-manifest = {{ path = {manifest:?} }}\naxion-runtime = {{ path = {runtime:?} }}\n",
            name = self.name,
            core = self
                .axion_root
                .join("crates")
                .join("axion-core")
                .display()
                .to_string(),
            manifest = self
                .axion_root
                .join("crates")
                .join("axion-manifest")
                .display()
                .to_string(),
            runtime = self
                .axion_root
                .join("crates")
                .join("axion-runtime")
                .display()
                .to_string(),
        )
    }

    fn manifest(&self) -> String {
        let description = match self.template {
            NewTemplate::Vanilla => "Generated Axion application",
            NewTemplate::NativeApiDemo => "Generated Axion native API demo application",
        };
        format!(
            "[app]\nname = {name:?}\nidentifier = \"dev.axion.{identifier}\"\nversion = \"0.1.0\"\ndescription = {description:?}\nauthors = [\"Axion Developer\"]\nhomepage = \"https://example.dev/{name}\"\n\n[window]\nid = \"main\"\ntitle = {title:?}\nwidth = 960\nheight = 720\nresizable = true\nvisible = true\n\n[build]\nfrontend_dist = \"frontend\"\nentry = \"frontend/index.html\"\n\n# To use `axion dev --launch` with a frontend dev server, uncomment and update:\n# [dev]\n# url = \"http://127.0.0.1:3000\"\n# command = \"python3 -m http.server 3000 --bind 127.0.0.1 --directory frontend\"\n# timeout_ms = 15000\n\n[bundle]\nicon = \"icons/app.icns\"\n\n[native.dialog]\nbackend = \"headless\"\n\n[native.clipboard]\nbackend = \"memory\"\n\n[native.lifecycle]\nclose_timeout_ms = 3000\n\n[capabilities.main]\nprofiles = [\"app-info\", \"app-control\", \"window-control\", \"clipboard-access\", \"shell-access\", \"file-access\", \"dialog-access\", \"app-events\"]\ncommands = [\"demo.greet\"]\nallowed_navigation_origins = []\nallow_remote_navigation = false\n",
            name = self.name,
            identifier = self.name.replace('-', "."),
            description = description,
            title = title_case(&self.name),
        )
    }

    fn main_rs(&self) -> String {
        include_str!("../../templates/main.rs").to_owned()
    }

    fn index_html(&self) -> String {
        include_str!("../../templates/index.html")
            .replace("@TITLE@", &title_case(&self.name))
            .replace("@EYEBROW@", self.template_eyebrow())
            .replace("@NATIVE_API_HEADING@", self.native_api_heading())
            .replace("@NATIVE_API_CONTROLS@", self.native_api_controls_html())
    }

    fn style_css(&self) -> String {
        include_str!("../../templates/style.css").to_owned()
    }

    fn app_js(&self) -> String {
        include_str!("../../templates/app.js")
            .replace("@APP_NAME@", &self.name)
            .replace("@GUI_SMOKE_SOURCE@", self.gui_smoke_source())
    }
}

impl NewTemplate {
    fn name(self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::NativeApiDemo => "native-api-demo",
        }
    }
}

fn axion_root_for_templates() -> Result<PathBuf, AxionCliError> {
    let cli_manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    if let Some(axion_root) = cli_manifest_dir
        .parent()
        .and_then(Path::parent)
        .filter(|path| path.join("crates").join("axion-core").exists())
    {
        return Ok(axion_root.to_path_buf());
    }

    Ok(std::env::current_dir()?)
}

fn normalize_project_name(name: &str) -> String {
    let normalized = name
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_owned();

    if normalized.is_empty() {
        "axion-app".to_owned()
    } else {
        normalized
    }
}

fn title_case(name: &str) -> String {
    name.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::{NewProject, axion_root_for_templates, normalize_project_name, title_case};
    use crate::cli::NewTemplate;

    #[test]
    fn project_name_is_normalized_for_package_use() {
        assert_eq!(normalize_project_name("Hello Axion!"), "hello-axion");
        assert_eq!(normalize_project_name("  "), "axion-app");
    }

    #[test]
    fn title_case_expands_kebab_name() {
        assert_eq!(title_case("hello-axion"), "Hello Axion");
    }

    #[test]
    fn template_axion_root_points_at_checkout_root() {
        let root = axion_root_for_templates().expect("template root should resolve");
        assert!(root.join("crates").join("axion-core").exists());
    }

    #[test]
    fn generated_frontend_uses_external_script_for_csp() {
        let root = axion_root_for_templates().expect("template root should resolve");
        let project = NewProject {
            name: "hello-axion".to_owned(),
            root: std::path::PathBuf::from("unused"),
            axion_root: root,
            template: NewTemplate::Vanilla,
        };

        assert!(
            project
                .index_html()
                .contains("<script src=\"app.js\"></script>")
        );
        assert!(
            project
                .index_html()
                .contains("<link rel=\"stylesheet\" href=\"style.css\">")
        );
        assert!(!project.index_html().contains("window.addEventListener"));
        assert!(project.app_js().contains("window.addEventListener"));
        assert!(project.app_js().contains("axion.emit('app.log'"));
        assert!(project.app_js().contains("axion.invoke('demo.greet'"));
        assert!(project.app_js().contains("axion.invoke('demo.missing'"));
        assert!(project.app_js().contains("installTextInputSelectionPatch"));
        assert!(project.app_js().contains("describeBridge"));
        assert!(project.app_js().contains("snapshotTextControl"));
        assert!(project.app_js().contains("textarea-tab-handler"));
        assert!(project.app_js().contains("window.set_title"));
        assert!(project.app_js().contains("window.set_size"));
        assert!(project.app_js().contains("app.exit.available"));
        assert!(project.app_js().contains("window.close.available"));
        assert!(project.app_js().contains("shell.open.available"));
        assert!(project.app_js().contains("window.close_decision.available"));
        assert!(project.app_js().contains("clipboard.write_text"));
        assert!(project.app_js().contains("clipboard.read_text"));
        assert!(project.app_js().contains("clipboard.roundtrip"));
        assert!(project.app_js().contains("shell.open"));
        assert!(project.app_js().contains("dialog.open"));
        assert!(project.app_js().contains("dialog.save"));
        assert!(project.app_js().contains("window.__AXION_GUI_SMOKE__"));
        assert!(project.app_js().contains("axion-vanilla-template"));
        assert!(project.app_js().contains("input.snapshot"));
        assert!(project.index_html().contains("compat-input"));
        assert!(project.index_html().contains("compat-textarea"));
        assert!(project.index_html().contains("compat-diagnostics"));
        assert!(project.style_css().contains(".field"));
        assert!(project.style_css().contains(".grid"));
    }

    #[test]
    fn generated_project_includes_custom_command_plugin() {
        let root = axion_root_for_templates().expect("template root should resolve");
        let project = NewProject {
            name: "hello-axion".to_owned(),
            root: std::path::PathBuf::from("unused"),
            axion_root: root,
            template: NewTemplate::Vanilla,
        };

        assert!(project.manifest().contains("\"demo.greet\""));
        assert!(project.manifest().contains("\"app-info\""));
        assert!(project.manifest().contains("\"app-control\""));
        assert!(project.manifest().contains("\"window-control\""));
        assert!(project.manifest().contains("\"clipboard-access\""));
        assert!(project.manifest().contains("\"shell-access\""));
        assert!(project.manifest().contains("\"file-access\""));
        assert!(project.manifest().contains("\"dialog-access\""));
        assert!(project.manifest().contains("\"app-events\""));
        assert!(project.manifest().contains("[bundle]"));
        assert!(project.manifest().contains("icon = \"icons/app.icns\""));
        assert!(project.manifest().contains("[native.dialog]"));
        assert!(project.manifest().contains("backend = \"headless\""));
        assert!(project.manifest().contains("[native.clipboard]"));
        assert!(project.manifest().contains("backend = \"memory\""));
        assert!(project.manifest().contains("[native.lifecycle]"));
        assert!(project.manifest().contains("close_timeout_ms = 3000"));
        assert!(project.manifest().contains("# [dev]"));
        assert!(
            project
                .manifest()
                .contains("# url = \"http://127.0.0.1:3000\"")
        );
        assert!(
            project
                .manifest()
                .contains("# command = \"python3 -m http.server 3000")
        );
        assert!(project.manifest().contains("# timeout_ms = 15000"));
        assert!(project.main_rs().contains("struct DemoPlugin"));
        assert!(
            project
                .main_rs()
                .contains("register_command(\"demo.greet\"")
        );
        assert!(project.main_rs().contains("run_with_plugins"));
        assert!(project.main_rs().contains("json_string_literal"));
        assert!(project.main_rs().contains("install_panic_reporter"));
        assert!(project.main_rs().contains("app_data_dir_for_config"));
        assert!(project.main_rs().contains("load_app_config_for_executable"));
        assert!(project.main_rs().contains("crash-reports"));
    }

    #[test]
    fn native_api_demo_template_focuses_generated_surface() {
        let root = axion_root_for_templates().expect("template root should resolve");
        let project = NewProject {
            name: "native-lab".to_owned(),
            root: std::path::PathBuf::from("/tmp/native-lab"),
            axion_root: root,
            template: NewTemplate::NativeApiDemo,
        };

        assert_eq!(project.template.name(), "native-api-demo");
        assert!(
            project
                .readme()
                .contains("Generated by Axion using the `native-api-demo` template.")
        );
        assert!(project.readme().contains("Native API Demo Focus"));
        assert!(project.readme().contains("Capability Profiles"));
        assert!(project.readme().contains("docs/capabilities.md"));
        assert!(project.readme().contains("Run all checks"));
        assert!(project.readme().contains(
            "clipboard round trips, shell URL validation, app-data file lifecycle operations"
        ));
        assert!(project.index_html().contains("Axion Native API Demo"));
        assert!(project.index_html().contains("Native API Workbench"));
        assert!(project.index_html().contains("run-native-api-checks"));
        assert!(project.index_html().contains("Run all checks"));
        assert!(
            project
                .manifest()
                .contains("Generated Axion native API demo application")
        );
        assert!(project.app_js().contains("axion-native-api-demo-template"));
        assert!(project.app_js().contains("runNativeApiChecks"));
        assert!(project.app_js().contains("Native API checks"));
        assert!(project.app_js().contains("clipboard.roundtrip"));
        assert!(project.app_js().contains("native.expected_errors"));
        assert!(project.app_js().contains("clipboard.invalid-payload"));
        assert!(project.app_js().contains("dialog.invalid-payload"));
        assert!(project.app_js().contains("window.invalid-size"));
        assert!(project.app_js().contains("shell.invalid-payload"));
        assert!(project.app_js().contains("shell.open"));
        assert!(project.app_js().contains("fs.create_dir"));
        assert!(project.app_js().contains("fs.exists"));
        assert!(project.app_js().contains("fs.list_dir"));
        assert!(project.app_js().contains("fs.remove"));
        assert!(project.app_js().contains("fs.expected_errors"));
        assert!(project.app_js().contains("fs.directory-not-empty"));
        assert!(project.app_js().contains("normalizeError"));
        assert!(project.app_js().contains("dialog.open"));
        assert!(project.app_js().contains("fs.write_text"));
        assert!(project.style_css().contains("button:disabled"));
        assert!(project.cargo_toml().contains("version = \"0.6.1\""));
    }

    #[test]
    fn generated_project_readme_documents_template_and_commands() {
        let root = axion_root_for_templates().expect("template root should resolve");
        let project = NewProject {
            name: "hello-axion".to_owned(),
            root: std::path::PathBuf::from("/tmp/hello-axion"),
            axion_root: root,
            template: NewTemplate::Vanilla,
        };

        let readme = project.readme();
        assert!(readme.contains("Generated by Axion using the `vanilla` template."));
        assert!(readme.contains("Bridge, lifecycle, native API"));
        assert!(readme.contains("cargo run -- --plan"));
        assert!(readme.contains("cargo run --features servo-runtime"));
        assert!(readme.contains("rust-toolchain.toml"));
        assert!(readme.contains(".cargo/config.toml"));
        assert!(readme.contains("MACOSX_DEPLOYMENT_TARGET"));
        assert!(readme.contains("RUSTC_BOOTSTRAP"));
        assert!(readme.contains("/usr/bin/ld"));
        assert!(readme.contains("Servo-compatible Clang"));
        assert!(readme.contains("profile stripping"));
        assert!(readme.contains("Frontend Development"));
        assert!(readme.contains("cargo run -p axion-cli -- dev"));
        assert!(readme.contains("--watch --reload"));
        assert!(readme.contains("--restart-on-change"));
        assert!(readme.contains("axion.dev-report.v1"));
        assert!(readme.contains("window.ready"));
        assert!(readme.contains("--open-devtools"));
        assert!(readme.contains("cargo run -p axion-cli -- gui-smoke"));
        assert!(readme.contains("cargo run -p axion-cli -- report"));
        assert!(readme.contains("--allow-failed"));
        assert!(readme.contains("--output target/axion/reports/release-summary.json"));
        assert!(readme.contains("--output target/axion/reports/gui-smoke-summary.json"));
        assert!(readme.contains("cargo run -p axion-cli -- check"));
        assert!(readme.contains("--dev --bundle"));
        assert!(readme.contains("--json --report-path target/axion/reports/check.json"));
        assert!(readme.contains("--report-path target/axion/reports/check.json"));
        assert!(readme.contains("--event-log target/axion/reports/dev-events.jsonl"));
        assert!(readme.contains("--deny-warnings --max-risk medium"));
        assert!(readme.contains("--cargo-target-dir target"));
        assert!(readme.contains("--serial-build"));
        assert!(readme.contains("cargo run -p axion-cli -- bundle"));
        assert!(readme.contains("--build-executable"));
        assert!(readme.contains("Local Run"));
        assert!(readme.contains("CI Validation"));
        assert!(readme.contains("Release Preview"));
        assert!(readme.contains("target/axion/reports/"));
        assert!(readme.contains("check.json"));
        assert!(readme.contains("release-summary.json"));
        assert!(readme.contains("gui-smoke-summary.json"));
        assert!(readme.contains("verification: ok"));
        assert!(readme.contains("fingerprinted_files"));
        assert!(readme.contains("Custom Command Demo"));
        assert!(readme.contains("Capability Profiles"));
        assert!(readme.contains("check --manifest-path"));
        assert!(readme.contains("docs/capabilities.md"));
        assert!(readme.contains("demo.greet"));
        assert!(readme.contains("Capability denial"));
        assert!(readme.contains("Input Compatibility Demo"));
        assert!(readme.contains("installTextInputSelectionPatch"));
        assert!(readme.contains("window.__AXION_GUI_SMOKE__()"));
        assert!(readme.contains("frontend/style.css"));
        assert!(readme.contains("dialog.open"));
        assert!(readme.contains("Runtime Artifacts"));
        assert!(readme.contains("operating system user-data directory"));
        assert!(readme.contains("crash-reports/"));
        assert!(readme.contains(".gitignore"));
        assert!(readme.contains("icons/app.icns"));
    }

    #[test]
    fn generated_project_next_steps_use_report_artifacts() {
        let root = axion_root_for_templates().expect("template root should resolve");
        let project = NewProject {
            name: "hello-axion".to_owned(),
            root: std::path::PathBuf::from("/tmp/hello-axion"),
            axion_root: root,
            template: NewTemplate::Vanilla,
        };

        let next_steps = project.next_steps();
        assert!(next_steps.iter().any(|step| step
            == "from Axion checkout: cargo run -p axion-cli -- check --manifest-path /tmp/hello-axion/axion.toml --dev --bundle --json --report-path target/axion/reports/check.json"));
        assert!(
            next_steps
                .iter()
                .any(|step| step
                    .contains("from Axion checkout: cargo run -p axion-cli -- gui-smoke"))
        );
        assert!(next_steps.iter().any(|step| step.contains(
            "--event-log target/axion/reports/dev-events.jsonl --report-path target/axion/reports/dev-report.json"
        )));
        assert!(next_steps.iter().any(|step| step.contains(
            "from Axion checkout: cargo run -p axion-cli -- release"
        )));
        assert!(next_steps.iter().any(|step| step.contains(
            "from Axion checkout: cargo run -p axion-cli -- report target/axion/reports/gui-smoke.json --allow-failed"
        )));
        assert!(
            next_steps
                .iter()
                .any(|step| step.contains("--output target/axion/reports/release-summary.json"))
        );
        assert!(
            next_steps
                .iter()
                .any(|step| step.contains("--output target/axion/reports/gui-smoke-summary.json"))
        );
    }

    #[test]
    fn generated_project_templates_have_focus_descriptions() {
        let root = axion_root_for_templates().expect("template root should resolve");
        let vanilla = NewProject {
            name: "hello-axion".to_owned(),
            root: std::path::PathBuf::from("/tmp/hello-axion"),
            axion_root: root.clone(),
            template: NewTemplate::Vanilla,
        };
        let native_api = NewProject {
            name: "native-lab".to_owned(),
            root: std::path::PathBuf::from("/tmp/native-lab"),
            axion_root: root,
            template: NewTemplate::NativeApiDemo,
        };

        assert!(vanilla.template_summary().contains("Bridge, lifecycle"));
        assert!(native_api.template_summary().contains("Focused clipboard"));
        assert_ne!(vanilla.template_summary(), native_api.template_summary());
    }

    #[test]
    fn checked_in_examples_document_validation_commands() {
        let root = axion_root_for_templates().expect("template root should resolve");
        for example in [
            "hello-axion",
            "file-access-demo",
            "multi-window",
            "bridge-diagnostics-demo",
        ] {
            let readme_path = root.join("examples").join(example).join("README.md");
            let readme = std::fs::read_to_string(&readme_path)
                .unwrap_or_else(|error| panic!("{}: {error}", readme_path.display()));
            assert!(
                readme.contains("--plan"),
                "{example} README should document --plan"
            );
            assert!(
                readme.contains("check --manifest-path"),
                "{example} README should document check"
            );
            assert!(
                readme.contains("gui-smoke --manifest-path"),
                "{example} README should document gui-smoke"
            );
            assert!(
                readme.contains("bundle --manifest-path"),
                "{example} README should document bundle preview"
            );
            assert!(
                readme.contains("warning") || readme.contains("notice"),
                "{example} README should explain expected warnings or notices"
            );
        }
    }

    #[test]
    fn generated_project_writes_servo_build_configuration() {
        let axion_root = axion_root_for_templates().expect("template root should resolve");
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        for template in [NewTemplate::Vanilla, NewTemplate::NativeApiDemo] {
            let root = std::env::temp_dir().join(format!(
                "axion-new-build-config-{unique}-{}",
                template.name()
            ));
            let project = NewProject {
                name: "hello-axion".to_owned(),
                root: root.clone(),
                axion_root: axion_root.clone(),
                template,
            };
            project.write().expect("template should write successfully");

            let toolchain = std::fs::read_to_string(root.join("rust-toolchain.toml"))
                .expect("generated project should include its Rust toolchain");
            assert_eq!(toolchain, super::TEMPLATE_RUST_TOOLCHAIN);
            assert!(toolchain.contains("channel = \"1.97.1\""));
            let cargo_config = std::fs::read_to_string(root.join(".cargo").join("config.toml"))
                .expect("generated project should include Servo build settings");
            assert_eq!(cargo_config, super::TEMPLATE_CARGO_CONFIG);
            assert!(cargo_config.contains("MACOSX_DEPLOYMENT_TARGET = \"13.0\""));
            assert!(cargo_config.contains(
                "RUSTC_BOOTSTRAP = \"crown,script,script_bindings,script_webgpu,style_tests,mozjs,mozjs_sys\""
            ));
            #[cfg(target_os = "macos")]
            {
                assert!(cargo_config.contains("CC = \"clang --ld-path=/usr/bin/ld"));
                assert!(cargo_config.contains("CXX = \"clang++ --ld-path=/usr/bin/ld"));
                assert!(cargo_config.contains("HOST_CC = \"clang --ld-path=/usr/bin/ld"));
                assert!(cargo_config.contains("HOST_CXX = \"clang++ --ld-path=/usr/bin/ld"));
                assert!(cargo_config.contains("[profile.dev]\nstrip = \"none\""));
                assert!(cargo_config.contains("[profile.release]\nstrip = \"none\""));
            }
            #[cfg(not(target_os = "macos"))]
            {
                assert!(!cargo_config.contains("--ld-path=/usr/bin/ld"));
                assert!(!cargo_config.contains("CARGO_PROFILE_DEV_STRIP"));
                assert!(!cargo_config.contains("CARGO_PROFILE_RELEASE_STRIP"));
            }

            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn generated_project_template_includes_icon_asset() {
        assert!(super::TEMPLATE_APP_ICON.starts_with(b"icns"));
    }

    #[test]
    fn generated_project_gitignore_covers_local_artifacts() {
        let root = axion_root_for_templates().expect("template root should resolve");
        let project = NewProject {
            name: "hello-axion".to_owned(),
            root: std::path::PathBuf::from("unused"),
            axion_root: root,
            template: NewTemplate::Vanilla,
        };

        let gitignore = project.gitignore();
        assert!(gitignore.lines().any(|line| line == "target/"));
        assert!(gitignore.lines().any(|line| line == ".DS_Store"));
        assert!(gitignore.lines().any(|line| line == "*.log"));
    }
}
