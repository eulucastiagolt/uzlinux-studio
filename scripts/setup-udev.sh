#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
RULE_FILE="${PROJECT_ROOT}/60-ulanzi-d200.rules"

if [ ! -f "$RULE_FILE" ]; then
    echo "Erro: Arquivo $RULE_FILE não encontrado."
    exit 1
fi

echo "=========================================================="
echo "  Uzlinux Studio - Instalador de Regras Udev Ulanzi D200"
echo "=========================================================="
echo ""
echo "1. Copiando regra para /etc/udev/rules.d/60-ulanzi-d200.rules..."
sudo cp "$RULE_FILE" /etc/udev/rules.d/60-ulanzi-d200.rules

echo "2. Recarregando e aplicando regras udev..."
sudo udevadm control --reload-rules
sudo udevadm trigger

echo ""
echo "✅ Sucesso! As permissões para o Ulanzi Deck foram configuradas."
echo "💡 Dica: Se o dispositivo já estiver conectado na USB, desconecte e reconecte-o."
echo ""
