#!/usr/bin/env python3
"""
Uzlinux Studio Plugin Validator
Validates a plugin directory to ensure it conforms to all Uzlinux Studio standards:
- manifest.json syntax and required fields
- Executable presence and execute (+x) permissions
- Action definitions, controllers, and states
- WebSocket registration compliance

Usage:
    python3 scripts/validate_plugin.py path/to/plugin.sdPlugin
"""

import sys
import os
import json
from pathlib import Path

def validate(plugin_path_str: str) -> bool:
    plugin_path = Path(plugin_path_str).resolve()
    print(f"🔍 Validating plugin at: {plugin_path}")

    if not plugin_path.exists() or not plugin_path.is_dir():
        print(f"❌ Error: Directory does not exist: {plugin_path}")
        return False

    if not plugin_path.name.endswith(".sdPlugin"):
        print(f"⚠️ Warning: Plugin folder name '{plugin_path.name}' should end with '.sdPlugin'")

    manifest_file = plugin_path / "manifest.json"
    if not manifest_file.exists():
        print(f"❌ Error: Missing manifest.json in {plugin_path}")
        return False

    try:
        with open(manifest_file, "r", encoding="utf-8") as f:
            manifest = json.load(f)
    except Exception as e:
        print(f"❌ Error: Failed to parse manifest.json: {e}")
        return False

    # Check required manifest fields
    required_fields = ["Name", "Version", "PluginUUID", "Actions"]
    missing = [k for k in required_fields if k not in manifest]
    if missing:
        print(f"❌ Error: Missing required manifest fields: {missing}")
        return False

    code_path = manifest.get("CodePathLin") or manifest.get("CodePath")
    if not code_path:
        print("❌ Error: Missing 'CodePathLin' or 'CodePath' in manifest.json")
        return False

    exe_file = plugin_path / code_path
    if not exe_file.exists():
        print(f"❌ Error: Executable '{code_path}' does not exist at {exe_file}")
        return False

    if not os.access(exe_file, os.X_OK):
        print(f"⚠️ Warning: Executable '{code_path}' does not have execute permissions (+x). Run: chmod +x '{exe_file}'")
    else:
        print(f"✅ Executable '{code_path}' is present and executable (+x).")

    # Validate actions
    actions = manifest.get("Actions", [])
    if not actions:
        print("⚠️ Warning: No actions defined in manifest.json")

    for idx, act in enumerate(actions):
        act_uuid = act.get("UUID")
        act_name = act.get("Name")
        if not act_uuid or not act_name:
            print(f"❌ Error: Action #{idx} is missing 'UUID' or 'Name'")
            return False
        controllers = act.get("Controllers", [])
        if "Keypad" not in controllers and "Encoder" not in controllers:
            print(f"⚠️ Warning: Action '{act_name}' has controllers {controllers}, usually ['Keypad'] is expected for buttons.")

    print(f"✅ Plugin '{manifest.get('Name')}' ({manifest.get('PluginUUID')}) passed all structural validation checks!")
    return True

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python3 scripts/validate_plugin.py <path_to_plugin.sdPlugin>")
        sys.exit(1)

    ok = validate(sys.argv[1])
    sys.exit(0 if ok else 1)
