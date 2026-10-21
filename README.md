# Nexora — Local-First AI Coding Studio for Windows

Nexora is a Windows desktop application that provides a conversational AI workspace, codebase understanding, and a review-before-apply patch engine powered by a Large Language Model running locally on your machine.

Nexora does not require paid external AI cloud services or subscriptions for its core functionality. All prompt construction, symbol indexing, file retrieval, diff generation, and model execution are designed to operate entirely on-device, preserving privacy and keeping project code strictly under user control.

---

## 1. Project Overview

Nexora combines a modern desktop shell with a native Rust core that manages local AI execution:
- **Core Purpose:** Deliver a high-performance, private, local-first coding assistant for Windows developers, engineers, and students who want to interact with codebases without uploading source code to third-party clouds.
- **Platform Focus:** Exclusively targeted at the Windows desktop ecosystem (Windows 10 22H2+ and Windows 11 x64) using Tauri 2 and Microsoft Edge WebView2.
- **Implementation Status:**
  - **Implemented & Verified:** All 32 planned phases of the core architecture and feature set—including the native desktop shell, conversation engine, codebase scanner, AST symbol indexer, dependency graph analyzer, review-before-apply patch proposals, local model manager, system settings, premium glassmorphism, Living Aurora ambient dynamics, and offline licensing verification—are implemented and verified by comprehensive automated test suites (83 test files, 833 tests passing).
  - **Unverified in Linux Sandbox:** Compilation of the native Rust crate (`nexora_lib`), generation of the Windows NSIS installer (`.exe`), native Windows DWM glass compositing, and live inference with 7B+ GGUF weights require a physical Windows PC or an authorized Windows CI runner.

---

## 2. Key Features

### 2.1 Workspace Dashboard (`#/home`)
- Centralized overview showing active project status, loaded model identity, system memory metrics, and quick navigation shortcuts.
- Status badges reflect genuine local engine state (`not_configured`, `stopped`, `ready`, `generating`, `error`).

### 2.2 Local AI Chat & Streaming (`#/chat`)
- Conversational interface with real-time token streaming from a local `llama-server.exe` sidecar process via Server-Sent Events (SSE).
- Message lifecycle management with support for immediate generation cancellation, token counting, and draft message retention across page switches.
- Persistent conversation history stored in local JSON storage (`conversations.json`) with rename and deletion capabilities.

### 2.3 Codebase Scanner & Project Explorer (`#/projects`)
- Bounded, iterative read-only directory scanner with configurable limits (max 8,000 files, 12 directory depth levels, 2 MiB inventory cap).
- Traversal protection preventing directory escapes (`..`) and outside symlink following.
- Automatic exclusion of sensitive files (`.env*`, `*.pem`, `*.key`, `id_rsa`, credentials) and standard build artifacts (`node_modules`, `.git`, `dist`, `target`, `__pycache__`).
- Safe, read-only file preview with syntax display and token budget estimation against the active model's context window.

### 2.4 Review-Before-Apply Patch Engine
- Strict JSON patch contracts (`version: 1` single-file, `version: 2` multi-file batch) requiring explicit user confirmation before any disk modification.
- Side-by-side and unified diff visualization showing additions, removals, and context lines.
- Atomic file application using temporary files and `fsync` with automatic rollback on validation failure.

### 2.5 Code Intelligence & Structural Analysis (`#/code`)
- AST and regex-based symbol extraction for TypeScript, JavaScript, Rust, and Python files (`functions`, `classes`, `interfaces`, `types`, `enums`, `imports`, `exports`).
- Deterministic project dependency graph generation with circular dependency detection and change-impact analysis.
- Grounded prompt augmentation providing the model with verified file paths and symbol declarations while filtering sensitive data.

