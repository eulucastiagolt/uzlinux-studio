# Claude Code Instructions — Uzlinux Studio

Refer to [AGENT.md](AGENT.md) for complete instructions on autonomously creating, scaffolding, validating, and testing plugins for Uzlinux Studio.

Key Commands:
- Scaffold plugin: `python3 scripts/create_plugin.py --id com.example.action --name "Action Name"`
- Validate plugin: `python3 scripts/validate_plugin.py path/to/plugin.sdPlugin`
- Build project: `cargo build --workspace`
- Test workspace: `cargo test --workspace`
