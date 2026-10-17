#!/usr/bin/env bash
set -e

echo "=== Desinstalador do Uzlinux Studio ==="

if [ "$EUID" -ne 0 ]; then
    echo "Privilégios de administrador necessários. Executando com sudo..."
    exec sudo bash "$0" "$@"
fi

echo "Removendo binário /usr/local/bin/uzlinux-studio..."
rm -f /usr/local/bin/uzlinux-studio

echo "Removendo regra udev /etc/udev/rules.d/60-ulanzi-d200.rules..."
rm -f /etc/udev/rules.d/60-ulanzi-d200.rules
if command -v udevadm &>/dev/null; then
    udevadm control --reload-rules || true
    udevadm trigger || true
fi

echo "Removendo atalho de aplicativo..."
rm -f /usr/share/applications/uzlinux-studio.desktop
rm -f /usr/share/icons/hicolor/scalable/apps/uzlinux-studio.svg

if command -v update-desktop-database &>/dev/null; then
    update-desktop-database /usr/share/applications || true
fi

echo "✅ Uzlinux Studio desinstalado com sucesso."
