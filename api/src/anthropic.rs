//! Anthropic Claude API client — direct REST calls over `reqwest`.
//!
//! The crate has no official Rust SDK; we hit `/v1/messages` directly with
//! the same shape the box-fraise-platform integrations crate uses. Two
//! operations: `summarise_commit` (one-shot text completion) and
//! `recommend_next_action` (JSON-mode response, parsed into `Recommendation`).

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use regie_shared::types::{
    CommitSummary, DeferredItem, DeferredItemPriority, Recommendation,
};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

const BASE: &str = "https://api.anthropic.com/v1/messages";
const MODEL: &str = "claude-sonnet-4-6";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicClient {
    client: reqwest::Client,
    api_key: String,
}

impl AnthropicClient {
    pub fn new(api_key: String) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("regie/0.1")
            .build()
            .expect("reqwest client is infallible");
        Self { client, api_key }
    }

    /// Summarise one commit message in a single sentence suitable for a
    /// dashboard list item. Returns the trimmed text content; never empty
    /// on success (Anthropic always emits at least one text block).
    pub async fn summarise_commit(
        &self,
        commit_message: &str,
        repo_name: &str,
    ) -> Result<String> {
        let prompt = format!(
            "You are an engineering intelligence assistant for the {repo_name} \
             repository. Summarise this git commit message in one plain-English \
             sentence that explains what changed and why it matters to an \
             engineer reading a dashboard. Be specific and concise. Do not \
             start with 'This commit'. Max 20 words.\n\n\
             Commit message:\n{commit_message}"
        );
        let text = self.call(100, None, &prompt).await?;
        Ok(text.trim().to_owned())
    }

    /// Ask Claude what the highest-leverage next action is given the current
    /// repo state. Returns a fully-formed `Recommendation` (repo_id +
    /// generated_at filled in here, Claude returns just the four content
    /// fields as JSON). The system prompt forbids markdown so the response
    /// parses without preamble-stripping.
    pub async fn recommend_next_action(
        &self,
        repo_name: &str,
        current_grade: &str,
        deferred_items: &[DeferredItem],
        recent_commits: &[CommitSummary],
        claude_md_context: Option<&str>,
        readme_context: Option<&str>,
    ) -> Result<Recommendation> {
        let mut prompt = String::new();
        let _ = writeln!(prompt, "Repository: {repo_name}");
        let _ = writeln!(prompt, "Current grade: {current_grade}");
        let _ = writeln!(prompt);

        let _ = writeln!(prompt, "Top deferred items:");
        if deferred_items.is_empty() {
            let _ = writeln!(prompt, "(none)");
        } else {
            for item in deferred_items.iter().take(5) {
                let prio = match item.priority {
                    DeferredItemPriority::High => "HIGH",
                    DeferredItemPriority::Medium => "MEDIUM",
                    DeferredItemPriority::Low => "LOW",
                };
                let _ = writeln!(
                    prompt,
                    "- [{prio}] (id: {}) {}",
                    item.id, item.description
                );
            }
        }
        let _ = writeln!(prompt);

        let _ = writeln!(prompt, "Recent commits:");
        if recent_commits.is_empty() {
            let _ = writeln!(prompt, "(none)");
        } else {
            for c in recent_commits.iter().take(5) {
                let short_sha: String = c.sha.chars().take(7).collect();
                let first_line = c.message.lines().next().unwrap_or("");
                let summary = c.semantic_summary.as_deref().unwrap_or("");
                let _ = writeln!(prompt, "- {short_sha}: {first_line} | {summary}");
            }
        }

        if let Some(ctx) = claude_md_context {
            let _ = writeln!(prompt);
            let _ = writeln!(prompt, "CLAUDE.md context (excerpt):\n{ctx}");
        }

        if let Some(readme) = readme_context {
            // Caller has already capped to ~300 chars on a UTF-8 boundary;
            // slice defensively here in case a future caller skips that.
            let truncated: String = readme.chars().take(300).collect();
            let _ = writeln!(prompt);
            let _ = writeln!(prompt, "README (first 300 chars):\n{truncated}");
        }

        let _ = writeln!(prompt);
        let _ = writeln!(
            prompt,
            "What single action would most improve this repository's grade right now?"
        );
        let _ = writeln!(prompt);
        let _ = writeln!(prompt, "Respond with JSON of this exact shape:");
        let _ = writeln!(
            prompt,
            r#"{{"top_action": "string", "justification": "string", "estimated_impact": "string", "related_deferred_items": ["id1", "id2"]}}"#
        );

        let raw = self
            .call(
                1024,
                Some(
                    "Respond only with valid JSON. No preamble, no markdown, no explanation.",
                ),
                &prompt,
            )
            .await?;

        let parsed: RecommendationJson = serde_json::from_str(raw.trim())
            .with_context(|| format!("parse Claude JSON response: {raw}"))?;

        Ok(Recommendation {
            repo_id: repo_name.to_owned(),
            generated_at: Utc::now(),
            top_action: parsed.top_action,
            justification: parsed.justification,
            estimated_impact: parsed.estimated_impact,
            related_deferred_items: parsed.related_deferred_items,
        })
    }

    async fn call(
        &self,
        max_tokens: u32,
        system: Option<&str>,
        user: &str,
    ) -> Result<String> {
        let body = ClaudeRequest {
            model: MODEL,
            max_tokens,
            system,
            messages: vec![ClaudeMessage {
                role: "user",
                content: user,
            }],
        };
        let resp = self
            .client
            .post(BASE)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Anthropic request failed")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Anthropic returned {status}: {text}"));
        }
        let parsed: ClaudeResponse =
            resp.json().await.context("invalid Anthropic JSON")?;
        let text = parsed
            .content
            .into_iter()
            .filter(|b| b.block_type == "text")
            .map(|b| b.text)
            .collect::<Vec<_>>()
            .join("");
        Ok(text)
    }
}

#[derive(Serialize)]
struct ClaudeRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    messages: Vec<ClaudeMessage<'a>>,
}

#[derive(Serialize)]
struct ClaudeMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContent>,
}

#[derive(Deserialize)]
struct ClaudeContent {
    #[serde(rename = "type")]
    block_type: String,
    text: String,
}

#[derive(Deserialize)]
struct RecommendationJson {
    top_action: String,
    justification: String,
    estimated_impact: String,
    related_deferred_items: Vec<String>,
}
