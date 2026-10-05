use std::path::Path;

use axion_runtime::json_string_literal;

use crate::cli::ReportArgs;
use serde::Deserialize;
use serde_json::Value;

use crate::commands::report_util::{
    json_string_array_literal, json_string_fields, optional_json_string_literal,
};
use crate::error::AxionCliError;

pub fn run(args: ReportArgs) -> Result<(), AxionCliError> {
    let body = std::fs::read_to_string(&args.path)?;
    let summary = ReportSummary::from_json(&args.path, &body)?;
    let summary_json = summary.to_json();

    if args.json {
        println!("{summary_json}");
    } else {
        summary.print_human();
    }

    if let Some(output) = &args.output {
        write_summary_json(output, &summary_json)?;
    }

    if summary.result == "failed" && !args.allow_failed {
        return Err(std::io::Error::other("report result is failed").into());
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReportSummary {
    path: String,
    schema: String,
    kind: String,
    manifest_path: Option<String>,
    result: String,
    failure_phase: Option<String>,
    next_step: Option<String>,
    next_action_kinds: Vec<String>,
    smoke_total: Option<usize>,
    failed_check_ids: Vec<String>,
    error_codes: Vec<String>,
    artifacts: Vec<ReportArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct ReportArtifact {
    kind: String,
    path: String,
    exists: Option<bool>,
}

impl ReportSummary {
    fn from_json(path: &Path, body: &str) -> Result<Self, AxionCliError> {
        let source: SourceReport = serde_json::from_str(body).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid report JSON: {error}"),
            )
        })?;
        let schema = source.schema;
        let kind = report_kind(&schema)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "unsupported report schema '{schema}'; expected one of: {}",
                        supported_report_schemas().join(", ")
                    ),
                )
            })?
            .to_owned();
        if !matches!(source.result.as_str(), "ok" | "failed") {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "report result must be ok or failed",
            )
            .into());
        }
        let manifest_path = source.manifest_path;
        let result = source.result;
        let diagnostics = source.diagnostics.unwrap_or_default();
        let failure_phase = source.failure_phase.or(diagnostics.failure_phase);
        let next_step = source.next_step.or(diagnostics.next_step);
        let next_action_kinds = source
            .next_actions
            .into_iter()
            .map(|action| action.kind)
            .collect();
        let smoke_summary = source
            .smoke_checks
            .as_deref()
            .or(diagnostics.smoke_checks.as_deref())
            .map(smoke_check_summary);
        let artifacts = source.artifacts;

        Ok(Self {
            path: path.display().to_string(),
            schema,
            kind,
            manifest_path,
            result,
            failure_phase,
            next_step,
            next_action_kinds,
            smoke_total: smoke_summary.as_ref().map(|summary| summary.total),
            failed_check_ids: smoke_summary
                .as_ref()
                .map(|summary| summary.failed_ids.clone())
                .unwrap_or_default(),
            error_codes: smoke_summary
                .map(|summary| summary.error_codes)
                .unwrap_or_default(),
            artifacts,
        })
    }

    fn print_human(&self) {
        println!("Axion report");
        println!("path: {}", self.path);
        println!("schema: {}", self.schema);
        println!("kind: {}", self.kind);
        if let Some(manifest_path) = &self.manifest_path {
            println!("manifest: {manifest_path}");
        }
        println!("result: {}", self.result);
        println!(
            "failure_phase: {}",
            self.failure_phase.as_deref().unwrap_or("none")
        );
        if let Some(next_step) = &self.next_step {
            println!("next_step: {next_step}");
        }
        if !self.next_action_kinds.is_empty() {
            println!("next_action_kinds: {}", self.next_action_kinds.join(","));
        }
        if let Some(total) = self.smoke_total {
            let failed = if self.failed_check_ids.is_empty() {
                "none".to_owned()
            } else {
                self.failed_check_ids.join(",")
            };
            let error_codes = if self.error_codes.is_empty() {
                "none".to_owned()
            } else {
                self.error_codes.join(",")
            };
            println!("smoke_checks: total={total}, failed={failed}, error_codes={error_codes}");
        }
        for artifact in &self.artifacts {
            println!(
                "artifact: kind={}, exists={}, path={}",
                artifact.kind,
                artifact
                    .exists
                    .map(|exists| exists.to_string())
                    .unwrap_or_else(|| "unknown".to_owned()),
                artifact.path
            );
        }
    }

    fn to_json(&self) -> String {
        format!(
            "{{\"schema\":\"axion.report-summary.v1\",\"path\":{},\"source_schema\":{},\"kind\":{},\"manifest_path\":{},\"result\":{},\"failure_phase\":{},\"next_step\":{},\"next_action_kinds\":{},\"smoke_checks\":{},\"artifacts\":{}}}",
            json_string_literal(&self.path),
            json_string_literal(&self.schema),
            json_string_literal(&self.kind),
            optional_json_string_literal(self.manifest_path.as_deref()),
            json_string_literal(&self.result),
            optional_json_string_literal(self.failure_phase.as_deref()),
            optional_json_string_literal(self.next_step.as_deref()),
            json_string_array_literal(&self.next_action_kinds),
            smoke_summary_json(self.smoke_total, &self.failed_check_ids, &self.error_codes),
            artifact_array_json(&self.artifacts),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SmokeSummary {
    total: usize,
    failed_ids: Vec<String>,
    error_codes: Vec<String>,
}

#[derive(Deserialize)]
struct SourceReport {
    schema: String,
    result: String,
    #[serde(default, alias = "manifestPath")]
    manifest_path: Option<String>,
    #[serde(default)]
    failure_phase: Option<String>,
    #[serde(default, alias = "nextStep")]
    next_step: Option<String>,
    #[serde(default)]
    next_actions: Vec<SourceAction>,
    #[serde(default)]
    artifacts: Vec<ReportArtifact>,
    #[serde(default)]
    smoke_checks: Option<Vec<SourceSmokeCheck>>,
    #[serde(default)]
    diagnostics: Option<SourceDiagnostics>,
}

#[derive(Default, Deserialize)]
struct SourceDiagnostics {
    #[serde(default)]
    failure_phase: Option<String>,
    #[serde(default)]
    next_step: Option<String>,
    #[serde(default)]
    smoke_checks: Option<Vec<SourceSmokeCheck>>,
}

#[derive(Deserialize)]
struct SourceAction {
    kind: String,
}

#[derive(Deserialize)]
struct SourceSmokeCheck {
    #[serde(default)]
    id: Option<String>,
    status: String,
    #[serde(default)]
    detail: Value,
}

fn supported_report_schemas() -> Vec<&'static str> {
    vec![
        "axion.check-report.v1",
        "axion.release-report.v1",
        "axion.bundle-report.v1",
        "axion.diagnostics-report.v1",
        "axion.dev-report.v1",
    ]
}

fn report_kind(schema: &str) -> Option<&'static str> {
    match schema {
        "axion.check-report.v1" => Some("check"),
        "axion.release-report.v1" => Some("release"),
        "axion.bundle-report.v1" => Some("bundle"),
        "axion.diagnostics-report.v1" => Some("diagnostics"),
        "axion.dev-report.v1" => Some("dev"),
        _ => None,
    }
}

