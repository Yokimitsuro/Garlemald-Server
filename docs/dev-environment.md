# Developer environment

How to build Garlemald Server, run the four binaries, turn on logging and packet
capture, and reset the server's save state. By the end you'll have the stack
running on localhost and know how to watch what it's doing.

For *what* the binaries are, read [`architecture.md`](architecture.md) first.

---

## Prerequisites

- **Rust 1.95.0**, pinned in `rust-toolchain.toml` (channel `1.95.0`, with
  `rustfmt` + `clippy`). If you use `rustup`, the correct toolchain is installed
  automatically the first time you build in this repo — you don't pin it
  yourself. The workspace is **edition 2024**.
- A **C toolchain**, because two dependencies build native C on first compile:
  - `tokio-rusqlite` bundles SQLite (`bundled` feature) — no system SQLite
    needed.
  - `mlua` vendors Lua 5.4 (`vendored` feature) — no system Lua needed.

  | Platform | What to install                                                        |
  |----------|------------------------------------------------------------------------|
  | macOS    | Xcode Command Line Tools (`xcode-select --install`)                    |
  | Linux    | `build-essential` (gcc/clang + make) and `pkg-config`                  |
  | Windows  | The MSVC build tools (the default `*-pc-windows-msvc` Rust toolchain)  |

- No external database or web server is required — SQLite is embedded and the
  HTTP frontend is the `web-server` binary itself.

---

## Build and run

The repo ships launcher scripts under `scripts/` (a `.sh` and a matching `.cmd`
for every one). The easiest path is to build + run everything at once:

```sh
./scripts/run-all.sh        # build all crates, then start web, lobby, world, map
```

`run-all.sh` builds the workspace, starts the four binaries, writes per-server
logs to `./logs/{web,lobby,world,map}.log`, and forwards Ctrl-C as a clean
shutdown. On Windows use `scripts\run-all.cmd`; `scripts\stop-all.cmd` stops a
detached stack.

