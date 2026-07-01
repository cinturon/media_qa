# Packaging Media QA Studio Desktop

Build the operator station (requires [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)):

```bash
cd desktop/src-tauri
cargo tauri build
```

Development run:

```bash
cargo tauri dev
```

Start the studio API in another terminal before using remote jobs or sign-in:

```bash
cargo run -- serve
```

Default demo credentials are seeded in `.mediaqa/studio/studio.json`:

- API token: `demo-token-change-me`
- Workspace: `demo-studio`

## Release artifacts

`cargo tauri build` produces platform bundles under `desktop/src-tauri/target/release/bundle/`.

For distribution signing, configure your platform keys in `tauri.conf.json` bundle settings and CI secrets.
