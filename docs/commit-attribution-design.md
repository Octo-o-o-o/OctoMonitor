# Commit Attribution Design

## Goal

Associate Git commits with Claude Code, Codex, and OpenClaw sessions so OctoMonitor can answer:

- Which sessions contributed to this commit?
- How many tokens and how much cost were allocated to this commit?
- Which source tools contributed, and with what confidence?
- How does this work across multiple git worktrees?

This document captures:

- Research findings from official docs and community tools
- The recommended long-term architecture
- The MVP implementation boundary for OctoMonitor right now
- Schema and API design
- Frontend display proposal

## Constraints

- Local-first
- No database by default
- Read-only by default
- Gateway/official APIs beat derived file scans
- Never expose secrets or raw tokens
- Work across multiple repos and multiple worktrees
- Support both:
  - one commit <- many sessions/sources
  - one session -> many commits

## Research Summary

### Official Claude Code surfaces

- Claude Code statusline runs locally and does not consume API tokens.
- Statusline payload includes:
  - `session_id`
  - `transcript_path`
  - `cwd`
  - `workspace.current_dir`
  - `workspace.project_dir`
  - cumulative cost and cumulative token counts
  - `worktree.name`
  - `worktree.path`
  - `worktree.branch`
  - `worktree.original_cwd`
  - `worktree.original_branch`
- Claude Code hooks expose lifecycle events including:
  - `SessionStart`
  - `SessionEnd`
  - `CwdChanged`
  - `WorktreeCreate`
  - `WorktreeRemove`
  - `FileChanged`

Implication:

- Claude is already capable of providing exact session identity and worktree context.
- A high-precision mode can be built with only local hooks/statusline, without extra network calls or token spend.

Sources:

- https://code.claude.com/docs/en/statusline
- https://code.claude.com/docs/en/hooks

### Git worktree and hook behavior

- Git hooks run in the working tree root for non-bare repos.
- `core.hooksPath` can redirect hooks to a shared hook runner.
- `git rev-parse` provides the right primitives:
  - `--show-toplevel`
  - `--git-dir`
  - `--git-common-dir`
- The shared repo identity should be derived from `git-common-dir`.
- The per-worktree identity should be derived from `git-dir`.

Implication:

- Repo identity and worktree identity must be stored separately.
- Matching by branch name or path alone is not stable enough.
- `post-commit` and `post-rewrite` are the right hook points for exact commit attribution.

Sources:

- https://git-scm.com/docs/githooks
- https://git-scm.com/docs/git-worktree
- https://git-scm.com/docs/git-rev-parse
- https://git-scm.com/docs/git-notes

### Codex / local JSONL practice

- OctoMonitor already parses Codex local JSONL sessions.
- Codex transcript data contains:
  - session/thread identity
  - cwd
  - model
  - timestamps
  - cumulative token counts via `token_count`
- Session scans are already good enough for read-only attribution.

Implication:

- For Codex, turn-level token events can be approximated by delta-ing cumulative `token_count`.
- For the MVP, session-level usage can be apportioned heuristically to commits.

### Community patterns worth copying

- `ccusage` proves local JSONL parsing is a viable and accepted usage accounting strategy.
- `claude-statusline-powerline` keeps a local usage store and project mapping for fast session analytics.
- `claude-code-hooks-multi-agent-observability` uses Claude hooks as an observability event stream.
- `Git AI` uses `post-commit` plus `git notes` to attach AI attribution metadata to commits.
- Community session managers for Claude Code commonly key session continuity by git worktree or gitdir rather than by raw path or branch.

Implication:

- The strongest pattern is:
  - local append-only ledger as source of truth
  - optional git notes mirror for portability and inspection
  - worktree-aware session identity
- Known weak spots in community practice:
  - `git mv`
  - generated code
  - lockfile churn
  - reformat-only commits
  - amend/rebase rewrites without explicit rewrite handling

Sources:

