# Development of nas-backup-manager

The Vue/TypeScript UI is in `src/`; the Rust/Axum backend is in `backend/axum/`. The shared scripts are in the library `@valeriogc/nas-base-app`; manifest and version are in `nas-app.json`, `package.json` and the Cargo metadata. `src/assets/logo.svg` is the source of the logo and the UI icons are in `src/assets/icons/`.

## Conventions

Use the Composition API with `<script setup lang="ts">`, imports through the `@/` alias, scoped SFC styles, CSS names with `_` and UI texts in the catalogues `src/i18n/locales/en.ts` and `it.ts`. Code, method names, test descriptions, this document and the README are in English. The NAS theme comes from the context of the iframe; palette, SCSS partials and shared components come from `@valeriogc/nas-base-app`. The bridge is the one of the library: `src/services/nasShell.ts` adapts its answers to the screens and `src/composables/useNasLauncherContext.ts` binds it to the catalogue of the app. Sidebar, confirmations and the empty state of the list are `NasAppLayout`, `NasConfirmDialog` and `NasEmptyState`.

The FE tests are in `src/tests/unit`, `component`, `scripts`, `e2e`, with sub-paths mirroring `src/`. The Rust tests are in `backend/tests/unit`, with a shared support (`support.rs`: in-memory database, temporary folders) and three fake 7z programs in `fixtures/`, fixed files because a script written and started at once, while other tests fork, gives `Text file busy`. The E2E tests also start the real backend on temporary disks and database (`playwright.config.ts`) and run a backup from the list.

## How it runs the rules

In the backend, `rules.rs` holds model, checks and routes; `runner.rs` runs one execution at a time per rule (at most two together), shows it among the operations of the launcher and writes its outcome (`last_run_*`); `scheduler.rs` computes the next run with `systemd-analyze calendar` and starts the scheduled ones; `watcher.rs` watches the sources of the real-time rules with `notify`; `sync.rs` copies, renames and mirrors one folder at a time, keeping in `rule_files` the files it copied; `archive.rs` prepares a folder of links in the state of the app (no copy) and starts 7z with the password on standard input, never on the command line; `storage.rs` resolves the logical paths on the disk of the launcher registry and refuses a disk that is not mounted; `filters.rs` and `rename.rs` hold the rules on files; `launcher.rs` talks to the socket of the launcher (`NAS_LAUNCHER_SOCKET`) for the background flag and the operations. The work on files runs on blocking threads, never on the two workers of the server. In development, without a registry, the paths are relative to `NAS_STORAGE_ROOT` (or `backend/data/storage`); `NAS_BACKUP_7Z` replaces the 7z program.

## Dependencies and lockfiles

Locally dependencies are installed with pnpm; the pipelines install with `npm ci` from `package-lock.json`, which therefore has to match `package.json`. `pnpm install` realigns it by itself (`postinstall`, which installs nothing with npm); `pnpm add`, `pnpm update` and `pnpm remove` do not: after them run `pnpm run lock:npm`. `pnpm run lint` checks the alignment and, when it is missing, fails saying so; `release:merge` realigns it anyway before publishing. `pnpm-lock.yaml` stays local.

## Shared library

The launcher bridge, styles, shared components and helpers, and the build, package and release scripts come from `@valeriogc/nas-base-app`, published on GitHub Packages; the commands of `package.json` use its `nas-base-app`. The `.npmrc` of the project points the `@valeriogc` scope to its registry; locally a classic token with `read:packages` is needed in your own `~/.npmrc` (`//npm.pkg.github.com/:_authToken=...`), while the pipelines use the token of the workflow. For a new version of the library, update the number in `package.json` and run `pnpm install`. The local `pnpm-workspace.yaml` has `'@valeriogc/nas-base-app': false` under `allowBuilds`.

## Checks

```bash
pnpm install
pnpm run format:check
pnpm run lint
pnpm run type-check
pnpm test
pnpm run test:e2e
pnpm run test:coverage
cargo fmt --manifest-path backend/Cargo.toml --all --check
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path backend/Cargo.toml
pnpm run nas:package
pnpm run nas:test
```

`pnpm run format` applies Prettier, ESLint and `format-sfc-spacing.mjs`; `pnpm run format:sfc` runs only the last step. The E2E tests start their own Vite on port 14217 and Axum on `nas-backup-manager-e2e.sock`, with temporary storage and database; they never reuse a development server. `release:merge` creates commit and tag: try it only in an isolated temporary Git repository.

The package must work even when moved out of the checkout; do not reference `external-apps/` or files of the launcher. For the full contract see [APPS.md](APPS.md).
