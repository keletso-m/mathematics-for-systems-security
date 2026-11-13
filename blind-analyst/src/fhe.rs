use tfhe::prelude::*;
use tfhe::{generate_keys, set_server_key, ClientKey, ConfigBuilder, FheUint32};
pub fn setup() -> ClientKey
pub fn encrypt_column(values: &[u32], ck: &ClientKey) -> Vec<FheUint32>
pub fn sum(col: &[FheUint32]) -> FheUint32
pub fn count_above(col: &[FheUint32], threshold: u32) -> FheUint32
pub fn decrypt(result: &FheUint32, ck: &ClientKey) -> u32