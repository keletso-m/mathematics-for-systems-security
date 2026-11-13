mod agent;
mod detect;
mod fhe;
mod plan;
mod shares;
mod tui;

use anyhow::Result;

async fn run_once(prompt: &str) -> Result<()> {
    // Fresh master key per request, split 3-of-5, then dropped.
    let key = shares::generate_master_key()?;
    let key_shares = shares::split(&*key, 3, 5);

    let found = detect::find(prompt);
    let secrets: Vec<&str> = found.iter().map(|f| f.value.as_str()).collect();

    let mut store = fhe::TokenStore::new();
    let redacted = store.redact(&*key, prompt, &secrets)?;
    drop(key);

    println!("Asking the agent (it only sees tokens)...");
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

#[tokio::main]
async fn main() -> Result<()> {
    while let Some(prompt) = tui::prompt_screen()? {
        if let Err(e) = run_once(&prompt).await {
            eprintln!("\nError: {e:#}\nPress Enter to continue...");
            std::io::stdin().read_line(&mut String::new())?;
        }
    }
    Ok(())
}