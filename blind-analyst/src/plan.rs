use anyhow::{bail, Result};
use serde::Deserialize;
use std::{collections::BTreeMap, fs::OpenOptions, io::Write};

use crate::fhe::TokenStore;

const ALLOWED_TOOLS: &[&str] = &["notify", "save_note"];
const MAX_STEPS: usize = 10;

#[derive(Debug, Deserialize)]
pub struct Plan {
    pub steps: Vec<Step>,
}

#[derive(Debug, Deserialize)]
pub struct Step {
    pub tool: String,
    #[serde(default)]
    pub args: BTreeMap<String, String>,
}

/// Parse the agent's reply (tolerates ```json fences). Treated as untrusted.
pub fn parse(raw: &str) -> Result<Plan> {
    let s = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let plan: Plan = serde_json::from_str(s)?;
    if plan.steps.len() > MAX_STEPS {
        bail!("plan too long ({} steps)", plan.steps.len());
    }
    Ok(plan)
}

/// Runs the plan LOCALLY: tokens are decrypted here, never at the provider.
pub fn execute(plan: &Plan, store: &TokenStore, key: &[u8]) -> Result<Vec<String>> {
    let mut log = Vec::new();
    for (i, step) in plan.steps.iter().enumerate() {
        if !ALLOWED_TOOLS.contains(&step.tool.as_str()) {
            bail!("step {}: tool '{}' is not allowed", i + 1, step.tool);
        }
        let raw = step.args.get("text").map(String::as_str).unwrap_or("");
        let text = store.reveal(key, raw)?;
        match step.tool.as_str() {
            "notify" => log.push(format!("NOTIFY: {text}")),
            "save_note" => {
                let mut f = OpenOptions::new().create(true).append(true).open("notes.txt")?;
                writeln!(f, "{text}")?;
                log.push(format!("SAVED: {text}"));
            }
            _ => unreachable!(),
        }
    }
    Ok(log)
}