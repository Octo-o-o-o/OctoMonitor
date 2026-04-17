#!/usr/bin/env node

const { execFileSync } = require("child_process");

const PLATFORMS = {
  "darwin-arm64": "octomonitor-darwin-arm64",
  "darwin-x64": "octomonitor-darwin-x64",
  "linux-x64": "octomonitor-linux-x64",
  "win32-x64": "@clawbutler/octomonitor-win32-x64",
};

function resolveBinary() {
  const { platform, arch } = process;
  const key = `${platform}-${arch}`;
  const pkg = PLATFORMS[key];

  if (!pkg) {
    console.error(
      `Unsupported platform: ${key}\nSupported: ${Object.keys(PLATFORMS).join(", ")}`
    );
    process.exit(1);
  }

  const binName = platform === "win32" ? "octomonitor-server.exe" : "octomonitor-server";
  try {
    return require.resolve(`${pkg}/bin/${binName}`);
  } catch {
    console.error(
      `Could not find the OctoMonitor binary for ${key}.\n` +
        `Expected package "${pkg}" to be installed.\n` +
        `Try: npm install ${pkg}`
    );
    process.exit(1);
  }
}

try {
  execFileSync(resolveBinary(), process.argv.slice(2), {
    stdio: "inherit",
    env: process.env,
  });
} catch (err) {
  // execFileSync populates `status` with the child's exit code when it exits
  // non-zero; other errors (ENOENT, EACCES, etc.) leave status nullish.
  if (typeof err.status === "number") process.exit(err.status);
  throw err;
}