### 2.6 Local Model Management (`#/models`)
- Direct integration with user-supplied GGUF models and the `llama-server.exe` binary.
- Header validation checking GGUF magic bytes, format version 3, and tensor counts.
- Readiness decided by the Rust core, not the UI: fourteen states from `unavailable` and `ready_to_download` through `unverified`, `ready_to_load`, `loaded` and `runtime_failure`. Each row shows the core's explanation, the model file size, the free space a download needs, and an estimated memory figure for the context you configured. See [docs/nexora-sonnet/READINESS.md](docs/nexora-sonnet/READINESS.md).
- Hardware fit estimation comparing published model size and context-derived cache against **available** physical RAM — never total RAM, and never a fixed "minimum" figure.
- Complete process lifecycle control with graceful startup, health probing, and explicit process teardown (`unload_model`).

### 2.7 Workspace Settings & Preference Persistence (`#/settings`)
- Atomic configuration persistence for editor preferences (`codeLigatures`, `reducedMotion`, `founderModeEnabled`).
- Persistent storage of last-opened workspace directories with manual path entry support.

### 2.8 Premium Glassmorphic Design System & Living Aurora
- Dark theme styling with layered glass surfaces (`backdrop-filter: blur(16px)`), luminous borders, and specular highlights.
- Ambient Living Aurora canvas positioned at `z-index: 0` behind interactive glass surfaces at `z-index: 1`.
- Accessible contrast exceeding WCAG 2.1 AA requirements (>14:1 ratio between text and base background).
- Strict motion suppression honoring `@media (prefers-reduced-motion: reduce)` and the `.reduced-motion` user toggle.

### 2.9 Developer Account & Founder Control Centre (`#/account`, `#/founder`)
- Local profile inspection, password change validation, and session termination.
- Real-time in-memory IPC telemetry monitoring invoke volume, error rates, and engine status transitions.
- Security vault audit inspecting operating system credential store status.

### 2.10 Offline Developer Licensing (`#/licensing`)
- Cryptographic offline license key activation and status verification using SHA-256 checksums.
- Clear tier comparisons: Community Edition ($0 forever), Pro, and Team tiers.
- Guaranteed offline sovereignty: zero phone-home requirement; Community features remain fully unlocked forever.

---

## 3. Architecture & Technology Stack

```text
┌────────────────────────────────────────────────────────────────────────┐
│                        Nexora Desktop Window                           │
│                                                                        │
│   ┌────────────────────────────────────────────────────────────────┐   │
│   │               React 19 + TypeScript Frontend                  │   │
│   │   (Router, State Hooks, Chat, Code Intelligence, Glass UI)    │   │
│   └───────────────────────────────┬────────────────────────────────┘   │
│                                   │ Typed IPC Commands                 │
│                                   ▼                                    │
│   ┌────────────────────────────────────────────────────────────────┐   │
│   │                   Tauri 2 Core (Rust)                          │   │
│   │   - Command Dispatcher (lib.rs)                                │   │
│   │   - Workspace Scanner, Symbol Indexer, Patch Engine            │   │
│   │   - Process Supervisor (spawns llama-server with CREATE_NO_WIN)│   │
│   │   - Hardware Probing (Win32 GlobalMemoryStatusEx)              │   │
│   │   - Local Credential Storage (Keyring / Windows DPAPI)         │   │
│   └───────────────────────────────┬────────────────────────────────┘   │
│                                   │ Loopback HTTP (127.0.0.1:port)     │
│                                   │ Auth: Per-session 32-byte key file │
│                                   ▼                                    │
│   ┌────────────────────────────────────────────────────────────────┐   │
│   │              llama.cpp sidecar (llama-server.exe)              │   │
│   │   - GGUF Model Execution (AVX2 / CUDA / Vulkan)                │   │
│   │   - Streaming Chat Completions via Server-Sent Events (SSE)    │   │
│   └────────────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────────────┘
```

