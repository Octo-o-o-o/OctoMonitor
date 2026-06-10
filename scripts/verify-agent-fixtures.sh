#!/usr/bin/env sh
set -eu

echo "octomonitor-fixture: verify agent fixture contract"
cargo test -p octomonitor-adapter-common agent_fixture_contract_tests -- --nocapture
echo "octomonitor-fixture: verify P0 adapter fixture parsers"
cargo test -p octomonitor-p0-adapters --lib