- https://github.com/ryoppippi/ccusage
- https://github.com/spences10/claude-statusline-powerline
- https://github.com/disler/claude-code-hooks-multi-agent-observability
- https://usegitai.com/docs/cli/how-git-ai-works
- https://github.com/Xuanwo/xlaude
- https://www.reddit.com/r/VibeCodersNest/comments/1p7lb3k/i_built_a_desktop_app_to_fix_claude_codes_session/

## Design Review

### What must be true

1. Multi-worktree use must not collapse into one undifferentiated repo stream.
2. Historical backfill must be possible in read-only mode.
3. Exact attribution must remain possible later via hooks without changing the schema.
4. Remote viewer mode must not leak absolute paths or transcript paths.
5. Session totals must not be double-counted across multiple commits.

### Resulting architecture decision

Use a layered model:

- Identity layer:
  - repo identity
  - worktree identity
  - session identity
- Event layer:
  - session usage snapshots
  - later turn-level usage events
  - commit creation / commit rewrite events
- Attribution layer:
  - many-to-many links between commits and sessions
  - weighted token/cost allocation
  - explicit confidence + method

## Recommended End State

### Default mode: read-only heuristic attribution

- Scan git history from monitored repos
- Scan Claude/Codex/OpenClaw sessions already known to OctoMonitor
- Match commits to runs using:
  - same repo
  - same worktree when available
  - timestamp proximity
  - session interval overlap
  - weak text similarity between commit summary and session prompts/actions
- Allocate run token/cost totals proportionally across matching commits

Benefits:

- No repo mutation
- No hook installation required
- Historical backfill works when local transcripts still exist

Tradeoff:

- Confidence is heuristic, not exact

### High-precision mode: hook-backed attribution

- Install `post-commit` and `post-rewrite`
- Record commit events into a local append-only ledger
- Record active session context at commit time
- Optionally mirror compact attribution summaries into `git notes`

Benefits:

- Exact worktree and commit-write linkage
- Handles amend/rebase better
- Survives transcript cleanup if ledger remains

Tradeoff:

- Writes local metadata
- Requires explicit user opt-in

## MVP To Implement Now

Implement the read-only vertical slice first:

- Extend run schema with VCS context
- Scan recent commits for monitored repos
- Scan commits across every local git worktree, not just the primary worktree HEAD
- Compute heuristic commit attribution from current runs
- Return commits in bootstrap payload
- Add a `Commits` tab in the web UI
- Show both per-commit source totals and expandable per-session attribution links

Do not implement yet:

- hook installation
- append-only ledger
- git notes mirror
- turn-level persistent attribution store

The schema must still be shaped so those can be added later without breaking the frontend.

## Implemented MVP Review

The current MVP should satisfy these read-only requirements:

- Runs carry repo/worktree identity
- Commit scanning is worktree-aware via `git worktree list --porcelain`
- Attribution remains heuristic, but now prefers same worktree and same branch when those signals exist
- The `Commits` tab shows:
  - per-project commit groups
  - per-commit time, author, summary, diff size, attributed tokens, attributed cost
  - per-source totals
  - expandable session links with allocated token/cost shares

Known limitations still remain:

- Session attribution is still session-level, not turn-level
- No hook-backed exact mode yet
- No persistent local ledger yet
- Rebase/amend history is only approximated in read-only mode

## Schema Design

### VCS context

Attach VCS identity to runs:

- `repo_id`
- `repo_name`
- `repo_root`
- `worktree_id`
- `worktree_name`
- `worktree_path`
- `branch`
- `confidence`

Rules:

- `repo_id` is derived from canonical `git-common-dir`
- `worktree_id` is derived from canonical `git-dir`
- `repo_root` is canonical `git rev-parse --show-toplevel`
- `worktree_path` is the active worktree root

### Commit attribution types

`CommitAttributionMethod`

- `ReadOnlyHeuristic`
- `HookExact`
- `HookRewrite`
- `Mixed`

`CommitSourceStat`

- per-source totals for a commit
- fields:
  - tool
  - run_count
  - attributed_tokens
  - attributed_cost_usd
  - confidence

`CommitAttributionLink`

- one link from one run/session to one commit
- fields:
  - run_id
  - tool
  - source_mode
  - project_name
  - session_label
  - score
  - allocated_tokens
  - allocated_cost_usd
  - confidence
  - method

