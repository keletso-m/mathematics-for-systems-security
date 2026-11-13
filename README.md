# blind-analyst

An LLM agent pipeline where the **host provider (Gemini / OpenAI) never sees sensitive data and never holds the key**.

Sensitive fields are encrypted locally and replaced with opaque tokens before anything leaves your machine. The master key is split with Shamir's Secret Sharing, so no single party (including the agent) can decrypt anything. The agent returns a plan that references tokens; a local executor rebuilds the key only after enough share-holders approve, runs the plan, then wipes the key.

## How it works

```
 your machine                                        host provider
 ------------                                        -------------
 prompt: "pay Alice, card 4111..."
        |
        v
 [fhe.rs]  encrypt secrets (ChaCha20-Poly1305)
           "Alice" -> {{SECRET_1}}
        |
 [shares.rs] master key -> 5 Shamir shares (3 needed)
             key dropped from memory
        |
        |   "pay {{SECRET_1}}, card {{SECRET_2}}"
        +----------------------------------------->  [agent.rs] LLM
                                                       plans using tokens only
        |   {"steps":[{"tool":"notify",...}]}          |
        <----------------------------------------------+
        |
 [plan.rs] validate plan (allow-listed tools, step cap)
        |
 [tui.rs]  holders approve shares (need 3 of 5)
        |
 key rebuilt -> tokens revealed -> tools run locally -> key zeroized
```

The provider only ever sees tokens and the plan structure. It never sees plaintext secrets or key material.

## Project layout

| File | Purpose |
|---|---|
| `src/shares.rs` | Master key generation, Shamir split/combine (k-of-n) |
| `src/fhe.rs` | Authenticated encryption and the token store (`redact` / `reveal`) |
| `src/agent.rs` | LLM client (Gemini, OpenAI), retry on 429/503, `MOCK` mode |
| `src/plan.rs` | Parse the agent's JSON plan, validate, execute locally |
| `src/tui.rs` | Terminal UI: provider view, agent plan, local-only view, share approvals |
| `src/main.rs` | Wires the pipeline together |

## Run it

```bash
# no API key needed
MOCK=1 cargo run

# live model
export GEMINI_API_KEY="..."        # or OPENAI_API_KEY
export LLM_MODEL="gemini-3.8-flash" # optional override
cargo run

# tests
cargo test
```

TUI keys: `1`-`5` toggle share-holder approval, `Enter` execute, `q` quit.

Demo flow: press `Enter` with 0 approvals (refused), approve 2 holders (still refused), approve a third (key rebuilt, plan runs, plaintext appears only in the LOCAL ONLY pane).

## Security properties

- Provider-side data contains only tokens and structure, never plaintext secrets.
- The master key is never held by the agent or sent anywhere; it exists only briefly during local execution.
- Any 3 of 5 shares recover the key; 2 or fewer reveal nothing.
- Ciphertexts are authenticated: tampering is detected on decrypt.
- The agent's output is treated as untrusted: only allow-listed tools run, and plans are capped at 10 steps.
- Key material is zeroized on drop (`zeroize`).

## Limitations (honest version)

- **This is not fully homomorphic encryption.** `fhe.rs` uses authenticated symmetric encryption plus tokenization. A hosted LLM cannot compute on ciphertext, so secrets are replaced by placeholders and resolved locally. The file name is a placeholder for a possible FHE extension.
- Secrets to hide are currently listed by hand; there is no automatic detection.
- The provider can still see the non-secret context of the prompt.
- All 5 shares live in one process for the demo. They are not distributed to real separate holders.
- Shamir shares are not verifiable: a malicious holder could submit a bad share.
- The plaintext prompt exists in local memory before redaction.

## Roadmap (TODO)

- [ ] Run and record the demo against a live model
- [ ] AWS Bedrock provider in `agent.rs`
- [ ] Automatic secret detection (regex/NER) instead of a hand-written list
- [ ] Interactive prompt input in the TUI
- [ ] Distribute shares to separate holders (files, devices, or network)
- [ ] Verifiable secret sharing
- [ ] Small real FHE demo with `tfhe-rs` (e.g. homomorphic sum over encrypted numbers)
- [ ] More tools beyond `notify` and `save_note`
- [ ] Unit tests for `plan.rs`
- [ ] Screenshot or GIF of the TUI

## Notes

- `notes.txt` is written in plaintext by the `save_note` tool and is git-ignored.
- Never commit API keys. Keep them in environment variables.
