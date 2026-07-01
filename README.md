# mediaqa

CLI media linter and QC tool for post-production delivery workflows.

## Requirements

- Rust toolchain
- `ffprobe` and `ffmpeg` on your `PATH`

## Quick start

```bash
cargo build
cargo run -- check examples --profile youtube
cargo run -- check examples --profile youtube --json
cargo run -- check examples --profile youtube --html report.html --save
cargo run -- checklist --profile youtube
```

## Commands

| Command | Purpose |
|---------|---------|
| `check <path>` | Scan, probe, validate, and analyze a file or folder |
| `probe <file>` | Run the QC pipeline on one file |
| `checklist --profile <name>` | Print a client-ready delivery checklist |
| `diff <before.json> <after.json>` | Compare two saved QC runs |
| `serve` | Start a read-only dashboard API over saved reports |
| `watch <folder>` | Watch a drop folder and QC new files automatically |

## Useful flags

- `--profile youtube|shorts` — delivery profile from `src/sample.toml`
- `--json` — machine-readable batch report
- `--html <path>` — write a client-facing HTML report
- `--save` — persist the batch report under `.mediaqa/history`
- `--webhook <url>` — POST the JSON report to a webhook endpoint
- `--parallel` / `--no-parallel` — toggle parallel per-file processing

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Pass |
| 1 | Fail or runtime error |
| 2 | Warn only |

## Project layout

See [ARCHITECTURE.md](ARCHITECTURE.md) for the pipeline phases and module boundaries.

## Sample config

Delivery profiles live in `src/sample.toml`. Profiles can inherit from a `base` profile and override individual fields.

## Example workflow

```bash
# Batch QC with history + HTML for producers
cargo run -- check ./exports --profile youtube --save --html qc-report.html

# Compare yesterday vs today
cargo run -- diff .mediaqa/history/old.json .mediaqa/history/new.json

# Run the dashboard API for saved reports
cargo run -- serve --port 8787
```
