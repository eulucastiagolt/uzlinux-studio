#!/usr/bin/env python3
"""
Uzlinux Studio Plugin Scaffolder
Creates a ready-to-run, fully compliant plugin structure for Uzlinux Studio.
Usage:
    python3 scripts/create_plugin.py --id com.example.myaction --name "My Action" --author "Author"
"""

import os
import sys
import argparse
import json
from pathlib import Path

PYTHON_TEMPLATE = '''#!/usr/bin/env python3
"""
{name} - Uzlinux Studio Plugin
Author: {author}
UUID: {uuid}
"""

import sys
import json
import argparse
import asyncio
import logging

try:
    import websockets
except ImportError:
    print("Error: 'websockets' library is required. Install via: pip3 install websockets", file=sys.stderr)
    sys.exit(1)

logging.basicConfig(level=logging.INFO, format="[%(asctime)s] %(levelname)s: %(message)s")
logger = logging.getLogger("{uuid}")

active_contexts = set()

async def run(port: int, plugin_uuid: str):
    uri = f"ws://127.0.0.1:{{port}}"
    logger.info(f"Connecting to Uzlinux Studio at {{uri}}...")

    while True:
        try:
            async with websockets.connect(uri) as ws:
                logger.info(f"Connected to WebSocket server. Registering plugin {{plugin_uuid}}...")
                # 1. Mandatory registration handshake
                await ws.send(json.dumps({{
                    "event": "registerPlugin",
                    "uuid": plugin_uuid
                }}))

                # Background task to refresh button state/data
                update_task = asyncio.create_task(background_worker(ws))

                # 2. Event loop processing incoming messages
                async for raw_message in ws:
                    try:
                        msg = json.loads(raw_message)
                    except json.JSONDecodeError:
                        continue

                    event = msg.get("event")
                    context = msg.get("context")
                    payload = msg.get("payload", {{}})

                    if event == "willAppear":
                        logger.info(f"Button appeared: context={{context}}")
                        active_contexts.add(context)
                        # Initialize button title or icon
                        await ws.send(json.dumps({{
                            "event": "setTitle",
                            "context": context,
                            "payload": {{ "title": "{short_title}\\nReady" }}
                        }}))

                    elif event == "willDisappear":
                        logger.info(f"Button disappeared: context={{context}}")
                        active_contexts.discard(context)

                    elif event == "keyDown":
                        logger.info(f"Key pressed: context={{context}}")
                        # Perform your action here (e.g. HTTP request, shell command, toggle state)
                        await ws.send(json.dumps({{
                            "event": "setTitle",
                            "context": context,
                            "payload": {{ "title": "{short_title}\\nActive!" }}
                        }}))

                    elif event == "keyUp":
                        await ws.send(json.dumps({{
                            "event": "setTitle",
                            "context": context,
                            "payload": {{ "title": "{short_title}\\nReady" }}
                        }}))

                    elif event == "didReceiveSettings":
                        logger.info(f"Settings updated for {{context}}: {{payload}}")

                update_task.cancel()

        except (websockets.exceptions.ConnectionClosedError, OSError) as e:
            logger.warning(f"Connection lost ({{e}}). Retrying in 2 seconds...")
            await asyncio.sleep(2)

async def background_worker(ws):
    counter = 0
    while True:
        await asyncio.sleep(3)
        counter += 1
        for ctx in list(active_contexts):
            try:
                await ws.send(json.dumps({{
                    "event": "setTitle",
                    "context": ctx,
                    "payload": {{ "title": f"{short_title}\\n#{{counter}}" }}
                }}))
            except Exception:
                pass

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="{name} Plugin for Uzlinux Studio")
    parser.add_argument("-port", type=int, required=True, help="WebSocket port")
    parser.add_argument("-pluginUUID", type=str, required=True, help="Plugin UUID")
    parser.add_argument("-registerEvent", type=str, default="registerPlugin")
    parser.add_argument("-info", type=str, default="{{}}")
    args = parser.parse_args()

    try:
        asyncio.run(run(args.port, args.pluginUUID))
    except KeyboardInterrupt:
        logger.info("Plugin terminated by user.")
'''

