use anyhow::{anyhow, Result};
use sharks::{Share, Sharks};
use zeroize::Zeroizing;

/// 32-byte master key. Zeroized on drop.
pub fn generate_master_key() -> Result<Zeroizing<[u8; 32]>> {
    let mut key = Zeroizing::new([0u8; 32]);
    getrandom::fill(&mut *key).map_err(|e| anyhow!("rng failure: {e}"))?;
    Ok(key)
}

/// Split `secret` into `n` shares; any `threshold` of them recover it.
pub fn split(secret: &[u8], threshold: u8, n: usize) -> Vec<Vec<u8>> {
    Sharks(threshold)
        .dealer(secret)
        .take(n)
        .map(|s| Vec::from(&s))
        .collect()
}

/// Recombine shares into the secret.
pub fn combine(shares: &[Vec<u8>], threshold: u8) -> Result<Zeroizing<Vec<u8>>> {
    let parsed: Vec<Share> = shares
        .iter()
        .map(|b| Share::try_from(b.as_slice()).map_err(|e| anyhow!("bad share: {e}")))
        .collect::<Result<_>>()?;
    let secret = Sharks(threshold)
        .recover(&parsed)
        .map_err(|e| anyhow!("recover failed: {e}"))?;
    Ok(Zeroizing::new(secret))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_k_of_n_recovers() {
        let key = generate_master_key().unwrap();
        let shares = split(&*key, 3, 5);
        let subset = vec![shares[4].clone(), shares[1].clone(), shares[2].clone()];
        let got = combine(&subset, 3).unwrap();
        assert_eq!(&got[..], &key[..]);
    }

    #[test]
    fn too_few_shares_fails() {
        let key = generate_master_key().unwrap();
        let shares = split(&*key, 3, 5);
        let got = combine(&shares[..2].to_vec(), 3);
        assert!(got.is_err() || &got.unwrap()[..] != &key[..]);
    }
}