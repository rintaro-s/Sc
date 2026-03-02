#!/usr/bin/env bash
# CR-Bridge Blender Addon インストーラー
# Usage: ./install_blender_addon.sh [blender_version]

set -e

ADDON_NAME="crb_coordinate_addon"
ADDON_SRC="$(dirname "$0")/crb_coordinate_addon.py"
BLENDER_VER="${1:-4.0}"

# Blender アドオンディレクトリを検索
SEARCH_DIRS=(
  "$HOME/.config/blender/${BLENDER_VER}/scripts/addons"
  "$HOME/Library/Application Support/Blender/${BLENDER_VER}/scripts/addons"
  "/usr/share/blender/${BLENDER_VER}/scripts/addons"
)

ADDON_DIR=""
for d in "${SEARCH_DIRS[@]}"; do
  if [ -d "$d" ]; then
    ADDON_DIR="$d"
    break
  fi
done

if [ -z "$ADDON_DIR" ]; then
  # ディレクトリが見つからなければ作成
  ADDON_DIR="$HOME/.config/blender/${BLENDER_VER}/scripts/addons"
  mkdir -p "$ADDON_DIR"
fi

echo "📦 CR-Bridge Blender Addon インストール"
echo "   → ${ADDON_DIR}/${ADDON_NAME}.py"
cp "$ADDON_SRC" "${ADDON_DIR}/${ADDON_NAME}.py"
echo "✅ インストール完了"
echo ""
echo "Blender を開いて:"
echo "  Edit > Preferences > Add-ons > 「CR-Bridge CAS Sync」を有効化"