### 3.1 Technology Matrix
| Layer | Technology | Version | Purpose |
| :--- | :--- | :--- | :--- |
| **Desktop Shell** | Tauri | 2.12.4 | Native Windows window, IPC bridge, process lifecycle, security sandbox. |
| **Backend Core** | Rust | 1.77.2+ (Pinned 1.93.0 in CI) | High-performance filesystem operations, process supervision, memory probing. |
| **UI Framework** | React | 19.3.1 | Component rendering, conversation virtual lists, state machines. |
| **Language** | TypeScript | 5.9.3 | Static typing, contract synchronization with Rust structs. |
| **Bundler & Server**| Vite | 7.3.7 | Frontend module bundling, development server (`http://localhost:1420`). |
| **Test Runner** | Vitest | 3.2.7 | Unit, integration, DOM interaction, and cross-language contract tests. |
| **Inference Engine**| llama.cpp (`llama-server`) | b11714 pinned target | Local LLM inference sidecar process. |

### 3.2 Local vs. Network Operations
- **100% Local Operations:** UI rendering, workspace scanning, symbol indexing, dependency graphing, patch creation and application, conversation storage, settings persistence, and model inference.
- **Network Boundaries:** Production Content Security Policy strictly blocks outbound HTTP/HTTPS requests from the WebView. The WebView connects only to `ipc:` and `http://ipc.localhost`.
- **User-Managed Assets:** The user provides the `llama-server.exe` executable and `.gguf` model files. Nexora does not download large model weights in the background.

---

## 4. System Requirements

### 4.1 Supported Operating System
- **Target OS:** Windows 10 (version 22H2 or newer) or Windows 11 (64-bit x86_64).
- **Runtime:** Microsoft Edge WebView2 Evergreen Runtime (pre-installed on Windows 11 and modern Windows 10).

### 4.2 Development & Build Prerequisites
- **Node.js:** Version 22 LTS (`v22.14.0+` recommended).
- **Package Manager:** `pnpm` version 9.15.4 or 10.x.
- **Rust Toolchain:** Rust stable `1.77.2+` (installed via `rustup` targeting `x86_64-pc-windows-msvc`).
- **C++ Build Tools:** Visual Studio 2022 C++ Build Tools with "Desktop development with C++" and the Windows 10/11 SDK.

### 4.3 Hardware Guidelines for Local Model Inference
Nexora does not publish a single "minimum RAM" figure, because the requirement is not a
property of the model alone: it is the weights plus a KV cache that grows with the context
window you configure, plus runtime overhead, and it has to fit inside the memory that is
*available*, not the memory installed.

At the recommended 8,192-token context the estimates are:

| Profile | File | Memory estimate @ 8k | @ 32k |
| --- | --- | --- | --- |
| Nexora Mini (7B Q4_K_M) | 4.36 GiB | 5.8 GiB | 7.1 GiB |
| Nexora Sonnet (14B Q4_K_M) | 8.37 GiB | 10.9 GiB | 15.4 GiB |

Plus 512 MiB of headroom on disk for the download itself. The core measures available RAM and
free space at the download folder and refuses a load that cannot fit; the numbers above are
estimates, not guarantees. CPU inference is supported and is the only mode Nexora configures —
no GPU-offload flags are passed — so a 4-core x86_64 CPU with AVX2 support is the practical
floor, and a modern 8-core CPU is recommended for 14B-class models. GPU acceleration is not
claimed. Details and limits: [docs/nexora-sonnet/READINESS.md](docs/nexora-sonnet/READINESS.md).

---

## 5. Installation

### 5.1 End-User Installation Notice
**A pre-built, signed installer is not currently published in this repository.** To install and run Nexora as a native desktop application, build the application from source on a supported Windows machine using the development instructions below.

### 5.2 Target Packaging Specifications
When packaged on a Windows machine:
- **Installer Type:** NSIS per-user installer (`Nexora_0.1.0_x64-setup.exe`).
- **Install Location:** `%LOCALAPPDATA%\Programs\Nexora\`.
- **Administrative Rights:** None required (`installMode: "currentUser"`).
- **System Footprint:** No background Windows services, no scheduled tasks, and no kernel drivers installed.

---

## 6. Development Setup

Run all commands from the repository root:

```bash
# 1. Install workspace dependencies
pnpm install --frozen-lockfile

