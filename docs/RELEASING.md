# Releasing

Garlemald Server uses [Semantic Versioning 2.0.0](https://semver.org/) driven by
git tags. The **git tag `vX.Y.Z` is the source of truth**; the root
`Cargo.toml` `[workspace.package].version` (inherited by every member crate via
`version.workspace = true`) and `Cargo.lock` are kept in lockstep automatically.

## Branching model

- **`develop`** is the default branch and the integration branch for day-to-day
  work. Branch features off `develop` and PR back into it. `develop` is protected
  and requires the CI checks (`fmt` / `clippy` / `build` / `test`) plus a pull
  request before merging.
- **`main`** is the protected release branch. A release is cut by opening a PR
  from `develop` into `main`; when it merges, the push to `main` triggers
  `release.yml` (version bump + tag), which in turn triggers `release-binaries.yml`
  (per-platform binaries). **Nothing merged into `develop` produces a release or a
  tag** — only the `develop` → `main` merge does.

Flow: feature → `develop` (CI-gated) → release PR `develop` → `main` (CI-gated) →
automatic bump + tag + GitHub Release with binaries.

> `main` intentionally does **not** require a pull request, so the release
> automation can push the bump commit straight to it via `RELEASE_PAT`.
>
> The version bump lands on `main` only (the tag is the source of truth), so
> `develop`'s `Cargo.toml` version may lag the latest tag between releases. This
> does **not** cause merge conflicts — only `release.yml` ever edits the version
> line, so each `develop` → `main` merge keeps `main`'s higher version
> automatically. If you'd rather keep `develop`'s version current, periodically
> merge `main` back into `develop` (a standard git-flow back-merge).

## How it works

`.github/workflows/release.yml` runs on every push/merge to `main` (i.e. when a
`develop` → `main` release PR merges) and:

1. reads the highest existing `vX.Y.Z` tag,
2. picks a bump level (see below),
3. rewrites `[workspace.package].version` in `Cargo.toml` and runs
   `cargo update --workspace` to sync `Cargo.lock`,
4. commits that back to `main` as `chore(release): vX.Y.Z`,
5. pushes an annotated `vX.Y.Z` tag, and
6. publishes a GitHub Release with auto-generated notes.

That tag push then triggers **`release-binaries.yml`** (issue #15), which
cross-compiles the four server binaries per platform and attaches the archives
+ SHA-256 checksums to the same Release — see [Binary releases](#binary-releases).

After a release, `git describe --tags` on `main` and `[workspace.package].version`
report the same `X.Y.Z`, and `cargo build` stamps every server binary with it.

## Choosing the bump level

| Bump      | How to trigger                                                                 |
|-----------|--------------------------------------------------------------------------------|
| **Patch** | Default. Any merge to `main` with no release label → `Z` increments.            |
| **Minor** | Add the **`release:minor`** label to the PR before merging → `Y+1`, `Z=0`.      |
| **Major** | Add the **`release:major`** label to the PR before merging → `X+1`, `Y=0`, `Z=0`. |

Guidance: bump **minor** for a new subsystem/feature, **major** for a breaking
wire/protocol or DB-schema change. The label is read from the PR associated with
the merge commit; if both labels are present, `release:major` wins.

### Manual minor/major (alternative)

Because the next version is computed from the **highest tag**, you can also bump
out-of-band by pushing a tag yourself:

```sh
git tag -a v0.2.0 -m v0.2.0 && git push origin v0.2.0
```

The automation then continues patch-incrementing from there (`v0.2.1`, …). This
is handy for the first minor/major when no PR is involved.

## One-time setup

The workflow needs to push the version-bump commit to **`main`, which is
branch-protected** (requires the `fmt`/`clippy`/`build`/`test` checks). The
default `GITHUB_TOKEN` cannot push to a protected branch, so the workflow uses a
Personal Access Token:

1. Create a **fine-grained PAT** owned by a repo **admin**, scoped to **only**
   `Garlemald-Server`, with **Repository permissions → Contents: Read and write**
   (and *Pull requests: Read* if you later want richer notes).
2. Save it as the repository secret **`RELEASE_PAT`**
   (`Settings → Secrets and variables → Actions`).
3. Keep branch protection's **"Do not allow administrators to bypass" OFF**
   (`enforce_admins: false`). The admin-owned PAT relies on that exemption to push
   the bump commit past the required checks. If you turn admin enforcement on, the
   bump push will be rejected.

The `release:minor` and `release:major` labels must exist in the repo (created as
part of issue #13).

> **Security note.** Because `RELEASE_PAT` is admin-owned and `enforce_admins` is
> off, a leak of this token lets the holder push to `main` *bypassing the required
> checks* — a strictly larger capability than vanilla Contents: write. Give the PAT
> a short expiry, rotate it on a schedule, and treat a suspected leak as urgent.
> The release step also needs `crates.io` reachable (it runs `cargo update
> --workspace` to resync `Cargo.lock`); a registry/network blip will fail the run,
> which is safe to re-run.

## Loop prevention

A PAT push re-triggers workflows (unlike the default token), so the bump commit's
push to `main` would re-run this workflow. That loop is broken by the job's `if`
guard, which skips commits authored by `github-actions[bot]` (the release bot).

The bump commit is **deliberately not** marked `[skip ci]`. `[skip ci]` would
suppress not just this workflow but also the **tag-push** event — and the binary
release (`release-binaries.yml`) triggers on that tag push, so a skip marker would
silently prevent the per-platform binaries from ever building.

## Binary releases

When the SemVer step pushes a `vX.Y.Z` tag, **`.github/workflows/release-binaries.yml`**
fires (`on: push: tags: ['v*.*.*']`, plus a manual `workflow_dispatch`). It:

1. builds the four server binaries (`lobby-server`, `map-server`, `web-server`,
   `world-server`) in `--release --locked` across a platform matrix
   (`x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, `x86_64-apple-darwin`,
   `x86_64-pc-windows-msvc`),
2. packages each platform into `garlemald-server-vX.Y.Z-<target>.tar.gz` (`.zip`
   on Windows) with a matching `.sha256`,
3. and attaches all archives + checksums to the existing `vX.Y.Z` Release
   (idempotently, so re-runs replace assets rather than duplicating them).

This only works because the SemVer step pushes the tag with `RELEASE_PAT` (a PAT
push triggers downstream workflows) and the tagged commit is **not** `[skip ci]`.
To (re)build assets for an existing tag, run the workflow manually via
`workflow_dispatch` with the tag name.

## Seeding

The sequence starts from the `v0.1.0` tag on `main`. The first merge after the
release automation lands bumps it to `v0.1.1` (or the labeled minor/major).
