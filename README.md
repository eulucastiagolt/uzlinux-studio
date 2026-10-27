# Uzlinux Studio 🎛️

> **Native Control and Automation Station for Stream Controllers in Rust.**  
> **Uzlinux Studio** is a high-performance desktop application built with a primary focus on deep integration with **Ulanzi** hardware (such as the **Ulanzi Stream Controller D200 / D200H** and other models), while providing open support for other stream controllers on the market.  
> It provides full interoperability with configuration files, profiles, actions, and plugins compatible with OpenDeck and Elgato Stream Deck, rebuilt from scratch with a modern asynchronous Rust architecture (Wayland / X11, native PipeWire, native MPRIS, and Tauri v2).

---

## 🌟 Key Features

- **Built & Optimized for Ulanzi Hardware (and compatible devices):**
  - Deep hardware integration with the **Ulanzi Stream Controller D200 / D200H**, featuring 14 independent LCD button displays and continuous status bar.
  - Core-embedded hardware driver (*Zero-Setup*) with connection monitoring and continuous 1000ms heartbeat loops to keep device firmware awake and active.
  - Open extensibility for other stream controllers and a built-in **Virtual Deck** mode for testing or on-screen touch control without physical hardware.
- **Native Core Actions & Built-in Services:**
  - Native Linux Audio powered by **PipeWire** (fine volume adjustment, output/input switching, mute toggle, Push-To-Talk) and native media control via **MPRIS** without relying on external plugins.
  - Real-time system actions: Shell execution (`runcommand`), web URLs (`openurl`), profile switching (`switchprofile`), backlight brightness adjustment (`brightness`), and simulated input/shortcuts (`simulateinput`).
- **Comprehensive Multi-Language (i18n) Engine:**
  - Automatic system language detection (via OS locale/environment).
  - Defaults to English if unidentified or unsupported.
  - Real-time language switching directly in Settings (English, Brazilian Portuguese, and extensible catalogs) with immediate UI re-rendering.
- **Autonomous AI Agent Integration & Scaffolding:**
  - Native support for autonomous coding agents (Antigravity, Claude Code, Cursor, Windsurf, Copilot) via universal [`AGENT.md`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/AGENT.md) directives and a dedicated Agent Skill ([`skills/uzlinux-plugin-creator/`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/skills/uzlinux-plugin-creator/SKILL.md)).
  - Instant CLI scaffolding (`python3 scripts/create_plugin.py`) and automated compliance validation (`python3 scripts/validate_plugin.py`), allowing an AI agent to build and install working deck buttons directly from natural language prompts.
- **Open Ecosystem Compatibility:**
  - Selective or full one-click import of profiles, plugins, and settings from OpenDeck.
  - Embedded HTML/JS Property Inspectors loaded in isolated WebViews via WebSocket and `postMessage` bridge.
- **Crisp Typography & Dynamic Rendering:**
  - High-performance button text and label rendering with contrast outlines, customizable alignments (`top`, `middle`, `bottom`), and subpixel alpha blending using `ab_glyph`.

---

## 📚 Documentation

Detailed developer and user guides are available in the [`docs/`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/docs) directory:

1. 🛠️ **[Plugin Development Guide](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/docs/PLUGIN_DEVELOPMENT_GUIDE.md)**
   - In-depth technical specification of the process-isolated plugin architecture.
   - CLI arguments (`-port`, `-pluginUUID`, `-registerEvent`, `-info`).
   - JSON-RPC WebSocket protocol specification (`setImage`, `setTitle`, `keyDown`, `willAppear`, etc.).
   - Creating HTML/JS Property Inspectors and communicating via `postMessage`.
   - Full reference implementations in **Python**, **Rust**, and **Shell Script**.

2. 🤖 **[AI Agents & Automated Plugin Creation Guide](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/docs/AI_CREATION_GUIDE.md)**
   - Connect any AI Agent (Antigravity, Claude Code, Cursor, Copilot, ChatGPT) to autonomously design, test, and install plugins.
   - Comprehensive instructions on using [`AGENT.md`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/AGENT.md) and [`CLAUDE.md`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/CLAUDE.md).
   - Dedicated Agent Skill specifications in [`skills/uzlinux-plugin-creator/`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/skills/uzlinux-plugin-creator/SKILL.md).
   - Automated CLI scaffolding (`scripts/create_plugin.py`) and validator (`scripts/validate_plugin.py`).

3. 📐 **[Architecture and Development Plan](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/docs/DEVELOPMENT_PLAN.md)**
   - Full technical architecture, crate breakdown, and device protocol specifications.

---

## 🚀 Building & Running

### System Prerequisites (Debian / Ubuntu / Fedora / Arch):
- Rust 1.80+ (`cargo`, `rustc`)
- Node.js 18+ and npm
- Linux development libraries: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libudev-dev`

### Running in Development Mode:
```bash
./dev.sh
```

### Compiling Optimized Release Binary:
```bash
cargo build --release
```
The optimized executable will be generated at:
```text
target/release/uzlinux-studio
```

---

## 📂 Workspace Structure

- [`crates/uzlinux-app/`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/crates/uzlinux-app): Central desktop application and Tauri v2 commands.
- [`crates/uzlinux-core/`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/crates/uzlinux-core): Configuration, profiles, and settings management with automatic migration.
- [`crates/uzlinux-renderer/`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/crates/uzlinux-renderer): High-performance graphics composition engine and typography using `ab_glyph`.
- [`crates/uzlinux-server/`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/crates/uzlinux-server): Dynamic WebSocket hubs for plugins/inspectors and embedded HTTP asset server.
- [`crates/uzlinux-protocol/`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/crates/uzlinux-protocol): Stream Deck inbound/outbound WebSocket event models.
- [`crates/uzlinux-device-driver/`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/crates/uzlinux-device-driver): Native USB HID device drivers (Ulanzi D200) and Virtual Deck.
- [`bundled_plugins/`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/bundled_plugins): Official bundled plugins shipped out-of-the-box.
