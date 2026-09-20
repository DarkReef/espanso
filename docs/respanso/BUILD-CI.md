# rEspanso build and CI architecture

This document describes the release-oriented build pipeline used by rEspanso,
with emphasis on the Astra Linux 1.7 / X11 portable build.

## Release safety

A tested release snapshot must not be moved after it is declared pre-stable or
stable. Development and CI optimization continue on separate branches; release
branches are rollback points, not moving integration branches.

Current branch roles:

- `release/prestable-*` — immutable tested snapshots.
- `refactor/post-prestable-cleanup` — code cleanup after the latest pre-stable snapshot.
- `ci/astra-cache-acceleration` — CI/build optimization work.
- `build/x11-latest` — moving branch that triggers the Astra portable workflow.
- `build/windows-4f8e369` / `build/windows-latest` — Windows portable build branches.

## Why Astra is built inside Debian 10

The Astra portable package targets an older userspace. The Rust binaries are
therefore compiled inside Debian 10 (buster), whose glibc is 2.28. Packaging
then records every binary's required GLIBC symbol versions and rejects anything
newer than 2.28.

The host GitHub runner is intentionally not the ABI authority. It only
orchestrates the container, verifies the resulting immutable archive and uploads
the artifact.

## Reproducible toolchain

CI pins Rust with `RESPANSO_RUST_TOOLCHAIN` rather than following `stable`
implicitly. Updating Rust is an explicit maintenance change and must be followed
by a complete Astra and Windows build.

The Cargo lockfile remains the dependency authority. CI uses `--locked` for
checks, tests and release builds.

## Cache design

The Astra workflow uses two independent caches.

### Toolchain and Cargo registry

Cached paths:

- `.ci-cache/cargo`
- `.ci-cache/rustup`

The key includes the Rust version and `Cargo.lock`. This cache changes only
when the compiler or dependency graph changes.

The Debian container runs as root, but GitHub cache actions run as the normal
runner user. The workflow therefore normalizes ownership on container exit,
including failure paths. Without this step the cache appears to restore
successfully but cannot be saved because tar cannot read root-owned files.

### Compiled dependencies

Only reusable Cargo artifacts are cached:

- `target/{debug,release}/.fingerprint`
- `target/{debug,release}/build`
- `target/{debug,release}/deps`
- `target/.rustc_info.json`

The portable archive, runtime state and generated package directory are not
cached. A source-SHA suffix creates a fresh cache snapshot for each validated
revision, while restore prefixes allow the next revision to reuse the previous
compiled dependency graph.

`CARGO_INCREMENTAL=0` is deliberate in hosted CI. Incremental directories are
large, contain transient lock files and are poor cache material across ephemeral
runners. Cargo fingerprints plus compiled dependency artifacts give a smaller,
more predictable cache.

## Astra build phases

`scripts/build_pol_run_astra17.sh` emits machine-readable timing lines:

```
[rESP-CI-TIMING] phase=<name> seconds=<N>
```

Important phases include:

- `rust-check`
- `release-build`
- `x11-worker-smoke`
- `x11-restart-smoke`
- `x11-search-smoke`
- `x11-injector-stress`
- `workspace-tests`
- `tray-build`

Core and Match Studio are compiled in one release Cargo invocation so shared
release dependencies are scheduled once.

## Quality gates

A successful Astra artifact requires all of the following:

1. release-contract checks and shell syntax checks;
2. native X11 detector regression tests;
3. release build of core and Match Studio in the Debian 10 ABI container;
4. worker/restart/search X11 smoke tests under Xvfb;
5. AstraSafeInjector repeated expansion stress;
6. complete Rust workspace tests;
7. GLIBC symbol ceiling verification;
8. packaged-library resolution checks;
9. modern and legacy MCP protocol smoke tests;
10. immutable archive checksum verification;
11. artifact upload.

Do not remove a gate only to reduce build time. Prefer caching, parallelism or
eliminating duplicated work.

## Windows pipeline

Windows uses a pinned Rust toolchain and `Swatinem/rust-cache`. Core, portable
launcher and Match Studio are built in one release Cargo invocation. The final
portable archive is validated for required executables and receives a SHA-256
sidecar file before upload.

## Measuring optimization

Compare workflow wall time only between successful runs on comparable source
changes. A cache change needs at least two runs:

1. cold run — validates the new cache and saves it;
2. warm run — measures the actual reuse benefit.

A cache implementation is not considered working merely because the restore
step is green. The job log must show a successful save on the cold run and a
cache hit on the warm run.

## Maintenance rules

- Keep release branches immutable.
- Pin compiler/tool versions used for release artifacts.
- Keep cache ownership compatible with the host runner.
- Do not cache generated release archives as compiler inputs.
- Prefer tests for behaviour and only use source-string checks for explicit
  user-visible release contracts.
- Any change to ABI, injector behaviour, hotkeys or packaging requires the full
  Astra pipeline before promotion.
