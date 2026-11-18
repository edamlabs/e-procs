# Decisions

## 2026-11-18 — Process, container and GPU sources

- **/proc through `procfs`**, not `sysinfo`: `sysinfo` has no socket-inode map, no cgroup and no PSS — the port
  column, the origin and the kind need all three. A capability gap decides it; no speed bench was needed.
- **`docker` CLI JSON**, not `bollard`: a listing measured 55.5 ms median (p90 58.7, n=15) on 35 containers, off the
  UI thread every 2 s; `bollard` would add an async runtime and a daemon client for no visible gain. Rejected: bollard.
- **`nvidia-smi` query**, not NVML bindings: 19.8 ms median (p90 21.8, n=15), no FFI and no `unsafe`; absent driver →
  empty column. Rejected: `nvml-wrapper`.
