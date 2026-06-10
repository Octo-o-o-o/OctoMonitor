use std::{
    cmp::Ordering,
    collections::{BTreeSet, HashMap, HashSet},
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
    time::Instant,
};

use chrono::{DateTime, Utc};
use octomonitor_core::{
    CommitAttributionLink, CommitAttributionMethod, CommitRecord, CommitSourceStat, RunRecord,
    SourceConfidence, ToolKind, VcsContext,
};

use crate::perf;
use crate::pricing::PricingStore;

const PRE_COMMIT_GRACE_MS: i64 = 20 * 60 * 1000;
const POST_COMMIT_GRACE_MS: i64 = 60 * 60 * 1000;
const MAX_COMMITS_PER_WORKTREE_SCAN: usize = 600;

static COMMIT_CACHE: OnceLock<Mutex<HashMap<String, CachedCommitScan>>> = OnceLock::new();

#[derive(Debug, Clone)]
struct ScannedCommit {
    sha: String,
    short_sha: String,
    author_name: String,
    committed_at: String,
    summary: String,
    files_changed: u64,
    insertions: u64,
    deletions: u64,
    branches: BTreeSet<String>,
    worktree_ids: BTreeSet<String>,
    worktree_names: BTreeSet<String>,
}

#[derive(Debug, Clone)]
struct LinkCandidate {
    commit_index: usize,
    worktree_match: bool,
    branch_match: bool,
    time_distance_ms: i64,
    committed_at_ms: i64,
}

#[derive(Debug, Clone)]
struct AggregatedCommit {
    base: ScannedCommit,
    repo_id: String,
    repo_name: String,
    repo_root: String,
    links: Vec<CommitAttributionLink>,
}

#[derive(Debug, Clone)]
struct CachedCommitScan {
    refs_fingerprint: String,
    commits: Vec<ScannedCommit>,
}

#[derive(Debug, Clone)]
struct WorktreeScanTarget {
    path: String,
    branch: Option<String>,
    worktree_id: Option<String>,
    worktree_name: Option<String>,
}

pub fn discover_vcs_context(path: &str) -> Option<VcsContext> {
    if path.is_empty() {
        return None;
    }
    let input = Path::new(path);
    if !input.is_absolute() {
        return None;
    }

    let output = run_rev_parse(path)?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    let mut lines = stdout.lines();
    let worktree_root_raw = lines.next()?.trim();
    let git_dir_raw = lines.next()?.trim();
    let common_dir_raw = lines.next()?.trim();

    let worktree_root = canonicalize_flexible(Path::new(worktree_root_raw), input.parent());
    let git_dir = resolve_git_storage(canonicalize_flexible(
        Path::new(git_dir_raw),
        Some(&worktree_root),
    ));
    let common_dir = derive_common_dir(
        resolve_git_storage(canonicalize_flexible(
            Path::new(common_dir_raw),
            Some(&worktree_root),
        )),
        &git_dir,
    );
    let is_git_dir = common_dir.file_name().is_some_and(|name| name == ".git");
    let repo_root = if is_git_dir {
        common_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| worktree_root.clone())
    } else {
        worktree_root.clone()
    };

    let branch = {
        let raw = command_stdout(path, &["branch", "--show-current"]);
        (!raw.is_empty()).then_some(raw)
    };

    let repo_name = repo_root
        .file_name()
        .map(|part| part.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".into());
    let worktree_name = worktree_root
        .file_name()
        .map(|part| part.to_string_lossy().to_string());

    Some(VcsContext {
        repo_id: stable_id("repo", &common_dir.to_string_lossy()),
        repo_name,
        repo_root: repo_root.to_string_lossy().to_string(),
        worktree_id: Some(stable_id("wt", &git_dir.to_string_lossy())),
        worktree_name,
        worktree_path: Some(worktree_root.to_string_lossy().to_string()),
        branch,
        confidence: SourceConfidence::Derived,
    })
}

pub fn hydrate_run_vcs(runs: &mut [RunRecord]) {
    let mut cache: HashMap<String, Option<VcsContext>> = HashMap::new();
    for run in runs {
        if run.vcs.is_some() {
            continue;
        }
        run.vcs = cache
            .entry(run.workspace_path.clone())
            .or_insert_with(|| discover_vcs_context(&run.workspace_path))
            .clone();
    }
}

