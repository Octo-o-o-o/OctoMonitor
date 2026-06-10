# Agent Fixture Contract

Agent fixtures lock external tool evidence before an adapter can be promoted.
Every case lives at:

```text
fixtures/agents/<tool>/<version>/<case>/
```

Each case must include:

- `evidence_lock.json`: tool/version/source/provenance and denied-path observations.
- `schema_fingerprint.json`: expected storage format and schema confidence.
- `golden_sessions.json`: normalized sessions the future parser should produce.
- `commands.sh`: replay or provenance commands, safe for local fixture checks.
- `README.md`: short case notes.

Fixtures must be anonymized. They must not contain OAuth tokens, API keys, `.env`
content, provider credentials, or user transcript bodies beyond short synthetic
text. Tools without passing positive and negative fixtures cannot be marked
stable.
