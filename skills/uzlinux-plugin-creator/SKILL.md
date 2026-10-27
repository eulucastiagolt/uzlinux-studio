---
name: uzlinux-plugin-creator
description: Autonomous AI agent skill to design, build, test, and install native plugins for Uzlinux Studio (Stream Controllers, Ulanzi D200, Elgato Stream Deck, and Virtual Decks). Trigger this skill whenever the user asks to create, customize, debug, or generate a plugin, action, or deck button.
---

# Uzlinux Studio Plugin Creator Skill

This skill equips AI agents (Antigravity, Claude, Cursor, Copilot, Roo Code, etc.) with the exact instructions, templates, protocols, and automation tools needed to build working, robust plugins for **Uzlinux Studio** on Linux.

---

## 1. When to Activate This Skill
Trigger this skill whenever the user:
- Asks to create a new button or plugin for **Uzlinux Studio**.
- Asks to automate a task, script, API, or service via a deck key.
- Wants to add custom controls for an app, web service, smart home device, or system utility.
- Wants to fix or inspect an existing `.sdPlugin`.

---

## 2. Core Plugin Architecture & Rules

1. **Target Directory:**
   Every installed plugin on Linux lives in:
   ```bash
   ~/.config/uzlinux-studio/plugins/<UUID>.sdPlugin/
   ```
2. **Process Isolation:**
   Uzlinux Studio executes plugins as independent OS processes. If a plugin crashes, the main app and deck continue running safely.
3. **Command Line Launch Signature:**
   Uzlinux Studio spawns the executable with:
   ```bash
   ./main.py -port <PORT> -pluginUUID <UUID> -registerEvent registerPlugin -info '<INFO_JSON>'
   ```
4. **WebSocket Communication Protocol:**
   - Connect immediately to `ws://127.0.0.1:<PORT>`.
   - Send handshake: `{"event": "registerPlugin", "uuid": "<UUID>"}`.
   - Listen for events:
     - `willAppear`: button is visible. Store `context` identifier.
     - `willDisappear`: button is no longer visible. Remove `context`.
     - `keyDown`: user pressed the button. Perform the action!
     - `keyUp`: user released the button.
   - Send commands:
     - `setTitle`: updates button text (`{"event": "setTitle", "context": context, "payload": {"title": "Text"}}`).
     - `setImage`: updates button graphic (data URI or base64 PNG/SVG).
     - `setSettings`: persists configuration for this button.

---

## 3. Step-by-Step Agent Workflow

When the user asks you to create a plugin:

### Step 1: Clarify the User's Intent
Identify:
- **Action Name & Purpose:** What does the button do?
- **Display Text / Icon:** What should appear on the screen?
- **Trigger Behavior:** Does it toggle, send a request, launch a command, or poll data?
- **Settings:** Does it need user inputs (API token, IP, URL)?

### Step 2: Scaffold Using the Scaffolder Tool
Run the included scaffolder script to create the complete folder structure:
```bash
python3 scripts/create_plugin.py --id com.user.<plugin_name> --name "<Human Name>" --author "<Author>"
```
This generates:
- `manifest.json` with proper Linux `CodePathLin` and action definitions.
- `main.py` with the complete WebSocket event loop, reconnection logic, and shebang.
- `assets/` with SVG vector icons.
- `property_inspector/` with HTML/JS settings panel.

### Step 3: Implement the Custom Logic
Edit `main.py` in the newly created directory (`~/.config/uzlinux-studio/plugins/com.user.<plugin_name>.sdPlugin/main.py`) to implement the requested capability (e.g., calling an API, reading system sensors, toggling a process, or communicating via D-Bus / HTTP).

### Step 4: Validate the Plugin
Run the validator script to ensure full compliance:
```bash
python3 scripts/validate_plugin.py ~/.config/uzlinux-studio/plugins/com.user.<plugin_name>.sdPlugin
```

### Step 5: Inform the User
Tell the user:
- The plugin is installed at `~/.config/uzlinux-studio/plugins/<UUID>.sdPlugin`.
- In **Uzlinux Studio**, it appears in the Action Library in the left sidebar.
- How to drag the action to their deck and configure it.
