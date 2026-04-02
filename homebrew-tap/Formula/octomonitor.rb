class Octomonitor < Formula
  desc "Local-first unified monitor for Claude Code, Codex & OpenClaw"
  homepage "https://github.com/Octo-o-o-o/OctoMonitor"
  license "MIT"
  version "0.1.2"

  url "https://github.com/Octo-o-o-o/OctoMonitor.git",
      tag: "v0.1.2"

  depends_on "rust" => :build
  depends_on "node" => :build
  depends_on "pnpm" => :build

  def install
    system "pnpm", "install", "--frozen-lockfile"
    system "pnpm", "--filter", "@octomonitor/web", "build"

    system "cargo", "build", "--release", "-p", "octomonitor-server"
    bin.install "target/release/octomonitor-server" => "octomonitor"
  end

  service do
    run [opt_bin/"octomonitor"]
    keep_alive true
    environment_variables OCTOMONITOR_NO_OPEN: "1"
    log_path var/"log/octomonitor.log"
    error_log_path var/"log/octomonitor.log"
  end

  test do
    assert_match "octomonitor", (bin/"octomonitor").to_s
  end
end
