# Official Plugin Development Guide — Uzlinux Studio

> **Compatibility:** Uzlinux Studio v0.1.0+ / OpenDeck / Elgato Stream Deck & OpenAction Standard  
> **Target Audience:** Software developers, system integrators, and automation engineers.  
> **Supported Languages:** Any language running on Linux (Rust, Python, Node.js, Go, C/C++, Shell Script, etc.).

---

## 1. Overview and Architecture

**Uzlinux Studio** utilizes an architecture built upon **Process Isolation** and event-driven asynchronous communication. Unlike monolithic applications that execute third-party scripts within their main UI thread (which can freeze windows or cause unrecoverable crashes), Uzlinux Studio supervises plugins as independent Linux processes.

### Communication Architecture

```text
┌────────────────────────────────────────────────────────────────────────┐
│                            Uzlinux Studio                              │
│                                                                        │
│  ┌───────────────────────┐             ┌────────────────────────────┐  │
│  │   Plugin WebSocket    │             │      Asset HTTP Server     │  │
│  │  Hub (ws://127.0.0.1) │             │ (http://127.0.0.1:<port>)  │  │
│  └───────────▲───────────┘             └─────────────▲──────────────┘  │
│              │                                       │                 │
│              │ Bidirectional                         │ HTTP / Assets   │
│              │ JSON-RPC                              │ & HTML Bridge   │
└──────────────┼───────────────────────────────────────┼─────────────────┘
               │                                       │
               ▼                                       ▼
┌──────────────────────────────┐       ┌─────────────────────────────────┐
│        Plugin Process        │       │       Property Inspector        │
│   (Python / Rust / Node /    │◄──────┤  (Embedded HTML/JS Settings     │
│         ELF Binary)          │       │     Panel inside Tauri Window)  │
└──────────────────────────────┘       └─────────────────────────────────┘
```

### Key Components:
1. **Plugin Process**: An executable binary or script spawned by Uzlinux Studio upon startup or on-demand.
2. **Plugin WebSocket Hub**: Secure local WebSocket server that receives outbound commands and dispatches device events (key presses, dial turns, window focus changes).
3. **Asset Server**: Embedded HTTP server that serves static files (action icons, HTML pages, CSS/JS resources) from plugin directories.
4. **Property Inspector (PI)**: The interactive configuration interface presented in Uzlinux Studio's right sidebar when a user clicks a key. It runs inside an isolated WebView.

---

## 2. Plugin Directory Structure

On Linux, installed plugins reside in:
```bash
~/.config/uzlinux-studio/plugins/
```

Each plugin directory name **must end with** `.sdPlugin` and adhere to reverse domain notation (`com.author.pluginname.sdPlugin`).

### Standard Layout:

```text
com.example.myplugin.sdPlugin/
├── manifest.json                  # [REQUIRED] Plugin metadata, actions, and entry points
├── my_plugin                      # [REQUIRED] Main executable or script (chmod +x)
├── assets/                        # Action icons and button graphics
│   ├── icon.png                   # 72x72 icon displayed in the action palette
│   ├── icon@2x.png                # 144x144 high-DPI icon
│   ├── state_off.png              # Inactive state graphic
│   └── state_on.png               # Active state graphic
└── property_inspector/            # [OPTIONAL] Visual settings panel
    ├── pi.html                    # HTML view loaded in the sidebar
    ├── pi.css                     # Custom styling
    └── pi.js                      # UI logic communicating with plugin and app
```

---

## 3. The `manifest.json` File

The `manifest.json` file declares the plugin metadata, exported actions, categories, icons, and binary entry points.

### Complete `manifest.json` Example:

```json
{
  "Name": "System Monitor",
  "Description": "Displays CPU, RAM, and thermal metrics on Uzlinux Studio",
  "Author": "Your Name",
  "Version": "1.0.0",
  "PluginUUID": "com.example.monitor",
  "CodePathLin": "my_plugin",
  "Icon": "assets/icon",
  "Category": "Linux System",
  "OS": [
    {
      "Platform": "linux"
    }
  ],
  "Actions": [
    {
      "UUID": "com.example.monitor.cpu",
      "Name": "CPU Usage",
      "Tooltip": "Shows current CPU utilization percentage",
      "Icon": "assets/cpu",
      "Controllers": [
        "Keypad"
      ],
      "PropertyInspectorPath": "property_inspector/pi.html",
      "States": [
        {
          "Image": "assets/cpu_idle",
          "Title": "CPU",
          "TitleColor": "#ffffff",
          "TitleAlignment": "middle",
          "FontSize": 12,
          "ShowTitle": true
        }
      ]
    }
  ]
}
```

### Fields Breakdown:

| Field | Type | Description |
| :--- | :--- | :--- |
| `Name` | String | User-facing plugin name in the application. |
| `Description` | String | Brief summary of the plugin's capability. |
| `Author` | String | Creator name or organization. |
| `Version` | String | SemVer version string (`1.0.0`). |
| `PluginUUID` | String | Globally unique plugin identifier (e.g. `com.company.plugin`). |
| `CodePathLin` | String | Relative path to the executable on Linux (e.g. binary or script with `chmod +x` and shebang). |
| `Icon` | String | Relative path to the icon without file extension (looks for `.png`). |
| `Category` | String | Category group where actions are placed in the sidebar palette. |
| `OS` | Array | Supported operating systems. On Linux: `[{"Platform": "linux"}]`. |
| `Actions` | Array | List of actions provided by this plugin. |

---

## 4. Plugin Lifecycle and Launch Arguments

When Uzlinux Studio launches, it traverses the plugin directories and invokes the binary specified in `CodePathLin` with the following CLI arguments:

