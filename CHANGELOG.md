# Changelog

## [0.2.0] - 2026-11-19

- Compose joins the kinds menu as `9 compose`, in its own colour (light red, also on the compose tab and table border): `9` or a click opens it and closes it; `1`–`8` from compose go back showing that kind, never hiding it. The kinds pane stays beside the compose table; the ports toggle moved one line down.
- Evidence: `make check` green (31 tests, 4 new); pty drive at 140 columns: `9 [ ] compose 36` drawn bold in red, `9` opens compose, a click on the line returns, `7` from compose turns system on and shows it.

## [0.1.0] - 2026-11-18

- Selection visible and still: rows that tie on the sort key fall back to the PID, so equal-CPU processes no longer reshuffle every refresh and carry the selection off screen; a 250 ms warm-up sample gives the first snapshot real CPU; the selected row carries a `▶` mark.
- Mouse: the wheel moves the selection (or scrolls logs), a click selects a row, a second click opens its detail, a click toggles a kind or switches tab; capture is released on exit and on panic.
- Narrow terminals: under 100 columns the kinds pane folds into the table title; the VRAM column shows only where a process holds GPU memory. Footer and README name the arrow keys.
- Evidence: `make check` green (27 tests, 7 new); pty drive at 160 and 80 columns: selection stays on row 1 across refreshes, wheel and click move it, second click opens the detail, tab click switches, mouse capture off at exit.
- TODO_procs closed FINISHED: procs built, sandboxed, installed from main and driven; the mouse and stable-selection fix found by Damien in use; the release moves to TODO_release.
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
