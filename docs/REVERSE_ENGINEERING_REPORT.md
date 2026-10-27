# Technical Compatibility and Reverse Engineering Report: OpenDeck

> **Reference Application:** OpenDeck (`me.amankhanna.opendeck`, version 2.14.2)  
> **Tooling:** REA MCP, Ghidra, and ELF Binary / Runtime Inspection  
> **Goal:** Ensure 100% interoperability with profiles, plugins, and WebSocket APIs in **Uzlinux Studio**

---

## 1. Binary Identification

* **Executable Target:** `/var/lib/flatpak/app/me.amankhanna.opendeck/x86_64/stable/active/files/bin/opendeck`
* **Format:** ELF 64-bit LSB PIE (x86-64), dynamically linked
* **SHA-256 Hash:** `a35f1de5cf2b8ac714933db3d0e9f7d479401b18a09e9e87a18c9f8df2a9d7bf`
* **Size:** ~55 MB
* **Framework:** Tauri v2, GTK 3, WebKit2GTK 4.1

---

## 2. Profile and Storage Schemas

OpenDeck stores its configurations in `~/.config/opendeck/` or Flatpak sandbox paths:

### 1. `settings.json`
Contains general user settings:
```json
{
  "autolaunch": false,
  "background": true,
  "brightness": 80,
  "language": "auto",
  "sleep_timeout_minutes": 30,
  "sleep_when_computer_locked": true,
  "version": "2.14.0",
  "disabled_plugins": []
}
```

### 2. `profiles/<device_id>/<profile_name>.json`
Stores 15 keys (3x5 grid) with action identifiers, custom titles, images, and plugin-specific state.

Uzlinux Studio provides automatic non-destructive reading, importing, and syncing of these profiles into its isolated directory `~/.config/uzlinux-studio/`.

---

## 3. WebSocket Interop Verification

The WebSocket protocol conforms to the Stream Deck / OpenAction specification:
- Ports dynamically assigned per instance.
- Handshake `registerPlugin` and `registerPropertyInspector`.
- Bi-directional events (`keyDown`, `keyUp`, `willAppear`, `willDisappear`, `setTitle`, `setImage`, `setSettings`).

Uzlinux Studio achieves full compatibility without relying on OpenDeck binaries.
