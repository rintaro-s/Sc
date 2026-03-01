#!/usr/bin/env bash
# ============================================================
#  CR-Bridge インストーラー
#  Usage: ./install.sh [--demo-only] [--uninstall]
# ============================================================
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_PREFIX="${CR_BRIDGE_PREFIX:-$HOME/.local}"
BIN_DIR="$INSTALL_PREFIX/bin"
LIB_DIR="$INSTALL_PREFIX/lib/cr-bridge"
LOG_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/cr-bridge"
SYSTEMD_USER_DIR="$HOME/.config/systemd/user"

# カラー出力
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

banner() {
cat << 'EOF'
  ____  ____       ____       _     _
 / ___|  _ \      | __ ) _ __(_) __| | __ _  ___
| |   | |_) |_____|  _ \| '__| |/ _` |/ _` |/ _ \
| |___|  _ <______| |_) | |  | | (_| | (_| |  __/
 \____|_| \_\     |____/|_|  |_|\__,_|\__, |\___|
                                       |___/
 クロスリアリティ低レイヤー基盤ミドルウェア  v0.1.0
EOF
}

log_info()  { echo -e "${CYAN}[INFO]${NC}  $*"; }
log_ok()    { echo -e "${GREEN}[OK]${NC}    $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }
log_step()  { echo -e "\n${BOLD}▶ $*${NC}"; }

# ====== 引数処理 ======
DEMO_ONLY=false
UNINSTALL=false
for arg in "$@"; do
  case "$arg" in
    --demo-only) DEMO_ONLY=true ;;
    --uninstall) UNINSTALL=true ;;
    --help|-h)
      echo "Usage: $0 [--demo-only] [--uninstall]"
      echo "  --demo-only   デモアプリのみインストール"
      echo "  --uninstall   CR-Bridge を削除"
      exit 0 ;;
  esac
done

# ====== アンインストール ======
if $UNINSTALL; then
  log_step "CR-Bridge をアンインストールしています..."
  systemctl --user stop cr-bridge-daemon.service 2>/dev/null || true
  systemctl --user disable cr-bridge-daemon.service 2>/dev/null || true
  rm -f "$SYSTEMD_USER_DIR/cr-bridge-daemon.service"
  rm -f "$BIN_DIR/cr-bridge-daemon" "$BIN_DIR/cr-bridge-demo"
  rm -rf "$LIB_DIR"
  log_ok "アンインストール完了"
  exit 0
fi

# ====== バナー表示 ======
banner
echo ""
log_info "インストール先: $INSTALL_PREFIX"
log_info "リポジトリ:     $REPO_DIR"

# ====== 依存チェック ======
log_step "依存関係を確認しています..."

check_cmd() {
  if command -v "$1" &>/dev/null; then
    log_ok "$1 が見つかりました: $(command -v "$1")"
    return 0
  else
    return 1
  fi
}

# Rust / Cargo
if ! check_cmd rustc; then
  log_warn "Rust が見つかりません。インストールします..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
  source "$HOME/.cargo/env"
fi

if ! check_cmd cargo; then
  log_error "Cargo が見つかりません。Rust を再インストールしてください。"
  exit 1
fi

RUST_VERSION=$(rustc --version | awk '{print $2}')
log_ok "Rust $RUST_VERSION"

# Rust バージョン確認（1.75+が必要）
RUST_MAJOR=$(echo "$RUST_VERSION" | cut -d. -f1)
RUST_MINOR=$(echo "$RUST_VERSION" | cut -d. -f2)
if [[ "$RUST_MAJOR" -lt 1 ]] || ( [[ "$RUST_MAJOR" -eq 1 ]] && [[ "$RUST_MINOR" -lt 75 ]] ); then
  log_warn "Rust 1.75+ が必要です（現在: $RUST_VERSION）。アップデートします..."
  rustup update stable
fi

# システムパッケージ確認
for pkg in pkg-config; do
  if ! check_cmd "$pkg"; then
    log_warn "$pkg が見つかりません"
    if command -v apt-get &>/dev/null; then
      sudo apt-get install -y "$pkg" 2>/dev/null || log_warn "$pkg のインストールをスキップしました"
    fi
  fi
done

# ====== ディレクトリ作成 ======
log_step "ディレクトリを作成しています..."
mkdir -p "$BIN_DIR" "$LIB_DIR" "$LOG_DIR"
log_ok "ディレクトリ作成完了"

# ====== ビルド ======
log_step "CR-Bridge をビルドしています..."
cd "$REPO_DIR"

