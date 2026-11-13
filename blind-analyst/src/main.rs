mod agent;
mod fhe;
mod plan;
mod shares;
mod tui;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Generate master key, split 3-of-5, redact secrets.
    let key = shares::generate_master_key()?;
    let key_shares = shares::split(&*key, 3, 5);

    let mut store = fhe::TokenStore::new();
    let prompt = "Write a short summary of the payment from Alice, card 4111-1111-1111-1111, \
                  notify the finance team and save a note.";
    let redacted = store.redact(&*key, prompt, &["Alice", "4111-1111-1111-1111"])?;
    drop(key); // the key is gone; only shares remain

    // 2. Agent sees ONLY this.
    println!("--> to provider: {redacted}\n");
    let reply = agent::ask(&redacted).await?;
    println!("<-- from agent : {reply}\n");
    let plan = plan::parse(&reply)?;

    // 3. Local execution: 3 share-holders approve, key is rebuilt briefly.
    let rebuilt = shares::combine(&key_shares[..3].to_vec(), 3)?;
    for line in plan::execute(&plan, &store, &rebuilt)? {
        println!("{line}");
    }
    Ok(())
}