Run a single service (handy when you're iterating on one crate):

```sh
./scripts/run-web.sh        # HTTP signup/login    :54993
./scripts/run-lobby.sh      # character list       :54994
./scripts/run-world.sh      # session/handoff      :54992
./scripts/run-map.sh        # zone / actor / Lua   :1989
```

Other helpers: `scripts/build.sh` (build only), `scripts/test.sh` (run the test
suite), `scripts/check-env.sh` (verify your toolchain).

### `PROFILE` — release vs debug builds

The scripts default to **`--release`**. Export `PROFILE=debug` for faster,
unoptimized builds while iterating:

```sh
PROFILE=debug ./scripts/run-all.sh
```

`PROFILE=release` (the default) builds with `--release` and runs from
`target/release`; `PROFILE=debug` drops `--release` and runs from `target/debug`.

### Running a binary by hand

Each binary takes `--config <path>`:

```sh
cargo build --release --workspace
./target/release/web-server   --config ./configs/web.toml
./target/release/lobby-server --config ./configs/lobby.toml
./target/release/world-server --config ./configs/world.toml
./target/release/map-server   --config ./configs/map.toml
```

### Ports and configs

All four bind to `127.0.0.1` by default and share one SQLite file. Edit the TOML
to change ports/binds; the comments in each file document its keys.

| Service        | Config                | Default bind        | Config notes                                    |
|----------------|-----------------------|---------------------|-------------------------------------------------|
| `web-server`   | `configs/web.toml`    | `127.0.0.1:54993`   | `[session].hours` = token lifetime (default 24) |
| `lobby-server` | `configs/lobby.toml`  | `127.0.0.1:54994`   | `[database].path` shared with the others        |
| `world-server` | `configs/world.toml`  | `127.0.0.1:54992`   | `[server].world_id` keys the `servers` row      |
| `map-server`   | `configs/map.toml`    | `127.0.0.1:1989`    | `[scripting].script_root`, `load_from_database` |

### The database

All four binaries share one SQLite file — `./data/garlemald.db` by default
(`[database].path`). On first run `common`'s DB layer (`common/src/db.rs::open_or_create`)
creates the file, enables WAL + foreign keys, and applies the bundled schema +
seeds once (tracked in a `schema_migrations` table, so concurrent boots don't
double-apply). You normally never touch it directly — to reset it, see
[Resetting save state](#resetting-save-state).

---

## Logging

Logging is `tracing` + `tracing-subscriber`'s `EnvFilter`, configured in
`common/src/logging.rs`.

- **Default (no `RUST_LOG` set):** third-party crates log at `info`, and the
  Garlemald crates (`common`, `lobby_server`, `world_server`, `map_server`) log
  at `debug` — verbose coverage of our own code without the noise of every
  dependency. Output has no ANSI colour (it's log-file friendly).
- **Override with `RUST_LOG`** (standard `EnvFilter` syntax):

  ```sh
  RUST_LOG=trace ./scripts/run-map.sh            # everything, every crate
  RUST_LOG=map_server=debug ./scripts/run-map.sh # just map-server at debug
  RUST_LOG=info ./scripts/run-all.sh             # quiet: info across the board
  RUST_LOG=warn,map_server=trace ./scripts/run-map.sh  # mix per-crate levels
  ```

> Tip: when debugging content, `RUST_LOG=map_server=debug` surfaces the Lua
> hook/command path (see [`lua-runtime.md`](lua-runtime.md)).

---

## Packet capture

For wire-protocol work, `common/src/packet_log.rs` can dump every inbound and
outbound packet as a hex dump. It is **off unless an env var is set**, so it
costs nothing in normal runs.

- **`GARLEMALD_PACKET_LOG_DIR=<dir>`** — enables capture. Each service appends to
  its own `<tag>-packets.log` in that directory (`lobby-packets.log`,
  `world-packets.log`, `map-packets.log`, `web-packets.log`), each entry tagged
  `[IN ]` / `[OUT]` with a timestamp, the peer address, a byte count, and an
  xxd-style hex dump. The fixed-width direction tags keep columns aligned for
  `grep`/`diff` against reference captures.
- **`GARLEMALD_PACKET_LOG_PLAINTEXT=1`** — also logs the **pre-encryption**
  bytes, so you can diff payloads without decrypting. (Presence of the variable
  is the toggle.)

```sh
GARLEMALD_PACKET_LOG_DIR=./logs/packets \
GARLEMALD_PACKET_LOG_PLAINTEXT=1 \
  ./scripts/run-all.sh
```

---

## Booting into a known game state

Getting from a cold server to "standing in a starting city, logged in" by hand is
tedious. The **`fresh-start-*` orchestrator scripts** automate it end to end.

> **These scripts live in the sibling [`ffxiv-actor-cli`](https://github.com/swstegall/Garlemald-Server)
> repo, not in this one** — under `ffxiv-actor-cli/scripts/`. They drive the
> whole stack (server + Garlemald Client + the Wine game) via the macOS UI
> automation in that repo. Run them from the `ffxiv-actor-cli` checkout.

| Script                      | End state                                                        |
|-----------------------------|-----------------------------------------------------------------|
| `fresh_start.sh`            | A brand-new character created from scratch (end of char creation) |
| `fresh-start-limsa.sh`      | A pre-built character at the start of **Limsa Lominsa**          |
| `fresh-start-gridania.sh`   | A pre-built character at the start of **Gridania**               |
| `fresh-start-uldah.sh`      | A pre-built character at the start of **Ul'dah**                 |

Each one: archives the current `data/` to a timestamped zip, restores the chosen
seed state (or wipes for a from-scratch run), kills any running stack, rebuilds +
starts the four servers, launches the client, mints a session via the web
`/signup` or `/login` endpoint, drives the launcher's dev-session field, and runs
the automation phases to reach the target state.

Common env knobs (see the script headers for the full set):

| Variable                          | Effect                                                       |
|-----------------------------------|-------------------------------------------------------------|
| `PROFILE=debug`                   | Build/run debug instead of release                          |
| `SKIP_CLIENT=1`                   | Bring the servers up but don't launch the game client       |
| `SKIP_AUTOMATION=1`               | Boot servers + client but don't drive signup/login          |
| `GARLEMALD_PACKET_LOG_DIR=<dir>`  | Route packet logs into a capture directory                  |
| `GARLEMALD_PACKET_LOG_PLAINTEXT=1`| Also capture pre-encryption bytes                           |

```sh
cd ../ffxiv-actor-cli
./scripts/fresh-start-limsa.sh
```

---

## Resetting save state

The server's entire live state is the SQLite **`data/`** directory
(`garlemald.db` plus its `-wal`/`-shm` sidecars). "Saving" and "restoring" the
server is just zipping and unzipping that directory.

`data-backups/` holds two kinds of zip:

- **Timestamped snapshots** — `data-YYYYMMDD-HHMMSS.zip`, written automatically by
  the fresh-start scripts before they wipe/restore. These are your undo history.
- **Canonical seed states** — `data-limsaStart.zip`, `data-gridaniaStart.zip`,
  `data-ulDahStart.zip`: a known account + character placed at the start of each
  city. The fresh-start scripts restore from these. A byte-identical copy lives
  at the workspace root in `data-backups-backup/` in case `data-backups/` is ever
  blown away.

### Checkpoint the current state

From the repo root, archive `data/` into a new snapshot (the `-X` drops extra
file attributes so the archive is reproducible and free of macOS resource forks):

```sh
zip -rq -X data-backups/data-$(date +%Y%m%d-%H%M%S).zip data
# or name it: zip -rq -X data-backups/data-mything.zip data
```

### Restore a snapshot or seed

Delete the live state and unzip the archive you want (each zip contains a
top-level `data/` directory, so unzip from the repo root):

```sh
rm -rf data
unzip -q -o data-backups/data-limsaStart.zip -x '__MACOSX/*'
```

The next server boot picks up the restored database. (The `fresh-start-*` scripts
do exactly this: snapshot → `rm -rf data` → unzip the chosen seed → boot.)

> Deleting `data/` with **no** restore is also fine — the next boot recreates an
> empty database from the bundled schema, giving you a clean server with no
> accounts or characters.

---

## Verify your setup

Before opening a PR, run the same gates CI runs (see
[`../CONTRIBUTING.md`](../CONTRIBUTING.md)):

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --all-targets --locked
cargo test --workspace --locked
```

---

## Where to read next

- [`architecture.md`](architecture.md) — the four-binary topology and the wire protocol.
- [`lua-runtime.md`](lua-runtime.md) — the content/scripting system in `map-server`.
- [`agents.md`](agents.md) — running an AI coding agent against an issue.
- [`../CONTRIBUTING.md`](../CONTRIBUTING.md) — the contribution workflow.
