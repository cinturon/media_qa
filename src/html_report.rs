use crate::report::BatchReport;
use crate::suggestions::FixSuggestion;
use std::path::Path;

pub fn write_html_report(report: &BatchReport, path: &Path) -> Result<(), String> {
    let html = render_html(report);
    std::fs::write(path, html).map_err(|e| e.to_string())
}

fn render_html(report: &BatchReport) -> String {
    let mut rows = String::new();
    for file in &report.files {
        let findings = file
            .findings
            .iter()
            .map(|finding| {
                format!(
                    "<li class=\"{}\"><strong>{}</strong>: {}</li>",
                    format!("{:?}", finding.severity).to_lowercase(),
                    finding.code,
                    html_escape(&finding.message)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let suggestions = file
            .suggestions
            .iter()
            .map(|suggestion: &FixSuggestion| {
                format!(
                    "<li><strong>{}</strong>: {}</li>",
                    suggestion.code, html_escape(&suggestion.suggestion)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let error = file
            .error
            .as_ref()
            .map(|err| format!("<p class=\"error\">Error: {}</p>", html_escape(err)))
            .unwrap_or_default();

        rows.push_str(&format!(
            "<section class=\"file\"><h2>{path}</h2><p>Status: {status:?} | Review: {review:?}</p>{error}<h3>Findings</h3><ul>{findings}</ul><h3>Suggestions</h3><ul>{suggestions}</ul></section>",
            path = html_escape(&file.path.display().to_string()),
            status = file.status,
            review = file.review_state,
            error = error,
            findings = if findings.is_empty() {
                "<li>None</li>".into()
            } else {
                findings
            },
            suggestions = if suggestions.is_empty() {
                "<li>None</li>".into()
            } else {
                suggestions
            },
        ));
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Media QA Report - {profile}</title>
  <style>
    body {{ font-family: sans-serif; margin: 2rem; }}
    .summary, .file {{ border: 1px solid #ddd; padding: 1rem; margin-bottom: 1rem; }}
    .fail {{ color: #b00020; }}
    .warn {{ color: #9a6700; }}
    .error {{ color: #b00020; }}
  </style>
</head>
<body>
  <h1>Media QA Report</h1>
  <section class="summary">
    <p><strong>Profile:</strong> {profile}</p>
    <p><strong>Total:</strong> {total} |
       <strong>Passed:</strong> {passed} |
       <strong>Warned:</strong> {warned} |
       <strong>Failed:</strong> {failed} |
       <strong>Errors:</strong> {errors}</p>
    <p><strong>Overall:</strong> {overall:?}</p>
  </section>
  {rows}
</body>
</html>"#,
        profile = html_escape(&report.profile),
        total = report.summary.total,
        passed = report.summary.passed,
        warned = report.summary.warned,
        failed = report.summary.failed,
        errors = report.summary.errors,
        overall = report.overall_status(),
        rows = rows,
    )
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
