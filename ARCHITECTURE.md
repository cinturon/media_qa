# Media QA Studio Architecture

`mediaqa` is a synchronous CLI pipeline for post-production media QC.

## Pipeline phases

```text
input path → scan → probe → validate → analyze → report
```

| Phase | Module | Responsibility |
|-------|--------|----------------|
| CLI | `main.rs` | Parse args, dispatch commands, set exit codes |
| Config | `config.rs` | Load TOML delivery profiles |
| Scan | `scanner.rs` | Discover candidate media paths |
| Probe | `probe.rs` | Run `ffprobe`, parse JSON at the boundary |
| Domain | `metadata.rs` | Normalized `MediaMetadata` |
| Validate | `validate.rs` | Pure rules: metadata + profile → findings |
| Analyze | `analyze.rs` | Richer checks (`ffmpeg` decode, black frames) |
| Orchestrate | `pipeline.rs` | Wire phases per file, tolerate per-file errors |
| Report | `report.rs` | `FileReport`, `BatchReport`, human + JSON output |

## Data boundaries

- **Raw ffprobe structs** stay inside `probe.rs` — not passed through the app.
- **Findings** are structured data (`code`, `severity`, `message`) — rules never print directly.
- **Reporting** is separate from checking — `report.rs` renders `BatchReport`.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | All files passed |
| 1 | Any failure or probe/decode error |
| 2 | Warnings only (e.g. black frames) |

## External tools

- `ffprobe` — metadata probe (required)
- `ffmpeg` — decode integrity and black-frame analysis (required for full QC)
