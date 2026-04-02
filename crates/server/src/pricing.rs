//! LiteLLM-based model pricing with file cache.
//!
//! Fetches `model_prices_and_context_window.json` from LiteLLM's GitHub repo,
//! caches it locally at `~/.octomonitor/litellm_pricing.json`, and provides
//! per-model cost calculation using base rates.  Tiered pricing (200K
//! threshold) is intentionally NOT applied because the token counts we receive
//! are session-level aggregates, not per-request totals — matching ccusage's
//! effective behaviour where per-entry tokens never reach the threshold.
//!
//! Cache strategy:
//!   - On startup: load from disk cache if < 24 h old.
//!   - If stale or missing: use stale cache / compiled-in defaults; fetch in
//!     background.
//!   - On fetch success: atomically write to cache file.
//!   - Background refresh: every 24 h while server is running.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use octomonitor_core::{RunRecord, ToolKind};
use serde::{Deserialize, Serialize};

// ── Constants ──────────────────────────────────────────────────────────────────

const LITELLM_PRICING_URL: &str =
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";

/// How long a cached file is considered fresh.
const CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(24 * 3600);

/// Background refresh interval.
const REFRESH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(24 * 3600);

/// HTTP request timeout.
const FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

// ── Provider prefixes to try when model name lacks a provider ───────────────

const PROVIDER_PREFIXES: &[&str] = &["anthropic/", "openai/", "azure/", "openrouter/openai/"];

const CODEX_MODEL_ALIASES: &[(&str, &str)] =
    &[("gpt-5-codex", "gpt-5"), ("gpt-5.3-codex", "gpt-5.2-codex")];

// ── Types ──────────────────────────────────────────────────────────────────────

/// Per-model pricing entry from LiteLLM's JSON.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ModelPricing {
    #[serde(default)]
    pub input_cost_per_token: Option<f64>,
    #[serde(default)]
    pub output_cost_per_token: Option<f64>,
    #[serde(default)]
    pub cache_creation_input_token_cost: Option<f64>,
    #[serde(default)]
    pub cache_read_input_token_cost: Option<f64>,

    // Tiered pricing — Claude 200K threshold
    #[serde(default)]
    pub input_cost_per_token_above_200k_tokens: Option<f64>,
    #[serde(default)]
    pub output_cost_per_token_above_200k_tokens: Option<f64>,
    #[serde(default)]
    pub cache_creation_input_token_cost_above_200k_tokens: Option<f64>,
    #[serde(default)]
    pub cache_read_input_token_cost_above_200k_tokens: Option<f64>,
}

/// Resolved base per-token rates for cost calculation.
/// Tiered (above-200K) rates are intentionally excluded: the token counts
/// we receive are session-level aggregates, not per-request totals, so
/// applying tiered pricing would over-charge.
struct ResolvedRates {
    input: f64,
    output: f64,
    cache_read: f64,
    cache_write: f64,
}

enum PricingResolution {
    Rates(ResolvedRates),
    Zero,
}

/// Disk cache envelope.
#[derive(Serialize, Deserialize)]
struct CacheEnvelope {
    fetched_at: i64,
    data: HashMap<String, ModelPricing>,
}

// ── PricingStore ───────────────────────────────────────────────────────────────

/// Thread-safe pricing store.  Uses `std::sync::RwLock` so it can be read from
/// both async handlers and blocking probe threads without requiring `.await`.
#[derive(Clone)]
pub struct PricingStore {
    models: Arc<RwLock<HashMap<String, ModelPricing>>>,
}

impl PricingStore {
    // ── Construction ────────────────────────────────────────────────────────

    /// Create a new PricingStore, loading from disk cache if available.
    pub fn new() -> Self {
        let data = match load_cache() {
            Some((data, fresh)) => {
                if fresh {
                    tracing::info!("LiteLLM pricing loaded from cache ({} models)", data.len());
                } else {
                    tracing::info!(
                        "LiteLLM pricing loaded from stale cache ({} models), will refresh",
                        data.len()
                    );
                }
                data
            }
            None => {
                tracing::info!("No LiteLLM pricing cache, using built-in defaults");
                HashMap::new()
            }
        };

        Self {
            models: Arc::new(RwLock::new(data)),
        }
    }

    /// Spawn a background tokio task that keeps the pricing data fresh.
    pub fn spawn_refresh(&self) {
        let store = self.clone();
        tokio::spawn(async move {
            // Immediate fetch if cache was empty or stale
            let needs_immediate = {
                let m = store.models.read().unwrap_or_else(|p| p.into_inner());
                m.is_empty() || !is_cache_fresh()
            };
            if needs_immediate {
                store.fetch_and_update().await;
            }

            loop {
                tokio::time::sleep(REFRESH_INTERVAL).await;
                store.fetch_and_update().await;
            }
        });
    }

