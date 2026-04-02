#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
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

APPLE_PASSWORD_VALUE="${APPLE_PASSWORD:-${APPLE_APP_SPECIFIC_PASSWORD:-}}"
APPLE_ID_VALUE="${APPLE_ID:-}"
APPLE_TEAM_ID_VALUE="${APPLE_TEAM_ID:-}"
KEYCHAIN_PROFILE="${OCTOMONITOR_NOTARY_PROFILE:-octomonitor-notary}"

if [[ -z "$APPLE_ID_VALUE" ]]; then
  echo "APPLE_ID is required for notarization." >&2
  exit 1
fi

if [[ -z "$APPLE_TEAM_ID_VALUE" ]]; then
  echo "APPLE_TEAM_ID is required for notarization." >&2
  exit 1
fi

if [[ -z "$APPLE_PASSWORD_VALUE" ]]; then
  echo "Set APPLE_PASSWORD or APPLE_APP_SPECIFIC_PASSWORD before notarizing." >&2
  exit 1
fi

APP_PATH="$ROOT_DIR/target/release/bundle/macos/$PRODUCT_NAME.app"
ZIP_PATH="$ROOT_DIR/target/release/bundle/macos/$PRODUCT_NAME-notarize.zip"
DMG_PATH="$ROOT_DIR/target/release/bundle/dmg/${PRODUCT_NAME}_${VERSION}_${BUNDLE_ARCH}.dmg"
DMG_STAGE_DIR="$(mktemp -d "$ROOT_DIR/target/release/bundle/dmg/${PRODUCT_NAME}.stage.XXXXXX")"

cleanup() {
  rm -rf "$DMG_STAGE_DIR"
}

trap cleanup EXIT

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

submit_with_retry() {
  local artifact_path="$1"
  local attempt=1
  local max_attempts=3

  while (( attempt <= max_attempts )); do
    if xcrun notarytool submit "$artifact_path" --keychain-profile "$KEYCHAIN_PROFILE" --wait; then
      return 0
    fi

    if (( attempt == max_attempts )); then
      echo "Notarization failed after ${max_attempts} attempts: $artifact_path" >&2
      return 1
    fi

    sleep $(( attempt * 15 ))
    attempt=$(( attempt + 1 ))
  done
}

staple_with_retry() {
  local artifact_path="$1"
  local attempt=1
  local max_attempts=4

  while (( attempt <= max_attempts )); do
    if xcrun stapler staple -v "$artifact_path" && xcrun stapler validate -v "$artifact_path"; then
      return 0
    fi

    if (( attempt == max_attempts )); then
      echo "Stapler failed after ${max_attempts} attempts: $artifact_path" >&2
      return 1
    fi

    sleep $(( attempt * 20 ))
    attempt=$(( attempt + 1 ))
  done
}

xcrun notarytool store-credentials "$KEYCHAIN_PROFILE" \
  --apple-id "$APPLE_ID_VALUE" \
  --password "$APPLE_PASSWORD_VALUE" \
  --team-id "$APPLE_TEAM_ID_VALUE" \
  --no-validate

bash "$ROOT_DIR/scripts/build-macos-signed.sh" --bundles app

rm -f "$ZIP_PATH"
ditto -c -k --keepParent "$APP_PATH" "$ZIP_PATH"
submit_with_retry "$ZIP_PATH"
staple_with_retry "$APP_PATH"

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
submit_with_retry "$DMG_PATH"
staple_with_retry "$DMG_PATH"

codesign --verify --deep --strict --verbose=2 "$APP_PATH"
spctl -a -t exec -vv "$APP_PATH"
spctl -a -t open --context context:primary-signature -vv "$DMG_PATH"

echo "Notarized artifacts:"
echo "  $APP_PATH"
echo "  $DMG_PATH"
