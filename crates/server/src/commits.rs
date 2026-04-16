use std::{
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
    score: f64,
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

    let branch = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("branch")
        .arg("--show-current")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

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

        for run in repo_run_list {
            let candidates = candidate_commits(run, &aggregated);
            if candidates.is_empty() {
                continue;
            }

            let weights = candidates.iter().map(|item| item.score).collect::<Vec<_>>();
            let token_allocations = allocate_proportionally(run.tokens.total, &weights);
            let run_cost = pricing.estimate_run_cost(run);
            let total_weight = weights.iter().sum::<f64>().max(1.0);

            for (idx, candidate) in candidates.iter().enumerate() {
                let tokens = token_allocations[idx];
                if tokens == 0 && run_cost.is_none() {
                    continue;
                }

                let share = candidate.score / total_weight;
                let allocated_cost = run_cost.map(|cost| cost * share);
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
                        score: share,
                        allocated_tokens: tokens,
                        allocated_cost_usd: allocated_cost,
                        confidence: SourceConfidence::Heuristic,
                        method: CommitAttributionMethod::ReadOnlyHeuristic,
                    });
            }
        }

        for aggregated_commit in aggregated {
            let mut run_ids = HashSet::new();
            let mut sources_by_tool: HashMap<ToolKind, CommitSourceStat> = HashMap::new();
            let mut worktree_votes: HashMap<(Option<String>, Option<String>), u64> = HashMap::new();
            let mut total_cost = 0.0;
            let mut has_cost = false;

            for link in &aggregated_commit.links {
                run_ids.insert(link.run_id.clone());
                let entry =
                    sources_by_tool
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
                    entry.attributed_cost_usd =
                        Some(entry.attributed_cost_usd.unwrap_or(0.0) + cost);
                    total_cost += cost;
                    has_cost = true;
                }

                if let Some(vcs) = run_index
                    .get(link.run_id.as_str())
                    .and_then(|run| run.vcs.as_ref())
                {
                    let key = (vcs.worktree_id.clone(), vcs.worktree_name.clone());
                    *worktree_votes.entry(key).or_insert(0) += link.allocated_tokens;
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
                .sum();

            let mut links = aggregated_commit.links;
            links.sort_by(|a, b| b.allocated_tokens.cmp(&a.allocated_tokens));

            out.push(CommitRecord {
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
            });
        }
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

fn candidate_commits(run: &RunRecord, commits: &[AggregatedCommit]) -> Vec<LinkCandidate> {
    let start = parse_timestamp_ms(&run.started_at);
    let end = parse_timestamp_ms(&run.last_activity_at);
    let (Some(start_ms), Some(end_ms)) = (start, end) else {
        return Vec::new();
    };

    commits
        .iter()
        .enumerate()
        .filter_map(|(idx, commit)| {
            let commit_ms = parse_timestamp_ms(&commit.base.committed_at)?;
            let time_score = temporal_score(commit_ms, start_ms, end_ms);
            if time_score <= 0.0 {
                return None;
            }

            let text_score = keyword_overlap_score(&commit.base.summary, run);
            let locality_score = locality_score(run, &commit.base)?;
            let score = time_score + text_score + locality_score;
            (score > 0.0).then_some(LinkCandidate {
                commit_index: idx,
                score,
            })
        })
        .collect()
}

fn locality_score(run: &RunRecord, commit: &ScannedCommit) -> Option<f64> {
    let Some(vcs) = run.vcs.as_ref() else {
        return Some(0.0);
    };

    let mut score = 0.0;
    let worktree_match = vcs
        .worktree_id
        .as_ref()
        .map(|worktree_id| commit.worktree_ids.contains(worktree_id))
        .unwrap_or(false);
    let branch_match = vcs
        .branch
        .as_ref()
        .map(|branch| commit.branches.contains(branch))
        .unwrap_or(false);

    if worktree_match {
        score += 0.45;
    } else if vcs.worktree_id.is_some() && !commit.worktree_ids.is_empty() && !branch_match {
        return None;
    }

    if branch_match {
        score += 0.2;
    }

    Some(score)
}

fn temporal_score(commit_ms: i64, start_ms: i64, end_ms: i64) -> f64 {
    if commit_ms < start_ms - PRE_COMMIT_GRACE_MS || commit_ms > end_ms + POST_COMMIT_GRACE_MS {
        return 0.0;
    }

    if commit_ms >= start_ms && commit_ms <= end_ms {
        let span = (end_ms - start_ms).max(1) as f64;
        let progress = (commit_ms - start_ms) as f64 / span;
        return 0.7 + 0.2 * progress;
    }

    if commit_ms > end_ms {
        let distance = (commit_ms - end_ms) as f64;
        return 0.2 + 0.6 * (1.0 - (distance / POST_COMMIT_GRACE_MS as f64)).max(0.0);
    }

    let distance = (start_ms - commit_ms) as f64;
    0.05 + 0.25 * (1.0 - (distance / PRE_COMMIT_GRACE_MS as f64)).max(0.0)
}

fn keyword_overlap_score(summary: &str, run: &RunRecord) -> f64 {
    let summary_tokens = tokenize(summary);
    if summary_tokens.is_empty() {
        return 0.0;
    }

    let session_parts: Vec<&str> = [Some(run.project_name.as_str())]
        .into_iter()
        .chain(
            [&run.first_question, &run.last_question, &run.last_action]
                .into_iter()
                .map(|opt| opt.as_deref()),
        )
        .flatten()
        .collect();
    let session_text = session_parts.join(" ");

    let session_tokens = tokenize(&session_text);
    if session_tokens.is_empty() {
        return 0.0;
    }

    let overlap = summary_tokens
        .iter()
        .filter(|token| session_tokens.contains(*token))
        .count();
    if overlap == 0 {
        return 0.0;
    }

    let overlap_ratio = overlap as f64 / summary_tokens.len() as f64;
    0.2 * overlap_ratio.min(1.0)
}

fn tokenize(text: &str) -> BTreeSet<String> {
    text.split(|ch: char| !ch.is_alphanumeric())
        .map(|token| token.trim().to_ascii_lowercase())
        .filter(|token| token.len() >= 3)
        .collect()
}

fn parse_timestamp_ms(value: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc).timestamp_millis())
}

