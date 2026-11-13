mod agent;
mod fhe;
mod plan;
mod shares;
mod tui;

use anyhow::Result;

fn main() -> Result<()> {
    // 1. Master key exists only briefly, then is split. 3-of-5.
    let key = shares::generate_master_key()?;
    let key_shares = shares::split(&*key, 3, 5);

    // 2. Redact sensitive data and seal it.
    let mut store = fhe::TokenStore::new();
    let prompt = "Summarise the payment from Alice, card 4111-1111-1111-1111";
    let redacted = store.redact(&*key, prompt, &["Alice", "4111-1111-1111-1111"])?;
    drop(key); // agent side never gets the key

    println!("Sent to provider : {redacted}");
    for (t, hex) in store.ciphertext_hex() {
        println!("  {t} -> {}...", &hex[..24]);
    }

    // 3. Later, locally: 3 share-holders approve, key is rebuilt.
    let rebuilt = shares::combine(&key_shares[..3].to_vec(), 3)?;
    println!("Revealed locally : {}", store.reveal(&rebuilt, &redacted)?);
    Ok(())
}
