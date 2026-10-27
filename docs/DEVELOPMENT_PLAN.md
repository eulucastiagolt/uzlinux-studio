# Architecture & Engineering Plan: Uzlinux Studio

> **Architecture and Port Engineering Document in Rust**  
> **Target Device Ecosystem:** Ulanzi Stream Controller D200/D200H (`e9-02C53A015U3671380`), open extensibility to Stream Deck devices and Virtual Decks.  
> **Interop Reference:** OpenDeck (`me.amankhanna.opendeck`) and Elgato Stream Deck Plugin Ecosystem.

---

## 1. Executive Summary

**Uzlinux Studio** is an asynchronous, modular, and high-performance desktop station engineered in Rust for modern Linux systems (Wayland / X11). The project provides native hardware integration for **Ulanzi** controllers while guaranteeing seamless interoperability with:
1. **Elgato Stream Deck & OpenAction Plugins** (`.streamDeckPlugin` / `.sdPlugin`).
2. **Embedded HTML/JS Property Inspectors** rendered inside sandboxed WebViews.
3. **OpenDeck Configuration Schemas** (`settings.json`, `applications.json`, `profiles/<device>/<profile>.json`).
4. **Hardware Devices**:
   - Out-of-the-box USB HID driver for **Ulanzi D200 / D200H** with 14 independent displays and continuous status bar.
   - Virtual Deck on-screen simulation.

---

## 2. Workspace Crate Architecture

Uzlinux Studio adopts a clean, decoupled Cargo workspace separation of concerns:

```text
UzlinuxStudioDeck/
├── crates/
│   ├── uzlinux-protocol/          # Inbound & Outbound WebSocket message models
│   ├── uzlinux-core/              # Configuration, profiles, settings, and migration
│   ├── uzlinux-device-driver/     # Hardware drivers (USB HID Ulanzi D200) & Virtual Deck
│   ├── uzlinux-renderer/          # Button composition & ab_glyph typography
│   ├── uzlinux-server/            # WebSocket hubs & local HTTP asset server
│   ├── uzlinux-plugin-manager/    # Process lifecycle, discovery, and isolation
│   ├── uzlinux-window-watcher/    # Active window tracking for auto-profiles
│   └── uzlinux-app/               # Desktop application & Tauri v2 commands
```

---

## 3. Core Milestones Completed

- [x] **Crate 1: uzlinux-protocol**
  - Strongly typed Serde models for all events (`registerPlugin`, `keyDown`, `keyUp`, `willAppear`, `willDisappear`, `setTitle`, `setImage`, `setSettings`).
- [x] **Crate 2: uzlinux-core**
  - Config discovery and automatic non-destructive migration.
  - Profile serialization and dynamic layout management.
- [x] **Crate 3: uzlinux-renderer**
  - High-precision subpixel text rasterization using `ab_glyph`.
  - Layered image composition (backgrounds, overlays, alert badges, state graphics).
- [x] **Crate 4: uzlinux-server**
  - Dynamic local port negotiation (starting from base port 57116).
  - Bidirectional WebSocket hubs for plugins and Property Inspectors.
- [x] **Crate 5: uzlinux-device-driver**
  - Native USB HID driver for Ulanzi D200 with 1000ms heartbeat loop to prevent hardware sleep.
- [x] **Crate 6: uzlinux-plugin-manager**
  - Process supervisor launching external binaries with precise CLI arguments.
- [x] **Crate 7: uzlinux-window-watcher**
  - Background daemon monitoring focused application window class names and triggering automatic profile switches.
- [x] **Crate 8: uzlinux-app**
  - Tauri v2 application, system tray integration, native PipeWire audio, native MPRIS, and complete i18n engine.
