#!/usr/bin/env node

const { execFileSync } = require("child_process");
const path = require("path");
const os = require("os");

const PLATFORMS = {
  "darwin-arm64": "octomonitor-darwin-arm64",
  "darwin-x64": "octomonitor-darwin-x64",
  "linux-x64": "octomonitor-linux-x64",
};

function getBinaryPath() {
  const key = `${os.platform()}-${os.arch()}`;
  const pkg = PLATFORMS[key];
  if (!pkg) {
    console.error(
      `Unsupported platform: ${key}\nSupported: ${Object.keys(PLATFORMS).join(", ")}`
    );
    process.exit(1);
  }

  try {
    return require.resolve(`${pkg}/bin/octomonitor-server`);
  } catch {
    console.error(
      `Could not find the OctoMonitor binary for ${key}.\n` +
        `Expected package "${pkg}" to be installed.\n` +
        `Try: npm install ${pkg}`
    );
    process.exit(1);
  }
}

const bin = getBinaryPath();

try {
  execFileSync(bin, process.argv.slice(2), {
    stdio: "inherit",
    env: process.env,
  });
} catch (e) {
  if (e.status !== null) process.exit(e.status);
  throw e;
}