# 2. Run TypeScript typecheck across all workspace packages
pnpm typecheck

# 3. Run the automated test suite (75 test files, 724 tests)
pnpm test

# 4. Compile production frontend assets
pnpm build

# 5. Start the frontend development server (browser preview)
pnpm dev
```

### 6.1 Native Windows Build & Verification Commands
The following commands require a genuine Windows workstation with the required C++ build tools and Rust toolchain:

```powershell
# Run the desktop application in development mode (starts Vite + native window)
pnpm tauri dev

# Typecheck Rust backend crate with Windows-specific cfg flags
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets

# Run Rust unit and contract tests
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml

# Compile release binary
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --release

# Package the final Windows NSIS installer
pnpm tauri build
```

*Note: In the Linux development container, `cargo` and `rustc` are not installed, and crates.io network egress is restricted. Run the Rust and packaging commands on a native Windows environment.*

---

## 7. Local Model Setup and Usage

Nexora relies on a local instance of llama.cpp rather than proprietary cloud APIs:

1. **Obtain the Runtime:**
   Download or build `llama-server.exe` from the official [llama.cpp repository](https://github.com/ggml-org/llama.cpp/releases) (target build: b11714).
2. **Obtain a Model:**
   Download a compatible `.gguf` file (e.g., `qwen2.5-coder-7b-instruct.Q4_K_M.gguf` or `deepseek-coder-6.7b-instruct.Q4_K_M.gguf`) from Hugging Face or other trusted sources.
3. **Configure in Nexora:**
   - Launch Nexora and open **Local Models** (`#/models`).
   - Enter the full path to `llama-server.exe` (e.g., `C:\llama\llama-server.exe`).
   - Enter the full path to your `.gguf` model (e.g., `C:\models\qwen-coder-7b.gguf`).
   - Configure context window size (default: 32,768 tokens; clamped between 2,048 and 131,072 tokens).
4. **Validation Gate:**
   - Nexora probes the runtime via `llama-server --version` to confirm compatibility.
   - Nexora inspects the GGUF header and calculates whether the model fits into physical RAM before starting the process.
5. **Execution & Lifecycle:**
   - On load, Nexora spawns `llama-server.exe` using `CREATE_NO_WINDOW` on an ephemeral loopback port (`127.0.0.1`) authenticated via a freshly generated 32-byte key file (`runtime-session.key`).
   - Clicking **Stop Engine** or exiting the application immediately sends a termination signal, draining standard error pipes and releasing system memory.

---

## 8. Project Structure

