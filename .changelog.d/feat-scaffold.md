- `procs` TUI (TODO_procs): every process classified into eight kinds (agent, ide, terminal, service, desktop,
  container, system, kernel) by the nearest marker up its parent chain, kind checkboxes, port / text filter, sort,
  detail pane (cmd, exe, cwd, chain, cgroup, PSS, swap, fds, I/O, CPU history, VRAM), SIGTERM/SIGKILL after `y`;
  compose view grouped by project with stop, start, restart and logs. Collector off the UI thread.
- Sources (docs/decisions.md): procfs (capability), docker CLI 55.5 ms median, nvidia-smi 19.8 ms median, n=15.
- Evidence: 18 unit tests; clippy pedantic clean; cargo deny clean; a pty drive found an own `http.server` by port,
  showed its chain and killed only its PID (exit −15). The drive caught a numeric filter matching argv — fixed,
  regression test `test_filter_number_never_matches_argv`.
- Spine materialized from e-arkhetypo `projects/rust/common`.