HTML_PI_TEMPLATE = '''<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>{name} Settings</title>
  <style>
    body {{
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      font-size: 12px;
      color: #cdd6f4;
      background: #11111b;
      margin: 0;
      padding: 12px;
    }}
    .group {{
      margin-bottom: 12px;
    }}
    label {{
      display: block;
      margin-bottom: 4px;
      font-weight: 600;
    }}
    input, select {{
      width: 100%;
      box-sizing: border-box;
      padding: 6px 8px;
      background: #1e1e2e;
      border: 1px solid #313244;
      color: #fff;
      border-radius: 4px;
      font-size: 12px;
    }}
    input:focus {{
      border-color: #89b4fa;
      outline: none;
    }}
    .hint {{
      font-size: 11px;
      color: #a6adc8;
      margin-top: 4px;
    }}
  </style>
</head>
<body>
  <div class="group">
    <label for="custom-param">Custom Parameter:</label>
    <input type="text" id="custom-param" placeholder="e.g. interval or URL">
    <div class="hint">Settings are automatically synchronized with Uzlinux Studio.</div>
  </div>

  <script>
    let websocket = null;
    let pluginUUID = null;
    let actionInfo = {{}};

    // Function called by Uzlinux Studio property inspector container
    function connectElgatoStreamDeckSocket(inPort, inPluginUUID, inRegisterEvent, inInfo, inActionInfo) {{
      pluginUUID = inPluginUUID;
      actionInfo = inActionInfo ? JSON.parse(inActionInfo) : {{}};

      websocket = new WebSocket("ws://127.0.0.1:" + inPort);

      websocket.onopen = function () {{
        websocket.send(JSON.stringify({{
          event: inRegisterEvent,
          uuid: inPluginUUID
        }}));

        // Restore settings if present
        if (actionInfo.payload && actionInfo.payload.settings) {{
          const val = actionInfo.payload.settings.customParam;
          if (val) document.getElementById('custom-param').value = val;
        }}
      }};

      websocket.onmessage = function (evt) {{
        const jsonObj = JSON.parse(evt.data);
        if (jsonObj.event === "didReceiveSettings") {{
          const s = jsonObj.payload.settings || {{}};
          if (s.customParam) document.getElementById('custom-param').value = s.customParam;
        }}
      }};
    }}

    document.getElementById('custom-param').addEventListener('change', function (e) {{
      if (websocket && actionInfo.context) {{
        const payload = {{ customParam: e.target.value }};
        websocket.send(JSON.stringify({{
          event: "setSettings",
          context: actionInfo.context,
          payload: payload
        }}));

        // Sync with parent window if inside iframe
        window.parent.postMessage({{
          event: "setSettings",
          context: actionInfo.context,
          payload: payload
        }}, "*");
      }}
    }});
  </script>
</body>
</html>
'''

def create_svg_icon(path: Path, label: str):
    svg_content = f'''<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72" viewBox="0 0 72 72">
  <rect width="72" height="72" rx="14" fill="#1e1e2e"/>
  <circle cx="36" cy="36" r="24" fill="#313244"/>
  <text x="36" y="42" font-family="sans-serif" font-size="20" font-weight="bold" fill="#89b4fa" text-anchor="middle">{label[:3].upper()}</text>
</svg>'''
    with open(path, "w", encoding="utf-8") as f:
        f.write(svg_content)

def main():
    parser = argparse.ArgumentParser(description="Create a new Uzlinux Studio Plugin")
    parser.add_argument("--id", required=True, help="Plugin UUID, e.g. com.example.myaction")
    parser.add_argument("--name", required=True, help="Human-readable plugin name")
    parser.add_argument("--author", default="Uzlinux Creator", help="Author name")
    parser.add_argument("--category", default="Custom Plugins", help="Action category in palette")
    parser.add_argument("--output-dir", default=None, help="Output directory (defaults to ~/.config/uzlinux-studio/plugins/)")
    args = parser.parse_args()

    uuid = args.id
    if not uuid.endswith(".sdPlugin"):
        folder_name = f"{uuid}.sdPlugin"
    else:
        folder_name = uuid
        uuid = uuid[:-9]

    if args.output_dir:
        base_dir = Path(args.output_dir)
    else:
        base_dir = Path.home() / ".config" / "uzlinux-studio" / "plugins"

    target_dir = base_dir / folder_name
    target_dir.mkdir(parents=True, exist_ok=True)
    assets_dir = target_dir / "assets"
    assets_dir.mkdir(exist_ok=True)
    pi_dir = target_dir / "property_inspector"
    pi_dir.mkdir(exist_ok=True)

    action_uuid = f"{uuid}.action"
    short_title = args.name.split()[0]

    # 1. Create manifest.json
    manifest = {
        "Name": args.name,
        "Description": f"{args.name} for Uzlinux Studio",
        "Author": args.author,
        "Version": "1.0.0",
        "PluginUUID": uuid,
        "CodePathLin": "main.py",
        "Icon": "assets/icon.svg",
        "Category": args.category,
        "OS": [
            {"Platform": "linux"}
        ],
        "Actions": [
            {
                "UUID": action_uuid,
                "Name": args.name,
                "Tooltip": f"Triggers {args.name}",
                "Icon": "assets/action.svg",
                "Controllers": ["Keypad"],
                "PropertyInspectorPath": "property_inspector/pi.html",
                "States": [
                    {
                        "Image": "assets/action.svg",
                        "Title": short_title,
                        "TitleColor": "#ffffff",
                        "TitleAlignment": "middle",
                        "FontSize": 12,
                        "ShowTitle": True
                    }
                ]
            }
        ]
    }

    with open(target_dir / "manifest.json", "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2)

    # 2. Create main.py
    main_py_content = PYTHON_TEMPLATE.format(
        name=args.name,
        author=args.author,
        uuid=uuid,
        short_title=short_title
    )
    main_file = target_dir / "main.py"
    with open(main_file, "w", encoding="utf-8") as f:
        f.write(main_py_content)
    main_file.chmod(0o755)

    # 3. Create property inspector
    pi_html = HTML_PI_TEMPLATE.format(name=args.name)
    with open(pi_dir / "pi.html", "w", encoding="utf-8") as f:
        f.write(pi_html)

    # 4. Create default vector icons
    create_svg_icon(assets_dir / "icon.svg", short_title)
    create_svg_icon(assets_dir / "action.svg", short_title)

    print(f"✅ Plugin successfully created at: {target_dir}")
    print(f"📦 UUID: {uuid}")
    print(f"⚡ Action: {action_uuid}")
    print(f"🚀 Executable: {main_file} (chmod +x set)")
    print("\nTo test, start or restart Uzlinux Studio and the action will appear in the library.")

if __name__ == "__main__":
    main()