fn allocate_proportionally(total: u64, weights: &[f64]) -> Vec<u64> {
    if total == 0 || weights.is_empty() {
        return vec![0; weights.len()];
    }

    let weight_sum = weights.iter().sum::<f64>();
    if weight_sum <= f64::EPSILON {
        return vec![0; weights.len()];
    }

    let raw = weights
        .iter()
        .map(|weight| (total as f64) * (*weight / weight_sum))
        .collect::<Vec<_>>();

    let mut floors = raw
        .iter()
        .map(|value| value.floor() as u64)
        .collect::<Vec<_>>();
    let allocated = floors.iter().sum::<u64>();
    let remainder = total.saturating_sub(allocated) as usize;

    let mut remainders = raw
        .iter()
        .enumerate()
        .map(|(idx, value)| (idx, value - value.floor()))
        .collect::<Vec<_>>();
    remainders.sort_by(|a, b| b.1.total_cmp(&a.1));

    for (idx, _) in remainders.into_iter().take(remainder) {
        floors[idx] += 1;
    }

    floors
}

fn primary_worktree(commit: &ScannedCommit) -> (Option<String>, Option<String>) {
    fn sole_element(set: &BTreeSet<String>) -> Option<String> {
        (set.len() == 1).then(|| set.iter().next().cloned())?
    }
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

        let branches: BTreeSet<String> = target.branch.iter().cloned().collect();
        let worktree_ids: BTreeSet<String> = target.worktree_id.iter().cloned().collect();
        let worktree_names: BTreeSet<String> = target.worktree_name.iter().cloned().collect();

        commits.push(ScannedCommit {
            sha: sha.to_string(),
            short_sha: short_sha.to_string(),
            author_name: author_name.to_string(),
            committed_at: committed_at.to_string(),
            summary: summary.to_string(),
            files_changed,
            insertions,
            deletions,
            branches,
            worktree_ids,
            worktree_names,
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
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success());
    match output {
        Some(o) => String::from_utf8(o.stdout)
            .map(|s| s.trim().to_string())
            .unwrap_or_default(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[test]
    fn allocate_proportionally_preserves_total() {
        let values = allocate_proportionally(100, &[3.0, 2.0, 1.0]);
        assert_eq!(values.iter().sum::<u64>(), 100);
        assert!(values[0] >= values[1]);
        assert!(values[1] >= values[2]);
    }

    #[test]
    fn keyword_overlap_scores_matching_terms() {
        let run = RunRecord {
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
                input: 0,
                output: 0,
                cache_read: 0,
                cache_write: 0,
                total: 0,
                context: 0,
            },
            cost: octomonitor_core::MoneyValue {
                usd: None,
                confidence: SourceConfidence::Derived,
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
            vcs: None,
            origin_label: None,
            origin_provider: None,
        };

        let score = keyword_overlap_score("Add worktree-aware commit attribution", &run);
        assert!(score > 0.0);
    }

    #[test]
    fn locality_score_rejects_mismatched_worktrees_without_branch_match() {
        let run = sample_run(Some(VcsContext {
            repo_id: "repo-1".into(),
            repo_name: "OctoMonitor".into(),
            repo_root: "/tmp/octomonitor".into(),
            worktree_id: Some("wt-feature".into()),
            worktree_name: Some("feature-wt".into()),
            worktree_path: Some("/tmp/octomonitor-feature".into()),
            branch: Some("feature".into()),
            confidence: SourceConfidence::Derived,
        }));
        let commit = ScannedCommit {
            sha: "abc".into(),
            short_sha: "abc".into(),
            author_name: "test".into(),
            committed_at: "2026-04-01T10:10:00Z".into(),
            summary: "feature work".into(),
            files_changed: 1,
            insertions: 1,
            deletions: 0,
            branches: BTreeSet::from(["main".into()]),
            worktree_ids: BTreeSet::from(["wt-main".into()]),
            worktree_names: BTreeSet::from(["main".into()]),
        };

        assert_eq!(locality_score(&run, &commit), None);
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
        assert!(commits
            .iter()
            .any(|commit| commit.summary == "feature worktree commit"));
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
                input: 0,
                output: 0,
                cache_read: 0,
                cache_write: 0,
                total: 0,
                context: 0,
            },
            cost: octomonitor_core::MoneyValue {
                usd: None,
                confidence: SourceConfidence::Derived,
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
            vcs,
            origin_label: None,
            origin_provider: None,
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
        root: PathBuf,
    }

    impl GitSandbox {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!("octomonitor-commit-tests-{unique}"));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }
    }

    impl Drop for GitSandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
