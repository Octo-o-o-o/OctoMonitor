# homebrew-octomonitor

Homebrew tap files for [OctoMonitor](https://github.com/Octo-o-o-o/OctoMonitor).

For product overview, source builds, desktop packaging, and remote viewer setup, see the repository [README](../README.md).

## Install

```bash
brew install Octo-o-o-o/octomonitor/octomonitor
```

Or tap first, then use the short formula name:

```bash
brew tap Octo-o-o-o/octomonitor
brew install octomonitor
```

## Run

```bash
octomonitor
```

Starts the local admin surface on `http://127.0.0.1:46321` and opens it in your browser. The separate read-only remote viewer remains disabled until you enable it from `Settings -> Remote Access`.

## Run as background service

```bash
brew services start octomonitor
```

The service keeps the local server running in the background and suppresses browser auto-open.