fn write_summary_json(path: &Path, summary_json: &str) -> Result<(), AxionCliError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    std::fs::write(path, format!("{summary_json}\n"))?;
    Ok(())
}

fn smoke_check_summary(checks: &[SourceSmokeCheck]) -> SmokeSummary {
    let mut failed_ids = Vec::new();
    let mut error_codes = Vec::new();
    for (index, check) in checks.iter().enumerate() {
        if check.status == "fail" {
            failed_ids.push(
                check
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("smoke-check-{}", index + 1)),
            );
            for code in json_string_fields(&check.detail, "code") {
                if !error_codes.contains(&code) {
                    error_codes.push(code);
                }
            }
        }
    }
    SmokeSummary {
        total: checks.len(),
        failed_ids,
        error_codes,
    }
}

fn smoke_summary_json(
    total: Option<usize>,
    failed_check_ids: &[String],
    error_codes: &[String],
) -> String {
    match total {
        Some(total) => format!(
            "{{\"total\":{},\"failed_check_ids\":{},\"error_codes\":{}}}",
            total,
            json_string_array_literal(failed_check_ids),
            json_string_array_literal(error_codes),
        ),
        None => "null".to_owned(),
    }
}

fn artifact_array_json(values: &[ReportArtifact]) -> String {
    let values = values
        .iter()
        .map(|artifact| {
            format!(
                "{{\"kind\":{},\"path\":{},\"exists\":{}}}",
                json_string_literal(&artifact.kind),
                json_string_literal(&artifact.path),
                artifact
                    .exists
                    .map(|exists| exists.to_string())
                    .unwrap_or_else(|| "null".to_owned())
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{values}]")
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::cli::ReportArgs;

    use super::{ReportSummary, run};

    fn temp_report_path(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("{name}-{unique}.json"))
    }

    #[test]
    fn summarizes_check_report_with_typed_actions() {
        let report = r#"{"schema":"axion.check-report.v1","manifest_path":"app/axion.toml","failure_phase":null,"next_step":"run smoke","next_actions":[{"kind":"gui_smoke","required":false,"step":"run smoke"}],"result":"ok"}"#;
        let summary = ReportSummary::from_json(std::path::Path::new("check.json"), report)
            .expect("summary should parse");

        assert_eq!(summary.kind, "check");
        assert_eq!(summary.result, "ok");
        assert_eq!(summary.failure_phase, None);
        assert_eq!(summary.next_action_kinds, vec!["gui_smoke".to_owned()]);
    }

    #[test]
    fn summarizes_gui_smoke_report_checks() {
        let report = concat!(
            "{\"schema\":\"axion.diagnostics-report.v1\",\"result\":\"failed\",",
            "\"diagnostics\":{\"smoke_checks\":[",
            "{\"id\":\"fs.roundtrip\",\"status\":\"fail\",\"detail\":{\"error\":{\"code\":\"fs.not-found\"}}}",
            "]}}"
        );
        let summary = ReportSummary::from_json(std::path::Path::new("gui.json"), report)
            .expect("summary should parse");

        assert_eq!(summary.kind, "diagnostics");
        assert_eq!(summary.smoke_total, Some(1));
        assert_eq!(summary.failed_check_ids, vec!["fs.roundtrip".to_owned()]);
        assert_eq!(summary.error_codes, vec!["fs.not-found".to_owned()]);
    }

    #[test]
    fn summary_uses_top_level_result_when_nested_reports_exist() {
        let report = concat!(
            "{\"schema\":\"axion.diagnostics-report.v1\",",
            "\"diagnostics\":{\"source_report\":{\"result\":\"ok\"},\"failure_phase\":\"runtime\"},",
            "\"result\":\"failed\"}"
        );
        let summary = ReportSummary::from_json(std::path::Path::new("gui.json"), report)
            .expect("summary should parse");

        assert_eq!(summary.result, "failed");
        assert_eq!(summary.failure_phase.as_deref(), Some("runtime"));
    }

    #[test]
    fn top_level_string_fields_ignore_nested_values() {
        let report = concat!(
            "{\"schema\":\"axion.diagnostics-report.v1\",",
            "\"diagnostics\":{\"schema\":\"nested\",\"result\":\"ok\"},",
            "\"result\":\"failed\"}"
        );

        let summary =
            ReportSummary::from_json(std::path::Path::new("report.json"), report).unwrap();
        assert_eq!(summary.schema, "axion.diagnostics-report.v1");
        assert_eq!(summary.result, "failed");
    }

    #[test]
    fn summarizes_dev_report_with_camel_case_fields() {
        let report = concat!(
            "{\"schema\":\"axion.dev-report.v1\",",
            "\"manifestPath\":\"app/axion.toml\",",
            "\"nextStep\":\"use packaged fallback\",",
            "\"result\":\"ok\"}"
        );
        let summary = ReportSummary::from_json(std::path::Path::new("dev.json"), report)
            .expect("summary should parse");

        assert_eq!(summary.kind, "dev");
        assert_eq!(summary.manifest_path.as_deref(), Some("app/axion.toml"));
        assert_eq!(summary.next_step.as_deref(), Some("use packaged fallback"));
        assert_eq!(summary.result, "ok");
    }

    #[test]
    fn rejects_unsupported_report_schema() {
        let error = ReportSummary::from_json(
            std::path::Path::new("unknown.json"),
            r#"{"schema":"axion.unknown.v1","result":"ok"}"#,
        )
        .unwrap_err();

        assert!(error.to_string().contains("unsupported report schema"));
    }

    #[test]
    fn rejects_incomplete_json_report() {
        let error = ReportSummary::from_json(
            std::path::Path::new("broken.json"),
            r#"{"schema":"axion.check-report.v1","result":"ok""#,
        )
        .unwrap_err();

        assert!(error.to_string().contains("invalid report JSON"));
    }

    #[test]
    fn rejects_missing_result_field() {
        let error = ReportSummary::from_json(
            std::path::Path::new("missing-result.json"),
            r#"{"schema":"axion.check-report.v1"}"#,
        )
        .unwrap_err();

        assert!(error.to_string().contains("missing field `result`"));
    }

    #[test]
    fn failed_reports_require_allow_failed() {
        let path = temp_report_path("axion-report-failed");
        std::fs::write(
            &path,
            r#"{"schema":"axion.check-report.v1","result":"failed"}"#,
        )
        .unwrap();

        let error = run(ReportArgs {
            path: path.clone(),
            json: true,
            output: None,
            allow_failed: false,
        })
        .unwrap_err();

        assert!(error.to_string().contains("report result is failed"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn failed_reports_can_be_summarized_when_allowed() {
        let path = temp_report_path("axion-report-allowed");
        std::fs::write(
            &path,
            r#"{"schema":"axion.check-report.v1","result":"failed"}"#,
        )
        .unwrap();

        run(ReportArgs {
            path: path.clone(),
            json: true,
            output: None,
            allow_failed: true,
        })
        .expect("allow_failed should preserve summary success");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn report_output_writes_summary_json_before_failed_exit() {
        let source = temp_report_path("axion-report-output-source");
        let output = temp_report_path("axion-report-output-summary");
        std::fs::write(
            &source,
            r#"{"schema":"axion.check-report.v1","result":"failed"}"#,
        )
        .unwrap();

        let error = run(ReportArgs {
            path: source.clone(),
            json: false,
            output: Some(output.clone()),
            allow_failed: false,
        })
        .unwrap_err();
        let summary = std::fs::read_to_string(&output).expect("summary should be written");

        assert!(error.to_string().contains("report result is failed"));
        assert!(summary.contains("\"schema\":\"axion.report-summary.v1\""));
        assert!(summary.contains("\"result\":\"failed\""));

        let _ = std::fs::remove_file(source);
        let _ = std::fs::remove_file(output);
    }
    #[test]
    fn compact_and_pretty_reports_have_identical_semantics() {
        let compact = r#"{"schema":"axion.diagnostics-report.v1","manifest_path":"/tmp/\u4f60/\ud83d\ude80.toml","result":"failed","artifacts":[{"kind":"report","path":"/tmp/报告.json","exists":true}],"diagnostics":{"smoke_checks":[{"id":"required.bridge","status":"fail","detail":{"error":{"code":"bridge.denied"}}}]}}"#;
        let pretty = serde_json::to_string_pretty(
            &serde_json::from_str::<serde_json::Value>(compact).unwrap(),
        )
        .unwrap();
        let path = std::path::Path::new("report.json");
        let summary = ReportSummary::from_json(path, compact).unwrap();
        assert_eq!(summary, ReportSummary::from_json(path, &pretty).unwrap());
        assert_eq!(summary.manifest_path.as_deref(), Some("/tmp/你/🚀.toml"));
        assert_eq!(summary.failed_check_ids, ["required.bridge"]);
        assert_eq!(summary.artifacts.len(), 1);
    }

    #[test]
    fn invalid_types_syntax_and_duplicate_contract_fields_are_rejected() {
        for source in [
            r#"{"schema":"axion.check-report.v1" "result":"ok"}"#,
            r#"{"schema":"axion.check-report.v1","result":true}"#,
            r#"{"schema":"axion.check-report.v1","result":"unknown"}"#,
            r#"{"schema":"axion.check-report.v1","result":"failed","result":"ok"}"#,
            r#"{"schema":"axion.check-report.v1","result":"ok","artifacts":[{"kind":"x","path":3}]}"#,
        ] {
            assert!(ReportSummary::from_json(std::path::Path::new("report.json"), source).is_err());
        }
    }
}
