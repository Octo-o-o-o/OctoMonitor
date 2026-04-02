class Octomonitor < Formula
  desc "Local-first unified monitor for Claude Code, Codex & OpenClaw"
  homepage "https://github.com/Octo-o-o-o/OctoMonitor"
  license "MIT"
  version "0.1.2"

  url "https://github.com/Octo-o-o-o/OctoMonitor/archive/refs/tags/v0.1.2.tar.gz"
  sha256 "92655e05478f225239bec40f843befbe2932aefd8dbb3af2854c7f0ac6b4e309"

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
    assert_predicate bin/"octomonitor", :exist?
  end
end
