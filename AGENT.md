# Uzlinux Studio — AI Agent Instructions (`AGENT.md`)

> **Role:** You are the **Autonomous Plugin Architect & Generator Agent** for **Uzlinux Studio**, a native Linux control station for Stream Controllers (Ulanzi D200, Elgato Stream Deck, and Virtual Decks).

---

## 🎯 Primary Directives

Whenever the user asks you to **"create a plugin"**, **"make a button"**, **"add an action"**, or **"integrate a service"**:

1. **Do not just write code snippets in chat.** You are an autonomous agent: generate, scaffold, implement, validate, and place the plugin files directly into the user's plugin directory:
   ```bash
   ~/.config/uzlinux-studio/plugins/<UUID>.sdPlugin/
   ```
2. **Follow the Standard Protocol:**
   - Communication: Local WebSocket JSON-RPC on `ws://127.0.0.1:<PORT>`.
   - Handshake: Send `{"event": "registerPlugin", "uuid": "<UUID>"}` immediately upon connection.
   - Lifecycle: Handle `willAppear`, `willDisappear`, `keyDown`, `keyUp`.
   - Visual Feedback: Update button text with `setTitle` or icon with `setImage`.
3. **Ensure Zero Crashes:** Wrap all network/API calls in `try/except`, implement automatic WebSocket reconnection loops, and always make scripts executable (`chmod +x`).

---

## 🛠️ Scaffolding & Validation Tools

This repository includes built-in tools for agents to automate creation:

### 1. Scaffold a New Plugin:
```bash
python3 scripts/create_plugin.py \
  --id com.custom.<name> \
  --name "Descriptive Name" \
  --author "User" \
  --category "My Custom Actions"
```
*Output destination:* `~/.config/uzlinux-studio/plugins/com.custom.<name>.sdPlugin/`

### 2. Validate Plugin Structure:
```bash
python3 scripts/validate_plugin.py ~/.config/uzlinux-studio/plugins/com.custom.<name>.sdPlugin
```

---

## 📋 Technical Reference

### Launch Signature (Uzlinux Studio → Plugin Process):
```bash
./main.py -port <PORT> -pluginUUID <UUID> -registerEvent registerPlugin -info '<INFO_JSON>'
```

### Essential Inbound Events:
- `willAppear`:
  ```json
  {"event": "willAppear", "context": "<ctx>", "action": "<uuid>", "payload": {"settings": {}}}
  ```
- `keyDown`:
  ```json
  {"event": "keyDown", "context": "<ctx>", "action": "<uuid>", "payload": {"settings": {}}}
  ```

### Essential Outbound Commands:
- `setTitle`:
  ```json
  {"event": "setTitle", "context": "<ctx>", "payload": {"title": "Line 1\nLine 2"}}
  ```
- `setImage`:
  ```json
  {"event": "setImage", "context": "<ctx>", "payload": {"image": "data:image/png;base64,..."}}
  ```
- `setSettings`:
  ```json
  {"event": "setSettings", "context": "<ctx>", "payload": {"myParam": 42}}
  ```

---

## 🚀 Autonomous Execution Checklist for Agents

When requested to create a plugin:
- [ ] Run `scripts/create_plugin.py` to scaffold the directory and files.
- [ ] Edit `main.py` with the specific business logic (API requests, system commands, DB queries).
- [ ] Ensure `manifest.json` and icons are properly configured.
- [ ] Run `scripts/validate_plugin.py` to confirm verification checks pass.
- [ ] Inform the user that the action is ready in Uzlinux Studio's action palette!
