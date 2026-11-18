- `procs` TUI (TODO_procs): every process classified into eight kinds (agent, ide, terminal, service, desktop,
  container, system, kernel) by the nearest marker up its parent chain, kind checkboxes, port / text filter, sort,
  detail pane (cmd, exe, cwd, chain, cgroup, PSS, swap, fds, I/O, CPU history, VRAM), SIGTERM/SIGKILL after `y`;
  compose view grouped by project with stop, start, restart and logs. Collector off the UI thread.
- Sources (docs/decisions.md): procfs (capability), docker CLI 55.5 ms median, nvidia-smi 19.8 ms median, n=15.
- Evidence: 18 unit tests; clippy pedantic clean; cargo deny clean; a pty drive found an own `http.server` by port,
  showed its chain and killed only its PID (exit −15). The drive caught a numeric filter matching argv — fixed,
  regression test `test_filter_number_never_matches_argv`.
- Spine materialized from e-arkhetypo `projects/rust/common`.
- Sandbox drive of the release tarball (ubuntu:24.04, own PID namespace, `--network none`, `--cap-drop ALL`, non-root)
  caught `Esc` leaving an applied filter in place, so a new `/` appended to it (`/4700111`): `Esc` now clears it;
  regression test `test_esc_clears_applied_filter_and_new_filter_starts_fresh`.
- The same sandbox caught a panic in 1 of 17 runs (`index out of bounds` in `selected`): a refresh with fewer processes
  swapped `procs` before reading the kept selection through the old rows. The pid is now read first
  (`common_place`); regression test `test_update_with_fewer_processes_never_reads_stale_rows` reproduces the exact
  panic on the old code. After: 20 of 20 parallel sandbox drives green, 12 cases each (start, no GPU, port filter,
  detail, SIGTERM, trapped TERM, SIGKILL, root EPERM, pid gone, no docker, resize, quit), 0 panics.
