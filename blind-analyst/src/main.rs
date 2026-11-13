mod agent;
mod detect;
mod fhe;
mod plan;
mod shares;
mod tui;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let key = shares::generate_master_key()?;
    let key_shares = shares::split(&*key, 3, 5);

    let prompt = "Write a short summary of the payment from Alice Smith, card 4111-1111-1111-1111, \
                  email alice@example.com, phone 082 123 4567. Notify the finance team and save a note.";

    // Auto-detect secrets, encrypt them, replace with tokens.
    let found = detect::find(prompt);
    println!("Detected {} secret(s):", found.len());
    for f in &found {
        println!("  [{}] {}", f.kind, f.value);
    }
    let secrets: Vec<&str> = found.iter().map(|f| f.value.as_str()).collect();

    let mut store = fhe::TokenStore::new();
    let redacted = store.redact(&*key, prompt, &secrets)?;
    drop(key);

    let reply = agent::ask(&redacted).await?;
    let plan = plan::parse(&reply)?;

    tui::run(tui::Session {
        redacted,
        plan,
        store,
        shares: key_shares,
        threshold: 3,
    })
}