BUILD_FEATURES=""
# AVX-512 サポート確認
if grep -q "avx512f" /proc/cpuinfo 2>/dev/null; then
  log_ok "AVX-512 対応CPU が検出されました。SIMD最適化を有効化します"
  export RUSTFLAGS="-C target-feature=+avx512f ${RUSTFLAGS:-}"
else
  log_info "AVX-512 非対応CPU。スカラーフォールバックを使用します"
fi

if $DEMO_ONLY; then
  log_info "デモのみビルドします..."
  cargo build --release -p demo-metaverse
else
  log_info "全コンポーネントをビルドします..."
  cargo build --release
fi

log_ok "ビルド完了"

# ====== インストール ======
log_step "バイナリをインストールしています..."

if ! $DEMO_ONLY; then
  if [[ -f "$REPO_DIR/target/release/cr-bridge-daemon" ]]; then
    cp "$REPO_DIR/target/release/cr-bridge-daemon" "$BIN_DIR/"
    chmod +x "$BIN_DIR/cr-bridge-daemon"
    log_ok "cr-bridge-daemon → $BIN_DIR/cr-bridge-daemon"
  fi
fi

if [[ -f "$REPO_DIR/target/release/demo-metaverse" ]]; then
  cp "$REPO_DIR/target/release/demo-metaverse" "$BIN_DIR/cr-bridge-demo"
  chmod +x "$BIN_DIR/cr-bridge-demo"
  log_ok "demo-metaverse → $BIN_DIR/cr-bridge-demo"
fi

# デモの静的ファイルをコピー
if [[ -d "$REPO_DIR/demo-metaverse/static" ]]; then
  cp -r "$REPO_DIR/demo-metaverse/static" "$LIB_DIR/"
  log_ok "静的ファイル → $LIB_DIR/static"
fi

# ====== systemd ユーザーサービス（オプション） ======
if ! $DEMO_ONLY && command -v systemctl &>/dev/null; then
  log_step "systemd ユーザーサービスを設定しています..."
  mkdir -p "$SYSTEMD_USER_DIR"
  cat > "$SYSTEMD_USER_DIR/cr-bridge-daemon.service" << SERVICE
[Unit]
Description=CR-Bridge Anti-Teleport Middleware Daemon
After=network.target

[Service]
Type=simple
ExecStart=$BIN_DIR/cr-bridge-daemon
Restart=on-failure
RestartSec=5
StandardOutput=append:$LOG_DIR/daemon.log
StandardError=append:$LOG_DIR/daemon.error.log
Environment="RUST_LOG=info"

[Install]
WantedBy=default.target
SERVICE

  systemctl --user daemon-reload
  log_ok "systemd サービスを設定しました"
  log_info "起動: systemctl --user start cr-bridge-daemon"
  log_info "自動起動: systemctl --user enable cr-bridge-daemon"
fi

# ====== PATH 設定 ======
log_step "PATH を確認しています..."
if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
  log_warn "$BIN_DIR が PATH に含まれていません"
  SHELL_RC=""
  if [[ -f "$HOME/.zshrc" ]]; then
    SHELL_RC="$HOME/.zshrc"
  elif [[ -f "$HOME/.bashrc" ]]; then
    SHELL_RC="$HOME/.bashrc"
  fi

  if [[ -n "$SHELL_RC" ]]; then
    echo "" >> "$SHELL_RC"
    echo "# CR-Bridge" >> "$SHELL_RC"
    echo "export PATH=\"$BIN_DIR:\$PATH\"" >> "$SHELL_RC"
    log_ok "PATH を $SHELL_RC に追記しました"
    export PATH="$BIN_DIR:$PATH"
  fi
fi

# ====== 完了メッセージ ======
echo ""
echo -e "${GREEN}${BOLD}=====================================${NC}"
echo -e "${GREEN}${BOLD}  CR-Bridge インストール完了！       ${NC}"
echo -e "${GREEN}${BOLD}=====================================${NC}"
echo ""
echo -e "  ${BOLD}デモを起動するには:${NC}"
echo -e "  ${CYAN}  cr-bridge-demo${NC}"
echo -e "  ブラウザで ${CYAN}http://localhost:3000${NC} を開いてください"
echo ""
if ! $DEMO_ONLY; then
  echo -e "  ${BOLD}デーモンを起動するには:${NC}"
  echo -e "  ${CYAN}  cr-bridge-daemon${NC}"
  echo ""
fi
echo -e "  ${BOLD}ログ:${NC} $LOG_DIR/"
echo ""

# パスを即時反映
if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
  echo -e "${YELLOW}  ヒント: 新しいシェルを開くか以下を実行してください:${NC}"
  echo -e "  ${CYAN}  export PATH=\"$BIN_DIR:\$PATH\"${NC}"
fi
