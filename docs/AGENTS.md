# Building Lampion for Windows: rules for every contributor

What to build is in `docs/SPEC.md` (Hungarian, authoritative for Windows behavior). How the macOS app
behaves in detail is in the Swift reference checkout at `C:\Users\akit\lampion-macos-ref\Sources`
(read-only, never edit it). Where SPEC.md is silent, port the Swift behavior faithfully.
The UI language is **English**, like the macOS app.

## Stack

- Tauri 2, Rust (edition 2021) in `src-tauri/`, SvelteKit + Svelte 5 (runes) + TypeScript in `src/` (SPA, `ssr = false`).
- Icons: `@lucide/svelte`. Design tokens: `src/app.css` (use the CSS variables; light + dark both must work).
- Crates already in `Cargo.toml`: tokio, mysql_async, reqwest (rustls), sha2, hex, zip, regex, chrono,
  iana-time-zone, sysinfo, windows (Win32 features listed), winreg, rand, once_cell, anyhow, thiserror, dirs, semver.
  Add a crate only if really needed, and only in your own clearly separated line.

## Ownership (one owner per file; do not edit files you do not own)

| Part | Rust | Svelte |
| --- | --- | --- |
| core (lead) | `src-tauri/src/core/*`, `lib.rs` (except your own section of `generate_handler!`) | `src/app.css`, `src/lib/api.ts`, `src/lib/types.ts`, `src/lib/state.svelte.ts`, `src/lib/components/*`, `src/routes/+page.svelte`, `+layout.*` |
| servers | `src-tauri/src/servers/**`, `src-tauri/src/app/quit.rs` | `src/lib/pages/OverviewPage.svelte`, `src/lib/banners/SetupBanner.svelte`, `src/lib/servers.svelte.ts`, `src/lib/servers/**` |
| config | `src-tauri/src/config/**` | `src/lib/pages/ApacheSettings.svelte`, `PhpSettings.svelte`, `MariadbSettings.svelte`, `GeneralSettings.svelte`, `src/lib/config/**` |
| logs + app | `src-tauri/src/logs/**`, `src-tauri/src/app/**` except `quit.rs`, `src-tauri/icons/**`, `src-tauri/tauri.conf.json` bundle section, `.github/**`, `README.md`, `tools/**` | `src/lib/pages/LogsPage.svelte`, `src/lib/banners/UpdateBanner.svelte`, `src/lib/logs.svelte.ts`, `src/lib/logs/**`, `src/lib/updater/**` |
| database A (data, query) | `src-tauri/src/database/commands.rs`, `src-tauri/src/database/data/**` | `src/routes/database/**`, `src/lib/db/**` except the files of part B |
| database B (structure, designer) | `src-tauri/src/database/structure_commands.rs`, `src-tauri/src/database/structure/**` | `src/lib/db/StructureView.svelte`, `src/lib/db/DesignerView.svelte`, `src/lib/db/StructureSheets.svelte`, `src/lib/db/structure/**`, `src/lib/db/designer/**` |

Shared files you may only *append to* in your own marked place:
- `src-tauri/src/lib.rs`: add your commands under your section comment in `generate_handler!`. Nothing else.
- `src-tauri/src/core/settings.rs`: add a field with `#[serde(default)]` only if you need a new app setting.
- `src/lib/types.ts`: add your own exported types at the end under a `// ---- <part> ----` comment.
- `package.json`: add an npm dependency if you need one (e.g. CodeMirror for part A).

## Contracts between parts (keep these signatures)

- `crate::core::activity::{info, ok, warn, error}(module, text)` — the Lampion Activity log.
  Modules: `Apache`, `MySQL`, `PHP`, `Setup`, `Panel`, `Move`, `Config`, `Database`, `Update`.
- `crate::core::paths::*` — every path (Lampion root, exes, config files, `fwd()` for forward slashes).
- `crate::core::shell::{command, run, run_timeout, spawn_detached}` — programs never show a console window.
- `crate::core::settings::{get, update}` — app settings.
- `crate::core::emit(event, payload)` — events to all windows.
- `crate::config::locations::current() -> Locations` — ports and log/data paths read from the real configs
  (config part implements; servers and logs call it).
- `crate::config::access::{root_no_password_state, set_root_no_password}` and
  `crate::config::phpmyadmin::{auto_login_enabled, set_auto_login}` — config part implements;
  servers part calls them at first install.
- `crate::database::client::{query, exec, conn, configure, ident, qualified, literal}` — the one MariaDB
  connection; every statement is emitted as `database://sql` for the SQL console. Servers part calls
  `configure(port, None)` when MariaDB's port changes.
- `crate::app::quit::may_close` — servers part implements stop-on-quit and the move guard.
- Events: `servers://status` (payload `{ apache: ServerStatus, mariadb: ServerStatus }`, see `src/lib/types.ts`,
  sent only on real change), `activity://line`, `settings://changed`, `database://sql`.
  Name new events `<part>://<what>`.
- Main window: `app` in `src/lib/state.svelte.ts` (`app.servers`, `setUnsaved(pane, dirty)`, `app.logErrors`, `app.pane`).
- Database window: `db` in `src/lib/db/dbstate.svelte.ts` (selection, tab, `schemaVersion`, `sheet`).
  Part A mounts `<StructureView database table/>`, `<DesignerView database/>` and `<StructureSheets/>` from part B.

## Quality bar

- Port every behavior and every "Buktatók" (pitfalls) row of SPEC.md; each pitfall in your area gets a unit test.
- Slow work (processes, files, network, queries) never blocks the UI thread: async commands or
  `tauri::async_runtime::spawn_blocking`. Commands return `Result<T, String>` with a human-readable error.
- Send the UI only real changes (no unchanged state every tick).
- Respect reduced motion (the CSS tokens already do). Keyboard access and visible focus for every control.
- Windows specifics: CRLF in config files (keep the original line endings when writing), paths with spaces,
  `CREATE_NO_WINDOW` for every child process, never write to Program Files or OneDrive.
- No placeholder UI or TODO stubs left in your area when you finish.

## Build and check (this is the owner's everyday PC: keep the load low)

- **Never launch the app**: no `npm run tauri dev`, no `npm run tauri build`, no running `lampion.exe`,
  no `npm run dev`. The owner allows launching only after a full review and their explicit OK; the lead does that.
- **Never start servers or download server packages** (Apache/PHP/MariaDB) on this machine. Test logic with unit
  tests, temp dirs (`tempfile`) and fake files instead.
- Run Cargo from **PowerShell**, not Git Bash (Git Bash's `link.exe` shadows the MSVC linker):
  `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; $env:CARGO_TARGET_DIR = "C:\Users\akit\lampion-target"; $env:CARGO_BUILD_JOBS = "2"`
  then `cd src-tauri; cargo check` and `cargo test <your module>`. Always `CARGO_BUILD_JOBS=2`.
- The shared target dir saves disk and time; Cargo locks it, so a build may wait for another one. That is fine.
- `npm install` once in your checkout, then `npm run check` for Svelte/TS.
- Do not add crates whose build scripts the antivirus blocks (`rfd` / `tauri-plugin-dialog` is blocked: use
  `core.pickPath` / `crate::core::dialog` for file and folder pickers).
- Commit your work on your branch in small logical commits (English messages). Never push.