pub fn build_commit_records(
    runs: &[RunRecord],
    pricing: &PricingStore,
    history_cutoff: DateTime<Utc>,
) -> Vec<CommitRecord> {
    let started_at = Instant::now();
    let mut repo_runs: HashMap<String, Vec<&RunRecord>> = HashMap::new();
    let mut repo_contexts: HashMap<String, VcsContext> = HashMap::new();
    let run_index = runs
        .iter()
        .map(|run| (run.id.as_str(), run))
        .collect::<HashMap<_, _>>();

    for run in runs {
        let Some(vcs) = &run.vcs else {
            continue;
        };
        repo_contexts
            .entry(vcs.repo_id.clone())
            .or_insert_with(|| vcs.clone());
        repo_runs.entry(vcs.repo_id.clone()).or_default().push(run);
    }

    let repo_count = repo_contexts.len();
    let mut out = Vec::new();

    for (repo_id, repo_vcs) in repo_contexts {
        let repo_run_list = repo_runs.remove(&repo_id).unwrap_or_default();
        let scan_cutoff =
            history_cutoff - chrono::Duration::milliseconds(PRE_COMMIT_GRACE_MS.max(0));
        let scanned = scan_recent_commits(&repo_vcs, Some(scan_cutoff));
        if scanned.is_empty() {
            continue;
        }

        out.extend(build_commit_records_for_repo(
            &repo_vcs,
            &repo_run_list,
            scanned,
            &run_index,
            pricing,
        ));
    }

    out.sort_by(|a, b| b.committed_at.cmp(&a.committed_at));
    perf::log_elapsed_with_details("build_commit_records", started_at, || {
        format!(
            "runs={} repos={} commits={}",
            runs.len(),
            repo_count,
            out.len()
        )
    });
    out
}

fn build_commit_records_for_repo(
    repo_vcs: &VcsContext,
    repo_runs: &[&RunRecord],
    scanned: Vec<ScannedCommit>,
    run_index: &HashMap<&str, &RunRecord>,
    pricing: &PricingStore,
) -> Vec<CommitRecord> {
    let mut aggregated = scanned
        .into_iter()
        .map(|base| AggregatedCommit {
            base,
            repo_id: repo_vcs.repo_id.clone(),
            repo_name: repo_vcs.repo_name.clone(),
            repo_root: repo_vcs.repo_root.clone(),
            links: Vec::new(),
        })
        .collect::<Vec<_>>();

    for run in repo_runs {
        let Some(candidate) = best_commit_candidate(run, &aggregated) else {
            continue;
        };
        let allocated_tokens = run.tokens.total;
        let allocated_cost = pricing.estimate_run_cost(run);
        if allocated_tokens == 0 && allocated_cost.is_none() {
            continue;
        }

        aggregated[candidate.commit_index]
            .links
            .push(CommitAttributionLink {
                run_id: run.id.clone(),
                tool: run.tool.clone(),
                source_mode: run.source_mode.clone(),
                project_name: run.project_name.clone(),
                session_label: run
                    .last_question
                    .clone()
                    .or_else(|| run.first_question.clone())
                    .or_else(|| run.last_action.clone())
                    .unwrap_or_else(|| run.project_name.clone()),
                score: 0.0,
                allocated_tokens,
                allocated_cost_usd: allocated_cost,
                confidence: SourceConfidence::Heuristic,
                method: CommitAttributionMethod::ReadOnlyHeuristic,
            });
    }

    aggregated
        .into_iter()
        .map(|aggregated_commit| finalize_commit_record(aggregated_commit, run_index))
        .collect()
}