`CommitRecord`

- fields:
  - id
  - repo_id
  - repo_name
  - repo_root
  - worktree_id
  - worktree_name
  - sha
  - short_sha
  - author_name
  - committed_at
  - summary
  - files_changed
  - insertions
  - deletions
  - attributed_tokens
  - attributed_cost_usd
  - run_count
  - source_count
  - confidence
  - method
  - sources
  - links

## API Design

### Implement now

`GET /api/bootstrap`

- Add `commits: CommitRecord[]`
- Keep it as the default data source for the new Commits tab

### Design now, implement later

`GET /api/commits`

- Query params:
  - `repoId`
  - `from`
  - `to`
  - `limit`
  - `cursor`
- Purpose:
  - paginated commit history
  - lighter payload than bootstrap for larger installs

`GET /api/commits/{sha}`

- Return one commit plus full attribution links

`POST /api/ingest/git/commit`

- Hook-backed exact commit event
- Payload should include:
  - repo identity
  - worktree identity
  - branch
  - sha
  - parent sha(s)
  - committed_at
  - active session ids

`POST /api/ingest/git/rewrite`

- Hook-backed commit rewrite mapping
- Payload should include:
  - old sha
  - new sha
  - rewrite kind
  - repo/worktree identity

## Attribution Algorithm

### Read-only heuristic scoring

For each run:

1. Restrict candidate commits to same `repo_id`
2. Prefer same `worktree_id` when available
3. Restrict by timestamp window around the run interval
4. Score each candidate using:
   - commit inside run interval
   - distance to run interval boundary
   - exact worktree match
   - weak keyword overlap between commit summary and run prompt/action
5. Normalize scores
6. Allocate the run's token/cost totals across those commits

Properties:

- one run can feed multiple commits
- one commit can receive allocations from multiple runs and multiple tools
- total allocated tokens for a run do not exceed that run's total

### Confidence rules

MVP:

- all commit attributions are `Heuristic`
- method is `ReadOnlyHeuristic`

Later:

- hook-backed links become `Live` or `Official` depending on source quality
- mixed commits can merge exact and heuristic links

## Historical Data

### Can old data be recovered?

Yes, partially.

If the user still has:

- git history
- local Claude/Codex/OpenClaw transcripts

then OctoMonitor can backfill commit attribution heuristically.

If transcripts are gone:

- install-time historical exact recovery is usually impossible
- later hook/ledger data can still preserve new history going forward

### Current practical limit

Current adapters scan only recent session windows, so backfill quality is bounded by:

- transcript retention
- scan window
- session volume caps

## Frontend Proposal

Add a new top-level tab:

- `Monitor`
- `Usage`
- `Commits`
- `Settings`

The `Commits` tab should prioritize project clarity over raw table density.

### Layout

Top strip:

- total commits in range
- total attributed tokens
- total attributed cost
- project count
- date range picker

Project sections:

- repo name
- repo root short label
- project total commits
- project total tokens/cost

Commit cards or rows inside each project:

- commit time
- short sha
- summary
- author
- files / insertions / deletions
- total attributed tokens
- total attributed cost
- attribution confidence badge

Source breakdown per commit:

- one chip per source tool
- show:
  - source name
  - allocated tokens
  - allocated cost
  - session/run count

Optional detail line:

- linked run labels or project/session snippets

### Why this layout

- Users first think by project
- Then by commit
- Then by which source contributed

This is clearer than a flat global commit table when multiple repos and worktrees are active at once.

## Security / Privacy

Remote viewer mode must redact:

- absolute repo roots
- worktree paths
- session ids
- thread ids
- transcript paths

Safe to retain:

- repo names
- short shas
- summaries
- aggregate tokens/cost
- source labels

## Review Checklist

- Multi-worktree safe: yes, repo and worktree identities are distinct
- One commit <- many sessions: supported
- One session -> many commits: supported
- Read-only default preserved: yes
- Historical backfill possible: yes, heuristically
- Hook-based exact mode possible later: yes
- Remote redaction required: yes
