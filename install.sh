#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "=========================================================="
echo "    Uzlinux Studio - Instalador de Sistema Automático     "
echo "=========================================================="
echo ""

# 1. Check or build release binary
echo "==> [1/5] Compilando binário de produção (Release)..."
if command -v cargo &>/dev/null; then
    cargo build --release
else
    if [ ! -f "target/release/uzlinux-studio" ]; then
        echo "Erro: cargo não encontrado e o binário target/release/uzlinux-studio não existe."
        exit 1
    fi
fi

# Request sudo privileges once for the installation steps
if [ "$EUID" -ne 0 ]; then
    echo "Privilégios de administrador necessários para instalar no sistema."
    SUDO="sudo"
else
    SUDO=""
fi

# 2. Install executable
echo "==> [2/5] Instalando executável em /usr/local/bin/uzlinux-studio..."
$SUDO install -d /usr/local/bin
$SUDO install -m 755 target/release/uzlinux-studio /usr/local/bin/uzlinux-studio

# 3. Install udev rules & reload
echo "==> [3/5] Configurando regras de hardware Udev em /etc/udev/rules.d/..."
$SUDO install -d /etc/udev/rules.d
$SUDO install -m 644 60-ulanzi-d200.rules /etc/udev/rules.d/60-ulanzi-d200.rules

if command -v udevadm &>/dev/null; then
    echo "    Recarregando e aplicando regras Udev..."
    $SUDO udevadm control --reload-rules || true
    $SUDO udevadm trigger || true
fi

# 4. Install Desktop Launcher
echo "==> [4/5] Instalando atalho de menu em /usr/share/applications/..."
$SUDO install -d /usr/share/applications
$SUDO install -m 644 uzlinux-studio.desktop /usr/share/applications/uzlinux-studio.desktop

# 5. Install App Icon
echo "==> [5/5] Instalando ícone do aplicativo em /usr/share/icons/..."
$SUDO install -d /usr/share/icons/hicolor/scalable/apps
$SUDO install -m 644 assets/uzlinux-studio.svg /usr/share/icons/hicolor/scalable/apps/uzlinux-studio.svg

# Update desktop & icon caches if available
if command -v update-desktop-database &>/dev/null; then
    $SUDO update-desktop-database /usr/share/applications || true
fi
if command -v gtk-update-icon-cache &>/dev/null; then
    $SUDO gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || true
fi

echo ""
echo "=========================================================="
echo "  ✅ Instalação concluída com sucesso!"
echo "  - O aplicativo foi instalado em: /usr/local/bin/uzlinux-studio"
echo "  - As regras Udev USB foram registradas automaticamente"
echo "  - O atalho já está disponível no menu de aplicativos do Linux"
echo "=========================================================="
echo ""
