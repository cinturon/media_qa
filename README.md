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
cargo run -- watch ~/exports/incoming --profile youtube --scan-existing
cargo run -- checklist --profile youtube
```

## Commands

| Command | Purpose |
|---------|---------|
| `check <path>` | Scan, probe, validate, and analyze a file or folder |
| `probe <file>` | Run the QC pipeline on one file |
| `checklist --profile <name>` | Print a client-ready delivery checklist |
| `diff <before.json> <after.json>` | Compare two saved QC runs |
| `serve` | Start the studio API (auth, workspaces, uploads, jobs, billing) |
| `watch <folder>` | Watch a drop folder and QC new media when exports finish copying |

## Studio API (JIB-349+)

Start the multi-tenant studio API:

```bash
cargo run -- serve --port 8787
```

Demo credentials are seeded in `.mediaqa/studio/studio.json`:

- API token: `demo-token-change-me`
- Session token: `sess_operator-mediaqa-local_demo-studio`

Key endpoints (requires `Authorization: Bearer <session-token>` unless noted):

- `POST /auth/login` — exchange API token for session
- `GET /workspaces`, `POST /workspaces/switch`
- `POST /uploads`, `POST /jobs`, `GET /jobs/{id}`
- `GET /usage`, `GET /plans`, `POST /invites`
- `GET /admin/overview`, `GET /experiments/pricing`

## Desktop operator station (JIB-357+)

The Tauri desktop app lives under `desktop/`. See [desktop/PACKAGING.md](desktop/PACKAGING.md).

```bash
# Terminal 1: studio API
cargo run -- serve

# Terminal 2: desktop app (requires Tauri CLI)
cd desktop/src-tauri && cargo tauri dev
```

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