    // ── Cost calculation (sync — callable from any thread) ──────────────────

    /// Calculate USD cost for a run.  Returns `None` when cost is zero.
    ///
    /// Uses base (non-tiered) rates only.  The token counts passed here are
    /// session-level aggregates (summed across many API requests).  Tiered
    /// pricing (200K threshold) is designed for per-request context windows,
    /// not for session totals.  Applying it to aggregates would massively
    /// inflate costs — matching ccusage's per-entry calculation behaviour.
    pub fn estimate_cost(
        &self,
        tool: &ToolKind,
        model: Option<&str>,
        input: u64,
        output: u64,
        cache_read: u64,
        cache_write: u64,
    ) -> Option<f64> {
        let model_name = model.unwrap_or("");
        let (billable_input, billable_cache_read) = billable_usage(tool, input, cache_read);
        let cost = match self.resolve_rates(tool, model_name) {
            PricingResolution::Rates(rates) => {
                billable_input as f64 * rates.input
                    + output as f64 * rates.output
                    + billable_cache_read as f64 * rates.cache_read
                    + cache_write as f64 * rates.cache_write
            }
            PricingResolution::Zero => 0.0,
        };

        Some(cost)
    }

    /// Calculate USD cost for a run, preferring adapter-provided USD when present.
    pub fn estimate_run_cost(&self, run: &RunRecord) -> Option<f64> {
        if run.cost.usd.is_some() {
            return run.cost.usd;
        }

        self.estimate_cost(
            &run.tool,
            run.model.as_deref(),
            run.tokens.input,
            run.tokens.output,
            run.tokens.cache_read,
            run.tokens.cache_write,
        )
    }

    // ── Model resolution ────────────────────────────────────────────────────

    /// Find pricing for a model name.  Tries, in order:
    ///   1. Exact match in LiteLLM data
    ///   2. Provider-prefixed matches (anthropic/, openai/, azure/, etc.)
    ///   3. Substring match (for partial model names from adapters)
    ///   4. Compiled-in fallback defaults
    fn resolve_rates(&self, tool: &ToolKind, model: &str) -> PricingResolution {
        if model.trim().is_empty() || is_openrouter_free_model(model) {
            return PricingResolution::Zero;
        }

        let models = self.models.read().unwrap_or_else(|p| p.into_inner());
        let store_empty = models.is_empty();

        let zero_rates = || {
            PricingResolution::Rates(ResolvedRates {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
            })
        };

        // 1. Exact/provider-prefixed/substring match
        let direct = Self::lookup_model_pricing(&models, model);
        if let Some(p) = direct {
            if has_non_zero_token_pricing(p) {
                return PricingResolution::Rates(Self::pricing_to_rates(tool, p));
            }
        }

        // 2. Codex-style aliases when direct pricing is missing or all-zero.
        if let Some(alias) = codex_model_alias(model) {
            if let Some(p) = Self::lookup_model_pricing(&models, alias) {
                if has_non_zero_token_pricing(p) {
                    return PricingResolution::Rates(Self::pricing_to_rates(tool, p));
                }
            }
        }

        if let Some(p) = direct {
            return PricingResolution::Rates(Self::pricing_to_rates(tool, p));
        }

        drop(models);

        // Match ccusage's behavior: once pricing data is available, an
        // unknown model should not be heuristically charged.
        if store_empty {
            PricingResolution::Rates(Self::fallback_rates(model))
        } else {
            zero_rates()
        }
    }

