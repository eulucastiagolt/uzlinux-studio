# AI Agent & Plugin Creation Guide — Uzlinux Studio

> **Connect any AI Agent to automatically build, test, and install plugins for your deck!**  
> Uzlinux Studio provides native support for autonomous AI coding agents (Antigravity, Claude Code, Cursor, GitHub Copilot, Roo Code, Aider, etc.) through structured instructions (`AGENT.md`), Agent Skills (`skills/uzlinux-plugin-creator`), and automated scaffolding tools.

---

## 🤖 1. Connecting an AI Agent to Build Plugins

Instead of manually writing code or copying and pasting snippets, you can have your AI agent autonomously scaffold, program, validate, and install plugins directly into Uzlinux Studio.

### Option A: Antigravity / Gemini Agents (Agent Skill)
Uzlinux Studio includes a dedicated skill at [`skills/uzlinux-plugin-creator/SKILL.md`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/skills/uzlinux-plugin-creator/SKILL.md).

When running an agent in this workspace or with the skill activated, simply tell the AI:
> *"Create a plugin for Uzlinux Studio that queries the current weather in London and updates the button every 10 minutes."*

The agent will:
1. Run `python3 scripts/create_plugin.py` to generate the plugin package.
2. Implement the API logic and error handling in `main.py`.
3. Validate the package with `python3 scripts/validate_plugin.py`.
4. Install it into `~/.config/uzlinux-studio/plugins/`.

---

### Option B: Claude Code / Cursor / Windsurf / Copilot (`AGENT.md` & `CLAUDE.md`)
The repository contains universal agent instructions in [`AGENT.md`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/AGENT.md) and [`CLAUDE.md`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/CLAUDE.md).

When opening the project in **Cursor**, **Windsurf**, or running **Claude Code** (`claude` in terminal), the agent automatically reads `AGENT.md` and gains full understanding of:
- The WebSocket communication protocol.
- The `manifest.json` schema.
- Built-in scaffolding tools.
- Installation paths.

Just ask:
> *"Build and install a plugin called CryptoTicker that shows BTC and ETH prices on key press."*

---

### Option C: Standalone Command-Line Tool (`scripts/create_plugin.py`)
You or any script/agent can generate a ready-to-run plugin in 1 second:

```bash
python3 scripts/create_plugin.py \
  --id com.myname.myaction \
  --name "My Action Name" \
  --author "Your Name" \
  --category "Custom Tools"
```

This immediately creates a fully configured plugin in:
```bash
~/.config/uzlinux-studio/plugins/com.myname.myaction.sdPlugin/
├── manifest.json
├── main.py (chmod +x)
├── assets/ (icon.svg, action.svg)
└── property_inspector/ (pi.html)
```

---

### Option D: Web AIs (ChatGPT, Claude Web, Gemini Web)
If you are using a web chat without file access, provide the contents of [`AGENT.md`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/AGENT.md) as the system prompt or project instruction.

---

## 🛠️ 2. Validating Your Plugin

To verify that any plugin is 100% compliant before opening Uzlinux Studio:

```bash
python3 scripts/validate_plugin.py ~/.config/uzlinux-studio/plugins/com.myname.myaction.sdPlugin
```

Checks performed:
- [x] Folder naming convention (`.sdPlugin`).
- [x] `manifest.json` syntax and mandatory fields (`Name`, `PluginUUID`, `Actions`, `CodePathLin`).
- [x] Executable permissions (`chmod +x`).
- [x] Valid controllers (`Keypad` / `Encoder`).

---

## 📂 3. Useful Links

- 🛠️ [Universal Agent Prompt & Directives (`AGENT.md`)](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/AGENT.md)
- 🧠 [Plugin Creator Agent Skill (`skills/uzlinux-plugin-creator/SKILL.md`)](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/skills/uzlinux-plugin-creator/SKILL.md)
- 📘 [Technical Plugin Development Specification](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/docs/PLUGIN_DEVELOPMENT_GUIDE.md)
- 🚀 [Scaffolder Script (`scripts/create_plugin.py`)](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/scripts/create_plugin.py)
- 🔍 [Validator Script (`scripts/validate_plugin.py`)](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/scripts/validate_plugin.py)
