# e-procs

`procs` — see, filter, inspect and kill every process and compose container from one terminal table. Linux.

```
cargo install e-procs        # installs the `procs` binary
procs
```

## Processes

Every process, classified by who launched it — the nearest marker up its parent chain decides:

| kind | is |
|---|---|
| agent | under `claude`, `codex`, `opencode`, `gemini`, `aider`, `goose` |
| ide | under VS Code, Cursor, Zed, JetBrains |
| terminal | under a terminal emulator or `tmux` |
| service | a `systemd --user` unit |
| desktop | anything else of yours: apps the desktop launched |
| container | inside a docker / podman / kubernetes cgroup |
| system | another user's (root, daemons) |
| kernel | kernel threads |

It opens on **agent · ide · terminal**: what you launched. Each row: PID, kind, name, CPU %, RSS, VRAM (NVIDIA),
listening ports, uptime and origin (the parent chain). `⏎` opens the detail: command, executable, working directory,
full chain with PIDs, cgroup unit, PSS, swap, threads, open fds, disk I/O and a CPU history.

| key | does |
|---|---|
| `1`–`8` | toggle a kind · `a` all · `m` mine |
| `p` | only processes listening on a port |
| `/` | filter: a number is a PID or a port, text matches name and command |
| `s` | sort by cpu · mem · pid · name |
| `⏎` / `Esc` | open / close the detail; `Esc` with the detail closed clears the filter |
| `t` / `K` | SIGTERM / SIGKILL, after `y` |
| `⇥` | compose view · `q` quit |

## Compose

Every container, grouped by compose project: state, status, ports. `x` stop, `r` restart (both after `y`),
`u` start, `l` the last 300 log lines.

## Notes

- Another user's sockets are unreadable without privilege: their ports show empty; `sudo procs` sees them.
- Docker through the `docker` CLI, VRAM through `nvidia-smi`; without either, that column or view says so.

## Develop

`make install` (toolchain, cargo-deny, hooks) · `make check` (fmt, clippy with the house lints, tests, deny) ·
`make run`. The project spine comes from e-arkhetypo `projects/rust/common`.

## Release

Bump `version` in `Cargo.toml` and `make fold` in that PR; once merged, `make release` on main tags `vX.Y.Z`.
The tag builds the GitHub Release (Linux x86_64 tarball + `e-procs-installer.sh`, by cargo-dist) and publishes the
crate (`crates.yml`, secret `CARGO_REGISTRY_TOKEN`). Locally: `make install-bin` → `~/.cargo/bin/procs`.