```bash
./my_plugin -port <PORT> -pluginUUID <UUID> -registerEvent registerPlugin -info '<INFO_JSON>'
```

### Command Line Arguments:
* `-port`: Local TCP port of the WebSocket server (e.g. `57116`).
* `-pluginUUID`: Plugin UUID matching `manifest.json`.
* `-registerEvent`: Handshake event name (`registerPlugin`).
* `-info`: Stringified JSON containing host application metadata and connected devices:

```json
{
  "application": {
    "font": "Liberation Sans",
    "language": "en",
    "platform": "linux",
    "platformVersion": "6.x",
    "version": "2.14.0"
  },
  "plugin": {
    "uuid": "com.example.monitor",
    "version": "1.0.0"
  },
  "devicePixelRatio": 1,
  "devices": [
    {
      "id": "e9-02C53A015U3671380",
      "name": "Ulanzi D200",
      "size": {
        "columns": 5,
        "rows": 3
      },
      "type": 0
    }
  ],
  "colors": {}
}
```

---

## 5. WebSocket Communication Protocol

Once started, the plugin must immediately connect to `ws://127.0.0.1:<PORT>`.

### Step 1: Registration Handshake (Mandatory)
The first message sent over WebSocket must be:

```json
{
  "event": "registerPlugin",
  "uuid": "com.example.monitor"
}
```

---

### Inbound Events (App → Plugin)

#### 1. `willAppear`
Triggered when an action key becomes visible on the active deck profile:
```json
{
  "event": "willAppear",
  "action": "com.example.monitor.cpu",
  "context": "ctx_unique_key_1234",
  "device": "e9-02C53A015U3671380",
  "payload": {
    "settings": {
      "interval": 2,
      "alertThreshold": 85
    },
    "coordinates": {
      "column": 1,
      "row": 0
    },
    "state": 0,
    "isInMultiAction": false,
    "controller": "Keypad"
  }
}
```

#### 2. `willDisappear`
Triggered when an action key leaves the visible view (e.g. page or profile switch). Useful to pause intervals:
```json
{
  "event": "willDisappear",
  "action": "com.example.monitor.cpu",
  "context": "ctx_unique_key_1234",
  "device": "e9-02C53A015U3671380",
  "payload": {
    "settings": {},
    "coordinates": { "column": 1, "row": 0 },
    "state": 0,
    "isInMultiAction": false,
    "controller": "Keypad"
  }
}
```

#### 3. `keyDown` & `keyUp`
Dispatched when the physical hardware button is pressed or released:
```json
{
  "event": "keyDown",
  "action": "com.example.monitor.cpu",
  "context": "ctx_unique_key_1234",
  "device": "e9-02C53A015U3671380",
  "payload": {
    "settings": { "interval": 2 },
    "coordinates": { "column": 1, "row": 0 },
    "state": 0,
    "userDesiredState": 1,
    "isInMultiAction": false
  }
}
```

---

### Outbound Commands (Plugin → App)

#### 1. `setTitle`
Updates the text rendered on the physical button LCD:
```json
{
  "event": "setTitle",
  "context": "ctx_unique_key_1234",
  "payload": {
    "title": "CPU\n42%",
    "target": 0,
    "state": 0
  }
}
```

#### 2. `setImage`
Dynamically renders a new graphic or base64 image on the button screen:
```json
{
  "event": "setImage",
  "context": "ctx_unique_key_1234",
  "payload": {
    "image": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAA...",
    "target": 0,
    "state": 0
  }
}
```

#### 3. `setSettings`
Persists user configuration for this specific button instance:
```json
{
  "event": "setSettings",
  "context": "ctx_unique_key_1234",
  "payload": {
    "interval": 5,
    "alertThreshold": 90
  }
}
```

---

## 6. Practical Reference Implementation (Python)

Below is a complete, production-ready Python plugin (`main.py`):

```python
#!/usr/bin/env python3
import sys
import json
import argparse
import asyncio
import websockets

active_contexts = set()

async def run_plugin(port, plugin_uuid):
    uri = f"ws://127.0.0.1:{port}"
    async with websockets.connect(uri) as ws:
        # 1. Register Plugin
        await ws.send(json.dumps({
            "event": "registerPlugin",
            "uuid": plugin_uuid
        }))

        # 2. Background update worker
        asyncio.create_task(update_worker(ws))

        # 3. Handle incoming events
        async for raw_message in ws:
            msg = json.loads(raw_message)
            event = msg.get("event")
            context = msg.get("context")

            if event == "willAppear":
                active_contexts.add(context)
                await ws.send(json.dumps({
                    "event": "setTitle",
                    "context": context,
                    "payload": { "title": "Online\nReady" }
                }))

            elif event == "willDisappear":
                active_contexts.discard(context)

            elif event == "keyDown":
                await ws.send(json.dumps({
                    "event": "setTitle",
                    "context": context,
                    "payload": { "title": "Pressed!" }
                }))

async def update_worker(ws):
    counter = 0
    while True:
        await asyncio.sleep(2)
        counter += 1
        for ctx in list(active_contexts):
            try:
                await ws.send(json.dumps({
                    "event": "setTitle",
                    "context": ctx,
                    "payload": { "title": f"Count\n{counter}" }
                }))
            except Exception:
                pass

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("-port", type=int, required=True)
    parser.add_argument("-pluginUUID", type=str, required=True)
    parser.add_argument("-registerEvent", type=str, required=False)
    parser.add_argument("-info", type=str, required=False)
    args = parser.parse_args()

    asyncio.run(run_plugin(args.port, args.pluginUUID))
```

Make sure the script is executable:
```bash
chmod +x main.py
```
