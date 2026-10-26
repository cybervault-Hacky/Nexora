# Host inventory relevant to verification

Observed during this campaign; see `tauri-environment.log` for the verbatim Tauri CLI diagnostic.

- OS/architecture: Debian 12, Linux x86_64; Tauri reports unknown desktop environment/session.
- Available memory: approximately 3.6 GiB; filesystem free space was approximately 20 GB at inventory time. These are environment observations, not inference-memory or compatibility measurements.
- Node.js 22.22.3, npm 10.9.8. pnpm 9.15.4 was invoked through Corepack (`corepack pnpm`) because `pnpm` was not directly on PATH.
- `rustc`, `cargo`, `rustup`, `webkit2gtk-4.1`, and `rsvg2` unavailable, per Tauri diagnostic. No Rust toolchain or native build was run.
- No GUI desktop session/display was established. No Windows host/toolchain, `nvidia-smi`, `vulkaninfo`, or FFmpeg executable was available in the captured inventory.
- The host did not provide a verified compatible media runtime/GPU path. Hardware detection, if any, would not by itself establish runtime compatibility.
