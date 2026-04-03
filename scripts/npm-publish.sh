#!/usr/bin/env bash
set -euo pipefail

PACKAGES=(
  "packages/octomonitor-darwin-arm64:octomonitor-server"
  "packages/octomonitor-darwin-x64:octomonitor-server"
  "packages/octomonitor-linux-x64:octomonitor-server"
  "packages/octomonitor-win32-x64:octomonitor-server.exe"
)

for spec in "${PACKAGES[@]}"; do
  IFS=":" read -r pkg bin_name <<< "$spec"
  bin_path="${pkg}/bin/${bin_name}"
  if [ ! -x "$bin_path" ]; then
    if [ "${bin_name}" = "octomonitor-server.exe" ] && [ -f "$bin_path" ]; then
      continue
    fi
    echo "Missing packaged binary: ${bin_path}"
    echo "Build each platform package first, or use the GitHub release workflow to assemble cross-platform npm artifacts."
    exit 1
  fi
done

for spec in "${PACKAGES[@]}"; do
  IFS=":" read -r pkg _ <<< "$spec"
  echo "==> Publishing ${pkg}..."
  (
    cd "$pkg"
    npm publish --access public
  )
done

echo "==> Publishing octomonitor (main package)..."
(
  cd packages/octomonitor
  npm publish --access public
)

echo "Done! Users can now run: npx octomonitor"
