# Staccato Shell Web UI

The Svelte shell web UI renders panel, dock, sidebar, overview, quick settings, and date center surfaces. Rust owns the compositor-facing state, IPC, app launching, tray, notifications, and surface lifetime. This is the shell chrome implementation, not a fallback beside a native UI.

Install dependencies with Bun:

```sh
bun install
```

Run a dev server:

```sh
bun run dev
```

Build the embedded single-file bundle:

```sh
bun run build
```

The Rust shell loads `dist/index.html`, so rebuild this package before compiling `staccato-shell` after UI changes. From the workspace root, `cargo run -p staccatoctl -- dev apply` builds the web bundle, rebuilds `staccato-shell`, and asks a running Baton session to restart only the shell process. Use `cargo run -p staccatoctl -- dev watch` for the normal edit loop.
