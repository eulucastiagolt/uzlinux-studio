#!/usr/bin/env bash
# Uzlinux Studio - Live Reload / Watch Script
# Reinicia e recompila o aplicativo automaticamente ao alterar arquivos .rs, .slint ou .toml

set -e

echo "🚀 Iniciando Uzlinux Studio no modo Watch (Auto-Reload)..."
echo "📂 Monitorando alterações em src/, ui/, Cargo.toml e build.rs..."
echo "Pressione Ctrl+C para encerrar."
echo ""

cargo watch -c -w src -w ui -w Cargo.toml -w build.rs -x run