fn finalize_commit_record(
    aggregated_commit: AggregatedCommit,
    run_index: &HashMap<&str, &RunRecord>,
) -> CommitRecord {
    let mut run_ids = HashSet::new();
    let mut sources_by_tool: HashMap<ToolKind, CommitSourceStat> = HashMap::new();
    let mut worktree_votes: HashMap<(Option<String>, Option<String>), u64> = HashMap::new();
    let mut total_cost = 0.0;
    let mut has_cost = false;

    for link in &aggregated_commit.links {
        run_ids.insert(link.run_id.clone());
        let entry = sources_by_tool
            .entry(link.tool.clone())
            .or_insert_with(|| CommitSourceStat {
                tool: link.tool.clone(),
                run_count: 0,
                attributed_tokens: 0,
                attributed_cost_usd: None,
                confidence: SourceConfidence::Heuristic,
            });
        entry.run_count += 1;
        entry.attributed_tokens += link.allocated_tokens;
        if let Some(cost) = link.allocated_cost_usd {
            entry.attributed_cost_usd = Some(entry.attributed_cost_usd.unwrap_or(0.0) + cost);
            total_cost += cost;
            has_cost = true;
        }

        if let Some(vcs) = run_index
            .get(link.run_id.as_str())
            .and_then(|run| run.vcs.as_ref())
        {
            let key = (vcs.worktree_id.clone(), vcs.worktree_name.clone());
            *worktree_votes.entry(key).or_insert(0) += link.allocated_tokens.max(1);
        }
    }

    let mut sources = sources_by_tool.into_values().collect::<Vec<_>>();
    sources.sort_by(|a, b| b.attributed_tokens.cmp(&a.attributed_tokens));

    let inferred_worktree = worktree_votes
        .into_iter()
        .max_by_key(|(_, tokens)| *tokens)
        .map(|(key, _)| key)
        .unwrap_or_else(|| primary_worktree(&aggregated_commit.base));

    let attributed_tokens = aggregated_commit
        .links
        .iter()
        .map(|link| link.allocated_tokens)
        .sum::<u64>();
    let link_count = aggregated_commit.links.len();
    let mut links = aggregated_commit.links;
    for link in &mut links {
        link.score = if attributed_tokens > 0 {
            link.allocated_tokens as f64 / attributed_tokens as f64
        } else if total_cost > 0.0 {
            link.allocated_cost_usd.unwrap_or(0.0) / total_cost
        } else if link_count > 0 {
            1.0 / link_count as f64
        } else {
            0.0
        };
    }
    links.sort_by(|a, b| b.allocated_tokens.cmp(&a.allocated_tokens));

    CommitRecord {
        id: format!(
            "{}:{}",
            aggregated_commit.repo_id, aggregated_commit.base.sha
        ),
        repo_id: aggregated_commit.repo_id,
        repo_name: aggregated_commit.repo_name,
        repo_root: aggregated_commit.repo_root,
        worktree_id: inferred_worktree.0,
        worktree_name: inferred_worktree.1,
        sha: aggregated_commit.base.sha,
        short_sha: aggregated_commit.base.short_sha,
        author_name: aggregated_commit.base.author_name,
        committed_at: aggregated_commit.base.committed_at,
        summary: aggregated_commit.base.summary,
        files_changed: aggregated_commit.base.files_changed,
        insertions: aggregated_commit.base.insertions,
        deletions: aggregated_commit.base.deletions,
        attributed_tokens,
        attributed_cost_usd: has_cost.then_some(total_cost),
        run_count: run_ids.len() as u64,
        source_count: sources.len() as u64,
        confidence: if links.is_empty() {
            SourceConfidence::Derived
        } else {
            SourceConfidence::Heuristic
        },
        method: CommitAttributionMethod::ReadOnlyHeuristic,
        sources,
        links,
    }
}

fn scan_recent_commits(vcs: &VcsContext, since: Option<DateTime<Utc>>) -> Vec<ScannedCommit> {
    if vcs.repo_root.is_empty() {
        return Vec::new();
    }

    let fingerprint = scan_fingerprint(&vcs.repo_root);
    let cache_key = commit_cache_key(&vcs.repo_root, since);
    let cache = COMMIT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(cache) = cache.lock() {
        if let Some(cached) = cache.get(&cache_key) {
            if cached.refs_fingerprint == fingerprint {
                return cached.commits.clone();
            }
        }
    }

    let mut commits_by_sha: HashMap<String, ScannedCommit> = HashMap::new();
    for target in list_worktree_targets(vcs) {
        for commit in scan_worktree_commits(&target, since) {
            let entry = commits_by_sha
                .entry(commit.sha.clone())
                .or_insert_with(|| commit.clone());
            entry.branches.extend(commit.branches.iter().cloned());
            entry
                .worktree_ids
                .extend(commit.worktree_ids.iter().cloned());
            entry
                .worktree_names
                .extend(commit.worktree_names.iter().cloned());
        }
    }

    let mut commits = commits_by_sha.into_values().collect::<Vec<_>>();
    commits.sort_by(|a, b| b.committed_at.cmp(&a.committed_at));

    if let Ok(mut cache) = cache.lock() {
        cache.insert(
            cache_key,
            CachedCommitScan {
                refs_fingerprint: fingerprint,
                commits: commits.clone(),
            },
        );
    }

    commits
}

