use std::path::Path;

use axion_runtime::json_string_literal;

use crate::cli::ReportArgs;
use crate::commands::report_util::{
    json_array_section, json_bool_field, json_string_array_literal, json_string_field,
    json_string_fields, matching_json_delimiter, next_json_object, optional_json_string_field,
    optional_json_string_literal,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReportArtifact {
    kind: String,
    path: String,
    exists: Option<bool>,
}

impl ReportSummary {
    fn from_json(path: &Path, body: &str) -> Result<Self, AxionCliError> {
        validate_report_shape(body)?;
        let schema = json_string_field(body, "schema").ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "report is missing schema; expected one of: {}",
                    supported_report_schemas().join(", ")
                ),
            )
        })?;
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
        let manifest_path = json_string_field(body, "manifest_path")
            .or_else(|| json_string_field(body, "manifestPath"));
        let result = json_string_field(body, "result").ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "report is missing string result field",
            )
        })?;
        let failure_phase = optional_json_string_field(body, "failure_phase");
        let next_step =
            json_string_field(body, "next_step").or_else(|| json_string_field(body, "nextStep"));
        let next_action_kinds = json_array_section(body, "\"next_actions\"")
            .map(|section| json_string_fields(section, "kind"))
            .unwrap_or_default();
        let smoke_summary = smoke_check_summary(body);
        let artifacts = json_array_section(body, "\"artifacts\"")
            .map(report_artifacts)
            .unwrap_or_default();

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

fn validate_report_shape(body: &str) -> Result<(), AxionCliError> {
    let body = body.trim();
    if !body.starts_with('{') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "report must be a JSON object",
        )
        .into());
    }

    let object_end = matching_json_delimiter(body, 0, '{', '}').ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "report is not a complete JSON object",
        )
    })?;
    if object_end != body.len() - 1 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "report contains trailing data after the top-level JSON object",
        )
        .into());
    }

    Ok(())
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

fn report_artifacts(section: &str) -> Vec<ReportArtifact> {
    let mut artifacts = Vec::new();
    let mut cursor = 0;
    while let Some((object, next_cursor)) = next_json_object(section, cursor) {
        if let (Some(kind), Some(path)) = (
            json_string_field(object, "kind"),
            json_string_field(object, "path"),
        ) {
            artifacts.push(ReportArtifact {
                kind,
                path,
                exists: json_bool_field(object, "exists"),
            });
        }
        cursor = next_cursor;
    }
    artifacts
}

fn smoke_check_summary(source: &str) -> Option<SmokeSummary> {
    let checks = json_array_section(source, "\"smoke_checks\"")?;
    let mut total = 0usize;
    let mut failed_ids = Vec::new();
    let mut error_codes = Vec::new();
    let mut cursor = 0;
    while let Some((object, next_cursor)) = next_json_object(checks, cursor) {
        total += 1;
        if json_string_field(object, "status").as_deref() == Some("fail") {
            failed_ids.push(
                json_string_field(object, "id").unwrap_or_else(|| format!("smoke-check-{total}")),
            );
            for code in json_string_fields(object, "code") {
                if !error_codes.contains(&code) {
                    error_codes.push(code);
                }
            }
        }
        cursor = next_cursor;
    }
    Some(SmokeSummary {
        total,
        failed_ids,
        error_codes,
    })
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

        assert!(error.to_string().contains("not a complete JSON object"));
    }

    #[test]
    fn rejects_missing_result_field() {
        let error = ReportSummary::from_json(
            std::path::Path::new("missing-result.json"),
            r#"{"schema":"axion.check-report.v1"}"#,
        )
        .unwrap_err();

        assert!(error.to_string().contains("missing string result"));
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
}