```text
Nexora/
├── .github/
│   └── workflows/
│       └── ci.yml                     # Continuous integration workflow (Linux & Windows jobs)
├── apps/
│   └── desktop/                       # Main Tauri desktop package
│       ├── package.json
│       ├── src-tauri/                 # Rust native desktop core
│       │   ├── Cargo.toml
│       │   ├── tauri.conf.json        # Tauri configuration (NSIS, window geometry, CSP)
│       │   ├── capabilities/          # Least-privilege window permission allowlists
│       │   ├── icons/                 # Windows application icons (.ico, .png)
│       │   └── src/
│       │       ├── lib.rs             # Tauri command registration & plugin initialization
│       │       ├── main.rs            # Desktop binary entry point (windows_subsystem suppress)
│       │       ├── inference/         # Process supervisor, GGUF validator, hardware probe
│       │       ├── workspace/         # Filesystem scanner, AST symbol indexer, patch engine
│       │       ├── history/           # Conversation persistence store
│       │       ├── auth/              # Account commands, PKCE listener, Windows keyring
│       │       └── licensing/         # Offline cryptographic license validation
│       └── ui/                        # React frontend application
│           ├── index.html
│           ├── vite.config.ts
│           ├── vitest.config.ts
│           └── src/
│               ├── App.tsx            # Root component, router composition, keyboard shortcuts
│               ├── components/        # TitleBar, Sidebar, Composer, Panels, Glass controls
│               ├── pages/             # Dashboard, Chat, Projects, Code, Models, Settings
│               ├── features/          # Domain hooks (useConversation, useWorkspace, usePatch)
│               ├── lib/               # Typed IPC clients, router, evidence, patch contracts
│               └── styles/            # Design tokens, Living Aurora, glassmorphism CSS
├── docs/                              # Authoritative documentation and phase audit reports
│   ├── phase-1/                       # Foundational architecture, security, and build specs
│   ├── phase-2/                       # Spike research and llama.cpp integration parameters
│   ├── phase-23h/                     # Offline AI regression & symbol syntax fix record
│   ├── phase-24/ ... phase-30/        # Implementation records for individual product features
│   ├── phase-31/                      # Comprehensive Quality Assurance audit report
│   └── phase-32/                      # Final Windows Verification & readiness report
├── resources/                         # Application branding and visual marks
├── services/
│   └── nexora-auth/                   # Standalone authentication service (reference backend)
├── package.json                       # Root workspace scripts and toolchain pins
├── pnpm-lock.yaml                     # Deterministic package dependency lockfile
├── pnpm-workspace.yaml                # Monorepo workspace configuration
└── rust-toolchain.toml                # Pinned Rust toolchain version (1.93.0)
```

---

## 9. Security & Privacy

Nexora is engineered around a strict local-first security model:

1. **Strict Content Security Policy (CSP):**
   ```text
   default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline';
   img-src 'self' data: asset: http://asset.localhost; font-src 'self';
   connect-src ipc: http://ipc.localhost
   ```
   Direct outbound HTTP/HTTPS communication from the WebView is blocked at the protocol level.
2. **Least-Privilege Tauri Capabilities:**
   `apps/desktop/src-tauri/capabilities/main.json` restricts the main window to essential window controls (`minimize`, `toggle-maximize`, `close`, `start-dragging`). Unrestricted shell execution, arbitrary network access, and broad filesystem permissions are strictly withheld.
3. **Loopback-Only Ephemeral Process Binding:**
   The `llama-server.exe` process is bound exclusively to `127.0.0.1` on an operating system-assigned ephemeral port. Access requires a 32-byte cryptographically secure random key stored in a temporary keyfile (`0600` permissions on Unix; non-shared user profile on Windows) and passed via `--api-key-file`.
4. **Filesystem Traversal Defenses:**
   Project scanning validates canonical paths against the selected workspace root. Path segments containing `..` or symbolic links pointing outside the root directory are rejected.
5. **Secret Protection:**
   The workspace scanner automatically skips private keys, password stores, and environment files (`.env*`, `*.pem`, `*.key`, `id_rsa`, `credentials.json`). Sensitive file contents are never attached to model prompt context.
6. **Non-Mutating Patch Approvals:**
   AI-generated code proposals are treated as untrusted text. No code is compiled, executed, or written to disk without explicit review and approval by the user.

---

## 10. Testing & Verification Status

### 10.1 Automated Verification (Executed in Sandbox)
All automated frontend, contract, and integration tests pass cleanly:
- **Total Test Suites:** **75 / 75 passed** (100%)
- **Total Unit & Integration Tests:** **724 / 724 passed** (100%)
- **TypeScript Static Typecheck:** `tsc --noEmit` exited with code `0` (zero type errors).
- **Vite Production Bundler:** Production client bundle built in 2.04s (`index-BaD5w8Y6.css` 93.4 kB, `index-cOYSa9hs.js` 522.6 kB).
- **Formatting & Whitespace Integrity:** `git diff --check` passed with zero errors.