fn best_commit_candidate(run: &RunRecord, commits: &[AggregatedCommit]) -> Option<LinkCandidate> {
    let start = parse_timestamp_ms(&run.started_at);
    let end = parse_timestamp_ms(&run.last_activity_at);
    let (Some(start_ms), Some(end_ms)) = (start, end) else {
        return None;
    };

    commits
        .iter()
        .enumerate()
        .filter_map(|(idx, commit)| {
            let commit_ms = parse_timestamp_ms(&commit.base.committed_at)?;
            if !within_commit_window(commit_ms, start_ms, end_ms) {
                return None;
            }

            let (worktree_match, branch_match) = locality_match(run, &commit.base)?;
            Some(LinkCandidate {
                commit_index: idx,
                worktree_match,
                branch_match,
                time_distance_ms: (commit_ms - end_ms).abs(),
                committed_at_ms: commit_ms,
            })
        })
        .max_by(compare_link_candidate)
}

fn locality_match(run: &RunRecord, commit: &ScannedCommit) -> Option<(bool, bool)> {
    let Some(vcs) = run.vcs.as_ref() else {
        return Some((false, false));
    };

    let worktree_match = vcs
        .worktree_id
        .as_ref()
        .is_some_and(|worktree_id| commit.worktree_ids.contains(worktree_id))
        || vcs
            .worktree_name
            .as_ref()
            .is_some_and(|worktree_name| commit.worktree_names.contains(worktree_name));
    let branch_match = vcs
        .branch
        .as_ref()
        .is_some_and(|branch| commit.branches.contains(branch));

    if (vcs.worktree_id.is_some() || vcs.worktree_name.is_some())
        && !worktree_match
        && !branch_match
    {
        return None;
    }

    Some((worktree_match, branch_match))
}

fn compare_link_candidate(a: &LinkCandidate, b: &LinkCandidate) -> Ordering {
    a.worktree_match
        .cmp(&b.worktree_match)
        .then_with(|| a.branch_match.cmp(&b.branch_match))
        .then_with(|| b.time_distance_ms.cmp(&a.time_distance_ms))
        .then_with(|| a.committed_at_ms.cmp(&b.committed_at_ms))
}

fn within_commit_window(commit_ms: i64, start_ms: i64, end_ms: i64) -> bool {
    commit_ms >= start_ms - PRE_COMMIT_GRACE_MS && commit_ms <= end_ms + POST_COMMIT_GRACE_MS
}

fn parse_timestamp_ms(value: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc).timestamp_millis())
}

fn primary_worktree(commit: &ScannedCommit) -> (Option<String>, Option<String>) {
    let sole_element = |set: &BTreeSet<String>| -> Option<String> {
        if set.len() == 1 {
            set.iter().next().cloned()
        } else {
            None
        }
    };
    (
        sole_element(&commit.worktree_ids),
        sole_element(&commit.worktree_names),
    )
}

fn canonicalize_flexible(path: &Path, base: Option<&Path>) -> PathBuf {
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else if let Some(base) = base {
        base.join(path)
    } else {
        path.to_path_buf()
    };
    std::fs::canonicalize(&resolved).unwrap_or(resolved)
}

fn resolve_git_storage(path: PathBuf) -> PathBuf {
    if path.is_file() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Some(gitdir) = content.trim().strip_prefix("gitdir: ") {
                return canonicalize_flexible(Path::new(gitdir), path.parent());
            }
        }
    }
    path
}

fn derive_common_dir(common_dir: PathBuf, git_dir: &Path) -> PathBuf {
    if common_dir.is_dir() {
        return common_dir;
    }
    git_dir
        .ancestors()
        .find(|ancestor| ancestor.file_name().is_some_and(|name| name == ".git"))
        .map(Path::to_path_buf)
        .unwrap_or(common_dir)
}

fn run_rev_parse(path: &str) -> Option<std::process::Output> {
    let shared_args = ["--show-toplevel", "--git-dir", "--git-common-dir"];

    // Try with --path-format=absolute first, fall back without it.
    Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "--path-format=absolute"])
        .args(shared_args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .or_else(|| {
            Command::new("git")
                .arg("-C")
                .arg(path)
                .arg("rev-parse")
                .args(shared_args)
                .output()
                .ok()
                .filter(|o| o.status.success())
        })
}

fn stable_id(prefix: &str, value: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in value.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{prefix}-{hash:016x}")
}

