use octomonitor_claude_adapter as claude;
use octomonitor_codex_adapter as codex;
use octomonitor_openclaw_adapter as openclaw;

fn main() {
    println!("=== Codex Adapter ===");
    let snap = codex::probe();
    println!("CLI available: {}", snap.cli_available);
    println!("Config exists: {}", snap.config_exists);
    println!("Sessions scanned: {}", snap.sessions.len());
    for (i, s) in snap.sessions.iter().enumerate().take(5) {
        let id_short = if s.session_id.len() > 8 {
            &s.session_id[..8]
        } else {
            &s.session_id
        };
        let cwd_short = s
            .cwd
            .as_deref()
            .and_then(|c| c.split('/').last())
            .unwrap_or("?");
        println!(
            "  [{}] id={} cwd={} tokens={} 5h={:?} 7d={:?}",
            i, id_short, cwd_short, s.total_tokens, s.five_hour_used_pct, s.seven_day_used_pct
        );
    }

    println!("\n=== Claude Adapter ===");
    let csnap = claude::probe();
    println!("CLI available: {}", csnap.cli_available);
    println!("Sessions scanned: {}", csnap.sessions.len());
    for (i, s) in csnap.sessions.iter().enumerate().take(5) {
        println!(
            "  [{}] project={} tokens={} cost={:?} msgs={}",
            i, s.project_name, s.total_tokens, s.cost_usd, s.message_count
        );
    }
    println!("Quota: {:?}", csnap.quota);

    println!("\n=== OpenClaw Adapter ===");
    let osnap = openclaw::probe();
    println!("CLI available: {}", osnap.cli_available);
    println!("Sessions scanned: {}", osnap.sessions.len());
    for (i, s) in osnap.sessions.iter().enumerate().take(5) {
        println!(
            "  [{}] agent={} key={} status={} model={:?}",
            i, s.agent_name, s.session_key, s.status, s.model
        );
    }
}
