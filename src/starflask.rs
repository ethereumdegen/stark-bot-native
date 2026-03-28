//! Starflask API client — handles session creation, polling, and result parsing.
//!
//! Instead of using the starflask SDK directly, we use raw HTTP so we can
//! control the polling loop and emit progress events to the TUI.

use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use uuid::Uuid;

use crate::config::Config;

/// Progress event emitted during session polling.
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    StatusChange(String),
    LogEntry {
        event_type: String,
        iteration: u64,
        summary: String,
    },
    Error(String),
}

/// Final session result.
#[derive(Debug, Clone)]
pub struct SessionResult {
    pub result: Option<Value>,
    pub result_summary: Option<String>,
}

pub struct StarflaskClient {
    client: Client,
    api_key: String,
    base_url: String,
    pub poll_interval: Duration,
    pub poll_timeout: Duration,
}

impl StarflaskClient {
    pub fn new(config: &Config) -> Result<Self, String> {
        let api_key = config.api_key().ok_or("STARFLASK_API_KEY not set")?;
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            api_key,
            base_url: config.base_url(),
            poll_interval: Duration::from_secs(config.poll_interval_secs),
            poll_timeout: Duration::from_secs(config.poll_timeout_secs),
        })
    }

    /// List agents from the Starflask API.
    pub async fn list_agents(&self) -> Result<Vec<Value>, String> {
        let resp = self.get("/agents").await?;
        resp.as_array()
            .or_else(|| resp.get("agents").and_then(|v| v.as_array()))
            .cloned()
            .ok_or_else(|| "Unexpected agents response format".into())
    }

    /// Create a query session. Returns session ID.
    pub async fn create_session(&self, agent_id: &str, message: &str) -> Result<Uuid, String> {
        let body = serde_json::json!({ "message": message });
        let resp = self.post(&format!("/agents/{}/query", agent_id), body).await?;
        extract_session_id(&resp)
    }

    /// Poll a session until completion, calling progress_fn with updates.
    pub async fn poll_session<F>(
        &self,
        agent_id: &str,
        session_id: &Uuid,
        mut progress_fn: F,
    ) -> Result<SessionResult, String>
    where
        F: FnMut(ProgressEvent),
    {
        let path = format!("/agents/{}/sessions/{}", agent_id, session_id);
        let deadline = tokio::time::Instant::now() + self.poll_timeout;
        let mut seen_log_count: usize = 0;
        let mut last_status = String::new();

        loop {
            tokio::time::sleep(self.poll_interval).await;

            if tokio::time::Instant::now() > deadline {
                return Err(format!("Session timed out after {}s", self.poll_timeout.as_secs()));
            }

            let session = match self.get(&path).await {
                Ok(s) => s,
                Err(e) => {
                    progress_fn(ProgressEvent::Error(format!("Poll error (retrying): {}", e)));
                    continue;
                }
            };

            let status = session.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");

            if status != last_status {
                progress_fn(ProgressEvent::StatusChange(status.to_string()));
                last_status = status.to_string();
            }

            // Emit new log entries
            if let Some(logs) = session.get("logs").and_then(|v| v.as_array()) {
                for entry in logs.iter().skip(seen_log_count) {
                    let event_type = entry.get("event")
                        .or_else(|| entry.get("type"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    if event_type == "heartbeat" || event_type == "delegation_waiting" {
                        continue;
                    }

                    let iteration = entry.get("iteration").and_then(|v| v.as_u64()).unwrap_or(0);
                    let summary = summarize_log_entry(event_type, entry);

                    progress_fn(ProgressEvent::LogEntry {
                        event_type: event_type.to_string(),
                        iteration,
                        summary,
                    });
                }
                seen_log_count = logs.len();
            }

            match status {
                "completed" => {
                    return Ok(SessionResult {
                        result: session.get("result").cloned(),
                        result_summary: session.get("result_summary")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                    });
                }
                "failed" => {
                    let err = session.get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown error");
                    return Err(format!("Session failed: {}", err));
                }
                _ => continue,
            }
        }
    }

    // ── HTTP helpers ──

    async fn post(&self, path: &str, body: Value) -> Result<Value, String> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("POST {}: {}", path, e))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("POST {} returned {}: {}", path, status, body));
        }

        resp.json::<Value>().await.map_err(|e| format!("POST {} parse: {}", path, e))
    }

    async fn get(&self, path: &str) -> Result<Value, String> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client
            .get(&url)
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| format!("GET {}: {}", path, e))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("GET {} returned {}: {}", path, status, body));
        }

        resp.json::<Value>().await.map_err(|e| format!("GET {} parse: {}", path, e))
    }
}

// ── Result parsing (ported from starflask_bridge.rs) ──

/// Extract plain text from a session result.
pub fn parse_text_result(result: &Option<Value>) -> String {
    let Some(value) = result else { return String::new() };

    for key in &["text", "message", "response", "summary"] {
        if let Some(text) = value.get(key).and_then(|v| v.as_str()) {
            return text.to_string();
        }
    }

    if let Some(text) = value.as_str() {
        return text.to_string();
    }

    serde_json::to_string_pretty(value).unwrap_or_default()
}

