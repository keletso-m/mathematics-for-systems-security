use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

pub const SYSTEM_PROMPT: &str = r#"You are a planning agent. The user's request contains opaque tokens like {{SECRET_1}}. Never guess or invent their values; copy tokens verbatim when you need them.
Respond ONLY with JSON, no prose, in this shape:
{"steps":[{"tool":"notify","args":{"text":"..."}},{"tool":"save_note","args":{"text":"..."}}]}
Allowed tools: notify, save_note."#;

/// Sends ONLY redacted text to the provider. Picks the provider from env vars.
pub async fn ask(redacted_prompt: &str) -> Result<String> {
        if std::env::var("MOCK").is_ok() {
        let plan = json!({"steps":[
            {"tool":"notify","args":{"text": format!("Finance team notified: {redacted_prompt}")}},
            {"tool":"save_note","args":{"text": redacted_prompt}}
        ]});
        return Ok(plan.to_string());
    }
    let client = reqwest::Client::new();
    if let Ok(key) = std::env::var("GEMINI_API_KEY") {
        return gemini(&client, &key, redacted_prompt).await;
    }
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        return openai(&client, &key, redacted_prompt).await;
    }
    bail!("set GEMINI_API_KEY or OPENAI_API_KEY")
}

async fn send(req: reqwest::RequestBuilder) -> Result<Value> {
    let mut delay: u64 = 2;
    for attempt in 1..=5 {
        let r = req.try_clone().ok_or_else(|| anyhow!("request not clonable"))?;
        let resp = r.send().await.context("request failed")?;
        let status = resp.status();
        let body: Value = resp.json().await.context("response was not JSON")?;
        if status.is_success() {
            return Ok(body);
        }
        if (status.as_u16() == 503 || status.as_u16() == 429) && attempt < 5 {
            eprintln!("provider busy ({status}), retrying in {delay}s...");
            tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
            delay *= 2;
            continue;
        }
        bail!("provider returned {status}: {body}");
    }
    unreachable!()
}

async fn gemini(c: &reqwest::Client, key: &str, user: &str) -> Result<String> {
    let model = std::env::var("LLM_MODEL").unwrap_or_else(|_| "gemini-3.8-flash".into());
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent");
    let body = json!({
        "system_instruction": { "parts": [{ "text": SYSTEM_PROMPT }] },
        "contents": [{ "role": "user", "parts": [{ "text": user }] }],
        "generationConfig": { "responseMimeType": "application/json" }
    });
    let v = send(c.post(url).header("x-goog-api-key", key).json(&body)).await?;
    v["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| anyhow!("unexpected Gemini response: {v}"))
}

async fn openai(c: &reqwest::Client, key: &str, user: &str) -> Result<String> {
    let model = std::env::var("LLM_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());
    let body = json!({
        "model": model,
        "response_format": { "type": "json_object" },
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": user }
        ]
    });
    let v = send(c.post("https://api.openai.com/v1/chat/completions").bearer_auth(key).json(&body)).await?;
    v["choices"][0]["message"]["content"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| anyhow!("unexpected OpenAI response: {v}"))
}