fn list_worktree_targets(vcs: &VcsContext) -> Vec<WorktreeScanTarget> {
    let stdout = Command::new("git")
        .arg("-C")
        .arg(&vcs.repo_root)
        .arg("worktree")
        .arg("list")
        .arg("--porcelain")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok());

    let Some(stdout) = stdout else {
        return fallback_worktree_targets(vcs);
    };

    let mut targets = Vec::new();
    let mut path: Option<String> = None;
    let mut branch: Option<String> = None;

    let mut flush = |path: &mut Option<String>, branch: &mut Option<String>| {
        let Some(worktree_path) = path.take() else {
            return;
        };
        let metadata = discover_vcs_context(&worktree_path);
        let worktree_name = metadata
            .as_ref()
            .and_then(|item| item.worktree_name.clone())
            .or_else(|| {
                Path::new(&worktree_path)
                    .file_name()
                    .map(|part| part.to_string_lossy().to_string())
            });
        targets.push(WorktreeScanTarget {
            path: worktree_path,
            branch: branch
                .take()
                .map(|value| value.trim_start_matches("refs/heads/").to_string()),
            worktree_id: metadata.and_then(|item| item.worktree_id),
            worktree_name,
        });
    };

    for line in stdout.lines() {
        if line.trim().is_empty() {
            flush(&mut path, &mut branch);
            continue;
        }
        if let Some(value) = line.strip_prefix("worktree ") {
            if path.is_some() {
                flush(&mut path, &mut branch);
            }
            path = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("branch ") {
            branch = Some(value.trim().to_string());
        }
    }

    flush(&mut path, &mut branch);
    if targets.is_empty() {
        fallback_worktree_targets(vcs)
    } else {
        targets
    }
}

fn fallback_worktree_targets(vcs: &VcsContext) -> Vec<WorktreeScanTarget> {
    vec![WorktreeScanTarget {
        path: vcs
            .worktree_path
            .clone()
            .unwrap_or_else(|| vcs.repo_root.clone()),
        branch: vcs.branch.clone(),
        worktree_id: vcs.worktree_id.clone(),
        worktree_name: vcs.worktree_name.clone(),
    }]
}

fn scan_worktree_commits(
    target: &WorktreeScanTarget,
    since: Option<DateTime<Utc>>,
) -> Vec<ScannedCommit> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(&target.path)
        .arg("log")
        .arg("--date=iso-strict")
        .arg("--no-renames")
        .arg(format!("--max-count={MAX_COMMITS_PER_WORKTREE_SCAN}"));
    if let Some(since) = since {
        command.arg(format!("--since=@{}", since.timestamp()));
    }
    let stdout = command
        .arg("--numstat")
        .arg("--pretty=format:\u{1e}%H\u{1f}%h\u{1f}%cI\u{1f}%an\u{1f}%s")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok());

    stdout
        .map(|s| parse_scanned_commits(&s, target))
        .unwrap_or_default()
}

fn commit_cache_key(repo_root: &str, since: Option<DateTime<Utc>>) -> String {
    let since_key = since.map(|value| value.timestamp() / 60).unwrap_or(-1);
    format!("{repo_root}|{since_key}|{MAX_COMMITS_PER_WORKTREE_SCAN}")
}

fn parse_scanned_commits(stdout: &str, target: &WorktreeScanTarget) -> Vec<ScannedCommit> {
    let mut commits = Vec::new();
    for record in stdout.split('\u{1e}').skip(1) {
        let trimmed = record.trim_start_matches('\n');
        if trimmed.is_empty() {
            continue;
        }

        let mut sections = trimmed.splitn(2, '\n');
        let header = sections.next().unwrap_or_default();
        let body = sections.next().unwrap_or_default();
        let mut fields = header.split('\u{1f}');
        let Some(sha) = fields.next() else { continue };
        let Some(short_sha) = fields.next() else {
            continue;
        };
        let Some(committed_at) = fields.next() else {
            continue;
        };
        let Some(author_name) = fields.next() else {
            continue;
        };
        let Some(summary) = fields.next() else {
            continue;
        };

        let mut files_changed = 0_u64;
        let mut insertions = 0_u64;
        let mut deletions = 0_u64;
        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut parts = line.splitn(3, '\t');
            let added = parts.next().unwrap_or_default();
            let removed = parts.next().unwrap_or_default();
            if parts.next().is_none() {
                continue;
            }
            files_changed += 1;
            insertions += added.parse::<u64>().unwrap_or(0);
            deletions += removed.parse::<u64>().unwrap_or(0);
        }

        commits.push(ScannedCommit {
            sha: sha.to_string(),
            short_sha: short_sha.to_string(),
            author_name: author_name.to_string(),
            committed_at: committed_at.to_string(),
            summary: summary.to_string(),
            files_changed,
            insertions,
            deletions,
            branches: target.branch.iter().cloned().collect(),
            worktree_ids: target.worktree_id.iter().cloned().collect(),
            worktree_names: target.worktree_name.iter().cloned().collect(),
        });
    }

    commits
}

