#!/usr/bin/env bash
set -euo pipefail

PACKAGES=(
  "packages/octomonitor-darwin-arm64"
  "packages/octomonitor-darwin-x64"
  "packages/octomonitor-linux-x64"
)

for pkg in "${PACKAGES[@]}"; do
  bin_path="${pkg}/bin/octomonitor-server"
  if [ ! -x "$bin_path" ]; then
    echo "Missing packaged binary: ${bin_path}"
    echo "Build each platform package first, or use the GitHub release workflow to assemble cross-platform npm artifacts."
    exit 1
  fi
done

for pkg in "${PACKAGES[@]}"; do
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
