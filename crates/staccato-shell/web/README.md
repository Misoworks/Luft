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

The Rust shell embeds `dist/index.html`, so rebuild this package before compiling `staccato-shell` after UI changes.