/// Extract URLs from free-form text.
pub fn extract_urls_from_text(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|word| word.trim_start_matches(|c: char| c == '"' || c == '\'' || c == '<' || c == '[' || c == '('))
        .filter(|word| word.starts_with("http://") || word.starts_with("https://"))
        .map(|url| url.trim_end_matches(|c: char| c == ',' || c == ')' || c == ']' || c == '>' || c == '"' || c == '\'' || c == '}' || c == '{').to_string())
        .collect()
}

/// Extract media URLs from a session result.
pub fn parse_media_result(result: &Option<Value>, result_summary: Option<&str>) -> Vec<String> {
    let Some(value) = result else {
        if let Some(summary) = result_summary {
            let urls = extract_urls_from_text(summary);
            if !urls.is_empty() { return urls; }
        }
        return vec![];
    };

    if let Some(urls) = value.get("urls").and_then(|v| v.as_array()) {
        let extracted: Vec<String> = urls.iter().filter_map(|v| v.as_str().map(String::from)).collect();
        if !extracted.is_empty() { return extracted; }
    }

    if let Some(url) = value.get("url").and_then(|v| v.as_str()) {
        return vec![url.to_string()];
    }

    if let Some(media) = value.get("media").and_then(|v| v.as_array()) {
        let extracted: Vec<String> = media.iter()
            .filter_map(|m| m.get("url").and_then(|v| v.as_str()).map(String::from))
            .collect();
        if !extracted.is_empty() { return extracted; }
    }

    for key in &["text", "message", "response", "summary"] {
        if let Some(text) = value.get(key).and_then(|v| v.as_str()) {
            let urls = extract_urls_from_text(text);
            if !urls.is_empty() { return urls; }
        }
    }

    if let Some(text) = value.as_str() {
        let urls = extract_urls_from_text(text);
        if !urls.is_empty() { return urls; }
    }

    if let Some(summary) = result_summary {
        let urls = extract_urls_from_text(summary);
        if !urls.is_empty() { return urls; }
    }

    vec![]
}

// ── Log entry helpers (ported from command_router.rs) ──

fn extract_session_id(resp: &Value) -> Result<Uuid, String> {
    let id_str = resp.get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("No session id in response: {}", resp))?;
    Uuid::parse_str(id_str)
        .map_err(|e| format!("Invalid session UUID '{}': {}", id_str, e))
}

fn summarize_log_entry(event_type: &str, entry: &Value) -> String {
    match event_type {
        "assistant_tool_calls" => {
            let tool_names = extract_tool_names(entry);
            if tool_names.is_empty() {
                "Calling tools...".to_string()
            } else if tool_names.iter().any(|n| n == "delegate") {
                let target = extract_delegation_target(entry);
                format!("Delegating to {}...", target.unwrap_or_else(|| "subagent".to_string()))
            } else {
                format!("Calling {}...", tool_names.join(", "))
            }
        }
        "tool_start" => "Running tool...".to_string(),
        "tool_results" => {
            let tool_names = extract_tool_names(entry);
            if tool_names.is_empty() {
                "Tool result received".to_string()
            } else if tool_names.iter().any(|n| n == "delegate") {
                "Delegation result received".to_string()
            } else {
                format!("{} completed", tool_names.join(", "))
            }
        }
        "assistant_text" => "Thinking...".to_string(),
        "report_result" => {
            let success = entry.get("success")
                .or_else(|| entry.get("payload").and_then(|p| p.get("success")))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            if success { "Task completed".to_string() } else { "Task failed".to_string() }
        }
        "llm_error" => "AI error occurred".to_string(),
        _ => event_type.to_string(),
    }
}

fn extract_tool_names(entry: &Value) -> Vec<String> {
    let candidates = [
        entry.get("tool_calls"),
        entry.get("payload").and_then(|p| p.get("tool_calls")),
    ];
    for tc in candidates.iter().flatten() {
        if let Some(arr) = tc.as_array() {
            let names: Vec<String> = arr.iter()
                .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(String::from))
                .collect();
            if !names.is_empty() {
                return names;
            }
        }
    }

    if let Some(name) = entry.get("name").and_then(|v| v.as_str()) {
        return vec![name.to_string()];
    }

    vec![]
}

fn extract_delegation_target(entry: &Value) -> Option<String> {
    let candidates = [
        entry.get("tool_calls"),
        entry.get("payload").and_then(|p| p.get("tool_calls")),
    ];
    for tc in candidates.iter().flatten() {
        if let Some(arr) = tc.as_array() {
            for tool in arr {
                if tool.get("name").and_then(|v| v.as_str()) == Some("delegate") {
                    if let Some(args) = tool.get("arguments") {
                        let args_obj = if let Some(s) = args.as_str() {
                            serde_json::from_str::<Value>(s).ok()
                        } else {
                            Some(args.clone())
                        };
                        if let Some(obj) = args_obj {
                            if let Some(name) = obj.get("agent_name").and_then(|v| v.as_str()) {
                                return Some(name.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}