    fn lookup_model_pricing<'a>(
        models: &'a HashMap<String, ModelPricing>,
        model: &str,
    ) -> Option<&'a ModelPricing> {
        if model.is_empty() {
            return None;
        }

        if let Some(p) = models.get(model) {
            return Some(p);
        }

        for prefix in PROVIDER_PREFIXES {
            let key = format!("{}{}", prefix, model);
            if let Some(p) = models.get(&key) {
                return Some(p);
            }
        }

        for (key, p) in models.iter() {
            let bare = key.split('/').next_back().unwrap_or(key);
            if bare == model || key.contains(model) {
                return Some(p);
            }
        }

        None
    }

    fn pricing_to_rates(tool: &ToolKind, p: &ModelPricing) -> ResolvedRates {
        let codex_cache_fallback = if matches!(tool, ToolKind::Codex) {
            p.input_cost_per_token.unwrap_or(0.0)
        } else {
            0.0
        };

        ResolvedRates {
            input: p.input_cost_per_token.unwrap_or(0.0),
            output: p.output_cost_per_token.unwrap_or(0.0),
            // Match ccusage's Codex rules: cached input falls back to input
            // pricing when LiteLLM omits an explicit cache-read rate.
            cache_read: p
                .cache_read_input_token_cost
                .unwrap_or(codex_cache_fallback),
            cache_write: p.cache_creation_input_token_cost.unwrap_or(0.0),
        }
    }

    /// Hard-coded fallback base rates (per-token) when LiteLLM data is unavailable.
    fn fallback_rates(model: &str) -> ResolvedRates {
        let (inp, out, cr, cw) = if model.contains("opus") {
            (15e-6, 75e-6, 1.5e-6, 18.75e-6)
        } else if model.contains("sonnet") {
            (3e-6, 15e-6, 0.3e-6, 3.75e-6)
        } else if model.contains("haiku") {
            (0.8e-6, 4e-6, 0.08e-6, 1e-6)
        } else if model.contains("gpt-5") || model.contains("o3") || model.contains("o4-mini") {
            (2e-6, 8e-6, 0.5e-6, 2.5e-6)
        } else if model.contains("gpt-4") {
            (2.5e-6, 10e-6, 0.625e-6, 3.125e-6)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };

        ResolvedRates {
            input: inp,
            output: out,
            cache_read: cr,
            cache_write: cw,
        }
    }

    // ── Background fetch ────────────────────────────────────────────────────

    async fn fetch_and_update(&self) {
        match fetch_litellm_pricing().await {
            Ok(data) => {
                let count = data.len();
                save_cache(&data);
                let mut models = self.models.write().unwrap_or_else(|p| p.into_inner());
                *models = data;
                drop(models);
                tracing::info!("LiteLLM pricing refreshed ({} models)", count);
            }
            Err(e) => {
                tracing::warn!("Failed to fetch LiteLLM pricing: {}", e);
            }
        }
    }
}

fn has_non_zero_token_pricing(pricing: &ModelPricing) -> bool {
    pricing.input_cost_per_token.unwrap_or(0.0) > 0.0
        || pricing.output_cost_per_token.unwrap_or(0.0) > 0.0
        || pricing.cache_read_input_token_cost.unwrap_or(0.0) > 0.0
}

fn codex_model_alias(model: &str) -> Option<&'static str> {
    CODEX_MODEL_ALIASES
        .iter()
        .find_map(|(from, to)| (*from == model).then_some(*to))
}

fn is_openrouter_free_model(model: &str) -> bool {
    let normalized = model.trim().to_ascii_lowercase();
    normalized == "openrouter/free"
        || (normalized.starts_with("openrouter/") && normalized.ends_with(":free"))
}

fn billable_usage(tool: &ToolKind, input: u64, cache_read: u64) -> (u64, u64) {
    if matches!(tool, ToolKind::Codex) {
        let cached = cache_read.min(input);
        (input.saturating_sub(cached), cached)
    } else {
        (input, cache_read)
    }
}

// ── HTTP fetch ─────────────────────────────────────────────────────────────────

async fn fetch_litellm_pricing() -> anyhow::Result<HashMap<String, ModelPricing>> {
    let client = reqwest::Client::builder().timeout(FETCH_TIMEOUT).build()?;

    tracing::debug!("Fetching LiteLLM pricing from {}", LITELLM_PRICING_URL);

    let resp = client.get(LITELLM_PRICING_URL).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("HTTP {}", resp.status());
    }

    // LiteLLM JSON is a flat object: { "model-name": { ...fields }, ... }
    // Some entries have non-object values (e.g. "sample_spec"), filter those.
    let raw: HashMap<String, serde_json::Value> = resp.json().await?;
    let mut models = HashMap::with_capacity(raw.len());

    for (key, value) in raw {
        if value.is_object() {
            if let Ok(pricing) = serde_json::from_value::<ModelPricing>(value) {
                if pricing.input_cost_per_token.is_some() || pricing.output_cost_per_token.is_some()
                {
                    models.insert(key, pricing);
                }
            }
        }
    }

    Ok(models)
}

// ── Disk cache ─────────────────────────────────────────────────────────────────

fn cache_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let dir = PathBuf::from(home).join(".octomonitor");
    if !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
    }
    Some(dir.join("litellm_pricing.json"))
}

