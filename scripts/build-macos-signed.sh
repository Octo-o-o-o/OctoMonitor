#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DESKTOP_DIR="$ROOT_DIR/apps/desktop"
PRODUCT_NAME="$(ROOT_DIR="$ROOT_DIR" python3 - <<'PY'
import json
import os
from pathlib import Path
config = json.loads(Path(os.environ["ROOT_DIR"], "apps/desktop/src-tauri/tauri.conf.json").read_text())
print(config["productName"])
PY
)"
SIGNING_IDENTITY="$(ROOT_DIR="$ROOT_DIR" python3 - <<'PY'
import json
import os
from pathlib import Path
config = json.loads(Path(os.environ["ROOT_DIR"], "apps/desktop/src-tauri/tauri.conf.json").read_text())
print(config["bundle"]["macOS"]["signingIdentity"])
PY
)"
VERSION="$(node -p "require('$ROOT_DIR/apps/desktop/package.json').version")"

case "$(uname -m)" in
  arm64)
    BUNDLE_ARCH="aarch64"
    ;;
  x86_64)
    BUNDLE_ARCH="x64"
    ;;
  *)
    BUNDLE_ARCH="$(uname -m)"
    ;;
esac

APP_PATH="$ROOT_DIR/target/release/bundle/macos/$PRODUCT_NAME.app"
SERVER_PATH="$APP_PATH/Contents/Resources/octomonitor-server"
DMG_PATH="$ROOT_DIR/target/release/bundle/dmg/${PRODUCT_NAME}_${VERSION}_${BUNDLE_ARCH}.dmg"

codesign_with_retry() {
  local target_path="$1"
  shift
  local attempt=1
  local max_attempts="${OCTOMONITOR_CODESIGN_ATTEMPTS:-3}"

  while (( attempt <= max_attempts )); do
    if codesign --force --sign "$SIGNING_IDENTITY" --timestamp "$@" "$target_path"; then
      return 0
    fi

    if (( attempt == max_attempts )); then
      echo "codesign failed after ${max_attempts} attempts: $target_path" >&2
      return 1
    fi

    sleep $(( attempt * 15 ))
    attempt=$(( attempt + 1 ))
  done
}

cd "$DESKTOP_DIR"

pnpm run build:web
pnpm run build:server
pnpm run prepare:bundle-resources

attempt=1
max_attempts="${OCTOMONITOR_SIGN_ATTEMPTS:-3}"

while (( attempt <= max_attempts )); do
  if env \
    -u APPLE_ID \
    -u APPLE_PASSWORD \
    -u APPLE_APP_SPECIFIC_PASSWORD \
    -u APPLE_TEAM_ID \
    CI=true \
    pnpm exec tauri build --config src-tauri/tauri.conf.json "$@"; then
    if [[ -f "$SERVER_PATH" ]]; then
      codesign_with_retry "$SERVER_PATH" --options runtime
    fi

    codesign_with_retry "$APP_PATH" --options runtime

    if [[ -f "$DMG_PATH" ]]; then
      DMG_STAGE_DIR="$(mktemp -d "$ROOT_DIR/target/release/bundle/dmg/${PRODUCT_NAME}.signed.XXXXXX")"
      trap 'rm -rf "$DMG_STAGE_DIR"' EXIT
      rm -f "$DMG_PATH"
      cp -R "$APP_PATH" "$DMG_STAGE_DIR/"
      ln -s /Applications "$DMG_STAGE_DIR/Applications"
      hdiutil create \
        -volname "$PRODUCT_NAME" \
        -srcfolder "$DMG_STAGE_DIR" \
        -ov \
        -format UDZO \
        "$DMG_PATH"
      codesign_with_retry "$DMG_PATH"
      rm -rf "$DMG_STAGE_DIR"
      trap - EXIT
    fi

    exit 0
  fi

  if (( attempt == max_attempts )); then
    echo "Signing build failed after ${max_attempts} attempts." >&2
    exit 1
  fi

  sleep $(( attempt * 15 ))
  attempt=$(( attempt + 1 ))
done
