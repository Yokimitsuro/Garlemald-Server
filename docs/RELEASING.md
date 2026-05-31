# Releasing

Garlemald Server uses [Semantic Versioning 2.0.0](https://semver.org/) driven by
git tags. The **git tag `vX.Y.Z` is the source of truth**; the root
`Cargo.toml` `[workspace.package].version` (inherited by every member crate via
`version.workspace = true`) and `Cargo.lock` are kept in lockstep automatically.

## How it works

`.github/workflows/release.yml` runs on every push/merge to `main` and:

1. reads the highest existing `vX.Y.Z` tag,
2. picks a bump level (see below),
3. rewrites `[workspace.package].version` in `Cargo.toml` and runs
   `cargo update --workspace` to sync `Cargo.lock`,
4. commits that back to `main` as `chore(release): vX.Y.Z [skip ci]`,
5. pushes an annotated `vX.Y.Z` tag, and
6. publishes a GitHub Release with auto-generated notes.

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

The bump commit message contains `[skip ci]`, which stops the PAT push from
re-triggering this workflow (PAT pushes *do* re-trigger workflows, unlike the
default token). The job also has an `if` guard that ignores commits whose message
starts with `chore(release):`, as a backstop.

## Seeding

The sequence starts from the `v0.1.0` tag on `main`. The first merge after the
release automation lands bumps it to `v0.1.1` (or the labeled minor/major).