### 10.2 Tested vs. Native Windows Verification Separation
| Area | Linux Sandbox Status | Native Windows Status | Verification Detail |
| :--- | :--- | :--- | :--- |
| **Frontend UI & Components** | **VERIFIED** | Ready for display | Full React component lifecycle, navigation, and state machines tested. |
| **Tauri Window & Chrome Wiring**| **VERIFIED** | Ready for execution | Titlebar window drag region and window control IPC contracts verified. |
| **Cross-Language IPC Contracts** | **VERIFIED** | Ready for execution | Serde field mapping, commands, and error payloads validated against Rust sources. |
| **Offline Sovereignty** | **VERIFIED** | Ready for execution | Proved zero network HTTP fetch attempts and 100% functionality without auth. |
| **Rust Desktop Compilation** | **BLOCKED** | Requires Windows/CI | Local sandbox lacks `cargo` and `rustc`. Ready for Windows CI runner. |
| **NSIS Installer Packaging** | **BLOCKED** | Requires Windows/CI | Building `.exe` installer requires native Windows tools (`makensis`). |
| **Direct DWM Glass Compositing**| **BLOCKED** | Requires Windows GUI | Headless Linux container lacks graphical display server. |
| **Live `llama-server.exe` Spawning**| **BLOCKED** | Requires Windows PC | Spawning Windows PE executable requires genuine Windows kernel. |
| **Real GGUF Model Inference** | **BLOCKED** | Requires Windows PC | Execution of 7B+ weights requires physical CPU/GPU hardware. |
| **Windows Credential Vault** | **BLOCKED** | Requires Windows PC | Win32 DPAPI storage round-trip requires genuine Windows OS service. |

---

## 11. Known Limitations

- **Headless Sandbox Constraints:** The current development container is Linux-based and lacks the Rust compiler, local GPU hardware, and the Windows Desktop Window Manager. Native compilation and visual desktop rendering must be verified on a Windows machine.
- **Windows 11 Snap Layouts:** Because the application uses custom window chrome (`decorations: false`), the native Windows 11 hover flyout on the maximize button is not supported by Tauri without returning `HTMAXBUTTON` from `WM_NCHITTEST`.
- **Model Hardware Bounds:** Running large language models locally is computationally intensive. Attempting to allocate models that exceed available physical RAM will result in performance degradation or engine rejection.

---

## 12. Troubleshooting

### 12.1 Engine Reports "Failed to start" or "Exit code 1"
- **Cause:** The path to `llama-server.exe` or the model file is incorrect, or port `127.0.0.1` is blocked by local firewall software.
- **Resolution:** Verify that `llama-server.exe` exists at the specified path and can be executed from a command prompt (`llama-server.exe --version`). Ensure the model file is not corrupted and has a `.gguf` extension.

### 12.2 Model Exceeds Available Memory Warning
- **Cause:** The model's tensor weights and context budget exceed physical RAM.
- **Resolution:** Select a higher-quantization model (e.g., `Q4_K_M` instead of `Q8_0` or `FP16`) or reduce the context window size in **Local Models** (`#/models`) to 8,192 or 16,384 tokens.

### 12.3 Workspace Scanner Skips Expected Files
- **Cause:** Files are located inside hidden directories, match `.gitignore` rules, or exceed size thresholds.
- **Resolution:** Nexora ignores files exceeding 256 KiB for context and skips standard build directories (`node_modules`, `dist`, `target`). Verify your project folder structure.

---

## 13. Project Status & Contribution Guidance

- **Project Status:** The original 32-phase engineering roadmap for Nexora is complete. The repository is stabilized for testing and local Windows packaging.
- **Making Changes:**
  1. Ensure all changes pass `pnpm typecheck` and `pnpm test`.
  2. Maintain strict separation of concerns between the Tauri IPC layer and the React frontend.
  3. Never commit API keys, credentials, or personal model weights to the repository.
  4. Ensure `git diff --check` passes with zero whitespace or line-ending errors.

---

## 14. License

No license has been identified in the repository root. All rights are reserved by the repository owner unless otherwise specified.