fn scan_fingerprint(repo_root: &str) -> String {
    let refs = command_stdout(repo_root, &["show-ref", "--heads", "--hash"]);
    let worktrees = command_stdout(repo_root, &["worktree", "list", "--porcelain"]);
    format!("{refs}\n{worktrees}")
}

fn command_stdout(repo_root: &str, args: &[&str]) -> String {
    Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
    };

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn best_commit_candidate_prefers_same_worktree_over_branch_only() {
        let run = sample_run(Some(sample_vcs()));
        let candidates = vec![
            aggregated_commit(
                "branch-only",
                "2026-04-01T10:19:00Z",
                &["feature/commit-attribution"],
                &["wt-main"],
                &["main"],
            ),
            aggregated_commit(
                "same-worktree",
                "2026-04-01T10:05:00Z",
                &["feature/commit-attribution"],
                &["wt-feature"],
                &["feature-wt"],
            ),
        ];

        let selected = best_commit_candidate(&run, &candidates).expect("candidate should exist");
        assert_eq!(selected.commit_index, 1);
        assert!(selected.worktree_match);
    }

    #[test]
    fn best_commit_candidate_uses_branch_only_as_fallback() {
        let run = sample_run(Some(sample_vcs()));
        let candidates = vec![
            aggregated_commit(
                "branch-fallback",
                "2026-04-01T10:18:00Z",
                &["feature/commit-attribution"],
                &["wt-main"],
                &["main"],
            ),
            aggregated_commit(
                "unrelated",
                "2026-04-01T10:19:00Z",
                &["main"],
                &["wt-main"],
                &["main"],
            ),
        ];

        let selected = best_commit_candidate(&run, &candidates).expect("candidate should exist");
        assert_eq!(selected.commit_index, 0);
        assert!(!selected.worktree_match);
        assert!(selected.branch_match);
    }

    #[test]
    fn best_commit_candidate_rejects_mismatched_worktree_without_branch_match() {
        let run = sample_run(Some(sample_vcs()));
        let candidates = vec![aggregated_commit(
            "wrong-worktree",
            "2026-04-01T10:10:00Z",
            &["main"],
            &["wt-main"],
            &["main"],
        )];

        assert!(best_commit_candidate(&run, &candidates).is_none());
    }

    #[test]
    fn best_commit_candidate_rejects_mismatched_worktree_name_without_branch_match() {
        let mut vcs = sample_vcs();
        vcs.worktree_id = None;
        let run = sample_run(Some(vcs));
        let candidates = vec![aggregated_commit(
            "wrong-worktree",
            "2026-04-01T10:10:00Z",
            &["main"],
            &[],
            &["main"],
        )];

        assert!(best_commit_candidate(&run, &candidates).is_none());
    }

    #[test]
    fn build_commit_records_for_repo_assigns_each_run_once_and_keeps_unmatched_commit() {
        let pricing = PricingStore::new();
        let run = sample_run(Some(sample_vcs()));
        let run_refs = vec![&run];
        let run_index = HashMap::from([(run.id.as_str(), &run)]);
        let repo_vcs = sample_vcs();
        let records = build_commit_records_for_repo(
            &repo_vcs,
            &run_refs,
            vec![
                scanned_commit(
                    "matched",
                    "2026-04-01T10:15:00Z",
                    &["feature/commit-attribution"],
                    &["wt-feature"],
                    &["feature-wt"],
                ),
                scanned_commit(
                    "unmatched",
                    "2026-04-01T10:16:00Z",
                    &["main"],
                    &["wt-main"],
                    &["main"],
                ),
            ],
            &run_index,
            &pricing,
        );

        assert_eq!(records.len(), 2);
        assert_eq!(
            records
                .iter()
                .map(|record| record.links.len())
                .sum::<usize>(),
            1
        );

        let matched = records
            .iter()
            .find(|record| record.summary == "matched")
            .expect("matched commit");
        assert_eq!(matched.links.len(), 1);
        assert_eq!(matched.run_count, 1);
        assert_eq!(matched.attributed_tokens, run.tokens.total);
        assert_eq!(matched.links[0].allocated_tokens, run.tokens.total);
        assert_eq!(matched.links[0].score, 1.0);
        assert_eq!(matched.confidence, SourceConfidence::Heuristic);

        let unmatched = records
            .iter()
            .find(|record| record.summary == "unmatched")
            .expect("unmatched commit");
        assert!(unmatched.links.is_empty());
        assert_eq!(unmatched.confidence, SourceConfidence::Derived);
    }

    #[test]
    fn scan_recent_commits_includes_linked_worktree_history() {
        let sandbox = GitSandbox::new();
        let repo_root = sandbox.root.join("repo");
        let feature_root = sandbox.root.join("repo-feature");

        run_cmd(
            None,
            &[
                "git",
                "init",
                "--initial-branch=main",
                repo_root.to_str().unwrap(),
            ],
        );
        run_cmd(
            Some(&repo_root),
            &["git", "config", "user.name", "OctoMonitor Test"],
        );
        run_cmd(
            Some(&repo_root),
            &["git", "config", "user.email", "octomonitor@example.com"],
        );

        fs::write(repo_root.join("README.md"), "main\n").unwrap();
        run_cmd(Some(&repo_root), &["git", "add", "README.md"]);
        run_cmd(Some(&repo_root), &["git", "commit", "-m", "main setup"]);

        run_cmd(
            Some(&repo_root),
            &[
                "git",
                "worktree",
                "add",
                "-b",
                "feature/commit-attribution",
                feature_root.to_str().unwrap(),
            ],
        );

        fs::write(feature_root.join("feature.txt"), "feature\n").unwrap();
        run_cmd(Some(&feature_root), &["git", "add", "feature.txt"]);
        run_cmd(
            Some(&feature_root),
            &["git", "commit", "-m", "feature worktree commit"],
        );

        let vcs = discover_vcs_context(repo_root.to_str().unwrap()).expect("main vcs");
        let commits = scan_recent_commits(&vcs, None);
        assert!(
            commits
                .iter()
                .any(|commit| commit.summary == "feature worktree commit")
        );
    }

    #[test]
    fn scan_recent_commits_respects_since_cutoff() {
        let sandbox = GitSandbox::new();
        let repo_root = sandbox.root.join("repo");

        run_cmd(
            None,
            &[
                "git",
                "init",
                "--initial-branch=main",
                repo_root.to_str().unwrap(),
            ],
        );
        run_cmd(
            Some(&repo_root),
            &["git", "config", "user.name", "OctoMonitor Test"],
        );
        run_cmd(
            Some(&repo_root),
            &["git", "config", "user.email", "octomonitor@example.com"],
        );

        fs::write(repo_root.join("history.txt"), "old\n").unwrap();
        run_cmd(Some(&repo_root), &["git", "add", "history.txt"]);
        run_cmd_with_env(
            Some(&repo_root),
            &[
                ("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z"),
                ("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z"),
            ],
            &["git", "commit", "-m", "old baseline commit"],
        );

        fs::write(repo_root.join("history.txt"), "new\n").unwrap();
        run_cmd(Some(&repo_root), &["git", "add", "history.txt"]);
        run_cmd_with_env(
            Some(&repo_root),
            &[
                ("GIT_AUTHOR_DATE", "2026-04-02T00:00:00Z"),
                ("GIT_COMMITTER_DATE", "2026-04-02T00:00:00Z"),
            ],
            &["git", "commit", "-m", "recent hot commit"],
        );

        let vcs = discover_vcs_context(repo_root.to_str().unwrap()).expect("repo vcs");
        let commits = scan_recent_commits(
            &vcs,
            Some(
                DateTime::parse_from_rfc3339("2026-03-01T00:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc),
            ),
        );

        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].summary, "recent hot commit");
    }

    fn sample_run(vcs: Option<VcsContext>) -> RunRecord {
        RunRecord {
            id: "run-1".into(),
            tool: ToolKind::Claude,
            source_mode: "test".into(),
            project_name: "OctoMonitor".into(),
            workspace_path: "/tmp/octomonitor".into(),
            workspace_short: "~/octomonitor".into(),
            model: None,
            provider: None,
            agent_name: None,
            agent_display_name: None,
            account_alias: None,
            auth_mode: None,
            auth_verified: true,
            session_id: None,
            thread_id: None,
            session_key: None,
            transcript_path: None,
            started_at: "2026-04-01T10:00:00Z".into(),
            last_activity_at: "2026-04-01T10:20:00Z".into(),
            elapsed_ms: 0,
            state: octomonitor_core::RunState::Completed,
            last_action: Some("implement commit attribution".into()),
            last_tail: None,
            pending_approval: false,
            first_question: Some("design commit attribution".into()),
            last_question: Some("worktree aware commit attribution".into()),
            error_message: None,
            message_count: 1,
            tokens: octomonitor_core::TokenUsage {
                input: 700,
                output: 500,
                cache_read: 0,
                cache_write: 0,
                total: 1_200,
                context: 0,
            },
            cost: octomonitor_core::MoneyValue {
                usd: Some(2.4),
                confidence: SourceConfidence::Estimated,
            },
            quota: octomonitor_core::QuotaValue {
                five_hour_used_pct: None,
                seven_day_used_pct: None,
                reset_at: Vec::new(),
                confidence: SourceConfidence::Derived,
            },
            source: octomonitor_core::SourceInfo {
                confidence: SourceConfidence::Derived,
                freshness: octomonitor_core::Freshness::Warm,
                last_updated_at: "2026-04-01T10:20:00Z".into(),
            },
            source_id: Some("test:commit".into()),
            lifecycle: Some(octomonitor_core::SessionLifecycle::default()),
            usage_semantics: Some(octomonitor_core::UsageSemantics {
                cost_kind: octomonitor_core::UsageCostKind::Estimated,
                source: octomonitor_core::UsageDataSource::Computed,
                enters_usage_totals: true,
                note: None,
            }),
            data_sources: Some(Vec::new()),
            capabilities: Some(Vec::new()),
            jump_targets: Some(Vec::new()),
            tool_specific: Some(serde_json::json!({})),
            vcs,
            origin_label: None,
            origin_provider: None,
        }
    }

    fn sample_vcs() -> VcsContext {
        VcsContext {
            repo_id: "repo-1".into(),
            repo_name: "OctoMonitor".into(),
            repo_root: "/tmp/octomonitor".into(),
            worktree_id: Some("wt-feature".into()),
            worktree_name: Some("feature-wt".into()),
            worktree_path: Some("/tmp/octomonitor-feature".into()),
            branch: Some("feature/commit-attribution".into()),
            confidence: SourceConfidence::Derived,
        }
    }

    fn aggregated_commit(
        summary: &str,
        committed_at: &str,
        branches: &[&str],
        worktree_ids: &[&str],
        worktree_names: &[&str],
    ) -> AggregatedCommit {
        AggregatedCommit {
            base: scanned_commit(
                summary,
                committed_at,
                branches,
                worktree_ids,
                worktree_names,
            ),
            repo_id: "repo-1".into(),
            repo_name: "OctoMonitor".into(),
            repo_root: "/tmp/octomonitor".into(),
            links: Vec::new(),
        }
    }

    fn scanned_commit(
        summary: &str,
        committed_at: &str,
        branches: &[&str],
        worktree_ids: &[&str],
        worktree_names: &[&str],
    ) -> ScannedCommit {
        ScannedCommit {
            sha: format!("sha-{summary}"),
            short_sha: summary.chars().take(7).collect(),
            author_name: "test".into(),
            committed_at: committed_at.into(),
            summary: summary.into(),
            files_changed: 1,
            insertions: 1,
            deletions: 0,
            branches: branches.iter().map(|value| (*value).to_string()).collect(),
            worktree_ids: worktree_ids
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            worktree_names: worktree_names
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
        }
    }

    fn run_cmd(cwd: Option<&Path>, args: &[&str]) {
        run_cmd_with_env(cwd, &[], args);
    }

    fn run_cmd_with_env(cwd: Option<&Path>, envs: &[(&str, &str)], args: &[&str]) {
        let mut command = Command::new(args[0]);
        command.args(&args[1..]);
        if let Some(dir) = cwd {
            command.current_dir(dir);
        }
        for (key, value) in envs {
            command.env(key, value);
        }
        let output = command.output().expect("command should start");
        assert!(
            output.status.success(),
            "command failed: {:?}\nstdout: {}\nstderr: {}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    struct GitSandbox {
        _dir: TempDir,
        root: PathBuf,
    }

    impl GitSandbox {
        fn new() -> Self {
            let dir = tempfile::tempdir().expect("temp dir");
            let root = dir.path().to_path_buf();
            Self { _dir: dir, root }
        }
    }
}
