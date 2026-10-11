# TablePro

**A fast, native, open-source database client** — inspired by TablePlus & HeidiSQL, built with Tauri 2 + SvelteKit 5 + Rust. Free forever, no subscription.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2.x-24C8D8?logo=tauri&logoColor=white)](https://tauri.app)
[![Svelte](https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte&logoColor=white)](https://svelte.dev)
[![Rust](https://img.shields.io/badge/Rust-sqlx-orange?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey)](https://github.com/bapenda-kalsel/table-pro)

> <!-- Add a screenshot: docs/screenshot.png -->
> Browse, edit, and manage **SQLite**, **MySQL/MariaDB**, and **PostgreSQL** databases through one shared `DatabaseDriver` trait — every feature works on all three engines where the underlying database supports it.

---

## Features

### Data Grid
- Browse tables & views with pagination, raw `WHERE` filter, and click-to-sort columns
- **Staged changes workflow** — inline edits, row insert/delete, and bulk-edit are buffered locally (green/amber/red highlighting), reviewed as generated SQL in a **Code Review diff modal**, then committed in one atomic transaction
- Row detail drawer (select a row, press `Space`)
- Export to **CSV / Excel (.xlsx) / JSON / SQL dump** — respects the active `WHERE` filter

### SQL Editor
- CodeMirror 6 with **real schema autocomplete** (live table/column names)
- `⌘⏎` run query — executes the **selected text** if a selection exists
- **Explain Plan** — renders the plan tree and auto-flags full table scans, missing indexes, and temp B-tree sorts
- **Production write guard** — write/DDL statements on `production`-tagged connections require confirmation
- Export result as CSV

### Schema Inspector
- Columns, Indexes (unique/partial/origin), Foreign Keys, Triggers, and DDL — all in tabbed views

### Workbench-style Tools
- **ER Diagram** (`⌘⇧E`) — reverse-engineers every table + foreign key into a draggable visual map, with zoom and auto-layout
- **Visual Relations** — phpMyAdmin Designer style: toggle *New Relation*, click a column then its referenced column, pick `ON DELETE`/`ON UPDATE` actions → real `FOREIGN KEY` created (SQLite does a safe transactional table rebuild with a `foreign_key_check` gate; MySQL/PostgreSQL use `ALTER TABLE … ADD CONSTRAINT`)
- **Server Dashboard** (`⌘⇧D`) — file size, page/freelist stats, journal mode, encoding, object counts, rows-per-table chart, fragmentation warnings
- **Saved Snippets** — save/reuse queries from the editor toolbar
- **Visual Table Designer** — create tables via a column editor (PK/NN/UQ/AI/DEFAULT) with live DDL preview
- **Copy row as INSERT / JSON** from the row detail drawer

### Power Tools
- **Find Text in Database** (`⌘⇧F`) — HeidiSQL-style scan across all text columns
- **Maintenance** — VACUUM, `PRAGMA integrity_check`, ANALYZE, REINDEX, **Backup** (`VACUUM INTO`)
- **Import CSV** — header row becomes columns, table auto-created, wrapped in a transaction
- **Query Audit Trail** (`⌘⇧A`) — every executed query persisted to `~/.tablepro/audit.jsonl`; stats, filtering, CSV/JSON export
- Quick Switcher (`⌘P`), regex table filter in sidebar, real-time SQL console (`⌘J`)

### Security & Privacy
- Passwords live in the **OS Keychain** (`com.tablepro.vault`) — never in config files, never committed to disk in plaintext
- Connection profiles in `~/.tablepro/connections.json` with environment tags (`local` / `staging` / `production`)
- 100% local — no telemetry, no external servers, your data never leaves your machine

### Internationalization
- UI available in **English** and **Bahasa Indonesia** — toggle EN/ID in the title bar
- Defaults to your OS language; choice persists across restarts

## Supported Databases

| Feature | SQLite | MySQL / MariaDB | PostgreSQL |
|---|---|---|---|
| Browse / edit / commit | ✓ | ✓ | ✓ (public schema) |
| Explain plan | `EXPLAIN QUERY PLAN` | `EXPLAIN` | `EXPLAIN` (text) |
| Export CSV / JSON / SQL / XLSX | ✓ | ✓ | ✓ |
| CSV import | ✓ | ✓ | ✓ |
| Backup | `VACUUM INTO` + SQL dump fallback | SQL dump | SQL dump |
| Maintenance | VACUUM / ANALYZE / REINDEX / integrity | OPTIMIZE / CHECK / ANALYZE | VACUUM / ANALYZE / REINDEX |

## Tech Stack

| Layer | Tech |
|---|---|
| Shell | [Tauri 2](https://tauri.app) |
| Frontend | SvelteKit 2 (SPA mode) · Svelte 5 runes · Tailwind CSS 4 |
| Editor | CodeMirror 6 |
| Backend | Rust · [sqlx](https://github.com/launchbadge/sqlx) · [keyring](https://github.com/hwchen/keyring-rs) |

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs) (stable)
- [Node.js](https://nodejs.org) 18+ and [pnpm](https://pnpm.io)
- Platform-specific requirements below

### Run from source

```bash
git clone https://github.com/bapenda-kalsel/table-pro.git
cd table-pro
pnpm install

# Frontend only (mock data mode, http://localhost:1420)
pnpm dev

# Full native app
pnpm tauri dev
```

## Building Installers

Tauri compiles native binaries, so **build on each target OS** — or use the included [GitHub Actions workflow](.github/workflows/release.yml), which builds all three platforms and drafts a release whenever you push a `v*` tag.

```bash
pnpm tauri build   # output → src-tauri/target/release/bundle/
```

### macOS

**Prerequisite:** Xcode Command Line Tools

```bash
xcode-select --install
pnpm tauri build
```

Output: `bundle/macos/TablePro.app` and `bundle/dmg/TablePro_*.dmg`

> Unsigned builds trigger Gatekeeper — users can right-click → **Open** to bypass. For public distribution, sign with an Apple Developer ID certificate (`APPLE_CERTIFICATE`, `APPLE_SIGNING_IDENTITY` secrets in CI).

### Windows

**Prerequisites:**
- [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) (Desktop development with C++)
- WebView2 Runtime — preinstalled on Windows 11 and most Windows 10 systems

```powershell
pnpm tauri build
```

Output: `bundle/nsis/TablePro_*-setup.exe` and `bundle/msi/TablePro_*_en-US.msi`

### Linux (Debian/Ubuntu)

**Prerequisites:**

```bash
sudo apt-get update
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
pnpm tauri build
```

Output: `bundle/deb/*.deb` and `bundle/appimage/*.AppImage`

For other distros see the [Tauri Linux prerequisites](https://v2.tauri.app/start/prerequisites/#linux).

### Releasing via CI (recommended)

The workflow at `.github/workflows/release.yml` builds macOS, Linux, and Windows installers in parallel and attaches them to a draft GitHub Release:

```bash
git tag v0.1.0
git push origin v0.1.0
```

## Development

```bash
pnpm check        # typecheck (svelte-check)
pnpm test         # frontend unit tests (vitest)
cargo test        # backend unit tests (run inside src-tauri/)
```

### Live integration tests (MySQL/PostgreSQL)

```bash
cd src-tauri
TABLEPRO_TEST_MYSQL="mysql://root:secret@127.0.0.1:3306/testdb" \
TABLEPRO_TEST_POSTGRES="postgres://postgres:secret@127.0.0.1:5432/postgres" \
cargo test
# Without the env vars the live smoke tests skip themselves.
```

## Project Structure

```
src/                    # SvelteKit frontend
  lib/api/              # Tauri invoke wrappers (with browser mock mode)
  lib/components/       # UI: panels/ · modals/ · chrome/ (window frame)
  lib/stores/           # Svelte 5 rune stores (workspace state)
  lib/i18n/             # EN/ID dictionaries (en.ts is source of truth)
  lib/utils/            # SQL builder helpers (+ colocated unit tests)
  lib/types.ts          # Shared TS types mirroring src-tauri models
src-tauri/
  src/drivers/          # DatabaseDriver trait + sqlite/ · mysql/ · postgres/ · common/
  src/commands/         # Tauri commands exposed to the frontend
  src/vault/            # connections.json, snippets, audit log, keychain passwords
  src/models/           # Shared serde types
  # every backend module keeps unit tests in a sibling tests.rs
```

## Keyboard Shortcuts

| Key | Action |
|---|---|
| `⌘T` | New query tab |
| `⌘W` | Close tab |
| `⌘P` | Quick switcher |
| `⌘J` | Toggle SQL console |
| `⌘⇧F` | Find text in database |
| `⌘⇧A` | Query audit trail |
| `⌘⇧E` | ER diagram |
| `⌘⇧D` | Server dashboard |
| `⌘⏎` | Run query |
| `⌘S` | Commit staged changes |

## Contributing

Contributions are welcome! Feel free to open an issue for bugs/feature requests, or submit a pull request.

### Branching

| Branch | Purpose |
|---|---|
| `main` | Stable, release-ready code. Only merge from `develop` when cutting a release. |
| `develop` | Active development — base all feature branches here. |
| `feature/*`, `fix/*` | Short-lived branches, merged back into `develop` via PR. |

```bash
git checkout develop
git checkout -b feature/my-feature
# ... work, then open a PR targeting develop
```

Please make sure `pnpm check`, `pnpm test`, and `cargo test` pass before submitting.

## License

[MIT](LICENSE) © Badan Pendapatan Daerah Provinsi Kalimantan Selatan (Bapenda Kalsel)

## Acknowledgments

Inspired by [TablePlus](https://tableplus.com) and [HeidiSQL](https://www.heidisql.com).