fn is_cache_fresh() -> bool {
    let Some(path) = cache_path() else {
        return false;
    };
    match std::fs::metadata(&path) {
        Ok(meta) => {
            if let Ok(modified) = meta.modified() {
                modified.elapsed().unwrap_or(CACHE_TTL) < CACHE_TTL
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

/// Load cache from disk.  Returns `(data, is_fresh)`.
fn load_cache() -> Option<(HashMap<String, ModelPricing>, bool)> {
    let path = cache_path()?;
    let bytes = std::fs::read(&path).ok()?;
    let envelope: CacheEnvelope = serde_json::from_slice(&bytes).ok()?;
    let fresh = is_cache_fresh();
    Some((envelope.data, fresh))
}

fn save_cache(data: &HashMap<String, ModelPricing>) {
    let Some(path) = cache_path() else { return };
    let envelope = CacheEnvelope {
        fetched_at: chrono::Utc::now().timestamp(),
        data: data.clone(),
    };
    let tmp = path.with_extension("json.tmp");
    if let Ok(bytes) = serde_json::to_vec(&envelope) {
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round12(value: f64) -> f64 {
        (value * 1e12_f64).round() / 1e12_f64
    }

    fn pricing_store(models: &[(&str, ModelPricing)]) -> PricingStore {
        PricingStore {
            models: Arc::new(RwLock::new(
                models
                    .iter()
                    .map(|(name, pricing)| ((*name).to_string(), pricing.clone()))
                    .collect(),
            )),
        }
    }

    #[test]
    fn codex_alias_uses_canonical_model_pricing() {
        let store = pricing_store(&[(
            "gpt-5",
            ModelPricing {
                input_cost_per_token: Some(1.25e-6),
                output_cost_per_token: Some(1.0e-5),
                cache_read_input_token_cost: Some(1.25e-7),
                ..Default::default()
            },
        )]);

        let cost = store.estimate_cost(&ToolKind::Codex, Some("gpt-5-codex"), 1_000, 500, 200, 0);

        let expected = 800.0 * 1.25e-6 + 500.0 * 1.0e-5 + 200.0 * 1.25e-7;
        assert_eq!(cost.map(round12), Some(round12(expected)));
    }

    #[test]
    fn codex_cache_read_falls_back_to_input_rate() {
        let store = pricing_store(&[(
            "gpt-5",
            ModelPricing {
                input_cost_per_token: Some(2.0e-6),
                output_cost_per_token: Some(8.0e-6),
                cache_read_input_token_cost: None,
                ..Default::default()
            },
        )]);

        let cost = store.estimate_cost(&ToolKind::Codex, Some("gpt-5"), 1_000, 500, 100, 0);
        let expected = 900.0 * 2.0e-6 + 500.0 * 8.0e-6 + 100.0 * 2.0e-6;
        assert_eq!(cost.map(round12), Some(round12(expected)));
    }

    #[test]
    fn codex_cached_input_is_not_double_charged() {
        let store = pricing_store(&[(
            "gpt-5",
            ModelPricing {
                input_cost_per_token: Some(2.0e-6),
                output_cost_per_token: Some(8.0e-6),
                cache_read_input_token_cost: Some(2.0e-7),
                ..Default::default()
            },
        )]);

        let cost = store.estimate_cost(&ToolKind::Codex, Some("gpt-5"), 1_000, 500, 100, 0);
        let expected = 900.0 * 2.0e-6 + 500.0 * 8.0e-6 + 100.0 * 2.0e-7;
        assert_eq!(cost.map(round12), Some(round12(expected)));
    }

    #[test]
    fn openrouter_free_models_are_zero_cost() {
        let store = pricing_store(&[]);
        assert_eq!(
            store.estimate_cost(
                &ToolKind::Codex,
                Some("openrouter/openai/gpt-5:free"),
                1_000,
                500,
                200,
                0,
            ),
            Some(0.0)
        );
    }

    #[test]
    fn unknown_models_do_not_use_family_fallback_when_pricing_store_is_populated() {
        let store = pricing_store(&[(
            "gpt-5",
            ModelPricing {
                input_cost_per_token: Some(1.25e-6),
                output_cost_per_token: Some(1.0e-5),
                cache_read_input_token_cost: Some(1.25e-7),
                ..Default::default()
            },
        )]);

        assert_eq!(
            store.estimate_cost(
                &ToolKind::Codex,
                Some("openrouter/unknown"),
                1_000,
                500,
                200,
                0
            ),
            Some(0.0)
        );
        assert_eq!(
            store.estimate_cost(
                &ToolKind::Claude,
                Some("claude-made-up-preview"),
                1_000,
                500,
                200,
                0,
            ),
            Some(0.0)
        );
    }
}
