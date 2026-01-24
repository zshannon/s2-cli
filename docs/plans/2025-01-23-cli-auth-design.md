# CLI Authorization Support

Support for Biscuit tokens and RFC 9421 HTTP Message Signatures in the s2 CLI.

## Overview

| Feature | Implementation |
|---------|----------------|
| Key generation | New `keygen` command |
| Request signing | SDK (via `with_signing_key()`) |
| Token storage | Config file (`signing_key` + `token`) |
| Offline attenuation | `issue-access-token --public-key` |
| Legacy support | `--id` arg still works |

## Config Schema

**New keys in `~/.config/s2/config.toml`:**

```toml
# Client auth (for regular operations)
signing_key = "base58-p256-private-key"  # ~44 chars
token = "base64-biscuit-token"

# Admin auth (for token management)
root_key = "base58-p256-private-key"

# Existing keys (legacy + endpoints)
access_token = "..."
account_endpoint = "..."
basin_endpoint = "..."
compression = "..."
```

**Environment variables:**
- `S2_SIGNING_KEY`
- `S2_TOKEN`
- `S2_ROOT_KEY`

**Precedence:** New auth (`signing_key` + `token`) wins over legacy (`access_token`).

**Validation:**
- `signing_key` requires `token` (and vice versa)
- `root_key` is independent

## Commands

### `keygen` (new)

Generate a P-256 keypair for request signing.

```bash
s2 keygen
```

**Output:**
```
public_key=2NEpo7TZRRrLZSi2U...
private_key=5HueCGU8rMjxEXxiPuD...
```

Simple key=value format. No flags, no JSON.

### `issue-access-token` (updated)

**New arg:** `--public-key <base58>` for new auth
**Existing arg:** `--id <string>` for legacy servers

```bash
# Legacy server
s2 issue-access-token --id my-token --basins "x/*"

# New server (with root_key configured)
s2 issue-access-token --public-key 2NEpo7TZ... --basins "x/*"

# Offline attenuation (with token configured, no root_key)
s2 issue-access-token --public-key 3ABcd8UV... --basins "x/subset/*" --ops read
```

**Mode detection:**

| `--id` | `--public-key` | `root_key` | `token` | Behavior |
|--------|----------------|------------|---------|----------|
| yes | - | - | - | Legacy server call |
| - | yes | yes | - | New server call |
| - | yes | - | yes | Offline attenuation |
| - | yes | - | - | Error: missing auth |

**Offline attenuation** creates a Biscuit attenuation block with:
- `public_key("<new-client-pubkey>")` fact
- `check if signer($s), $s == "<new-client-pubkey>"` caveat
- Scope restriction caveats from args

## SDK Integration

**Builder pattern:**

```rust
// New auth
S2Config::new(token)
    .with_signing_key(key)

// Legacy auth
S2Config::new(access_token)
```

**CLI config flow:**

```rust
pub fn sdk_config(config: &CliConfig) -> Result<S2Config, CliError> {
    if let (Some(key), Some(token)) = (&config.signing_key, &config.token) {
        Ok(S2Config::new(token).with_signing_key(parse_key(key)?))
    } else if let Some(access_token) = &config.access_token {
        Ok(S2Config::new(access_token))
    } else {
        Err(CliConfigError::MissingAuth.into())
    }
}
```

Commands don't change - SDK handles signing transparently.

## Dependencies

**CLI additions:**
- `p256` - P-256 key generation
- `bs58` - base58 encoding
- `biscuit-auth` - offline attenuation

**SDK additions (separate work):**
- `p256`, `bs58`, `sha2` - signing
- RFC 9421 implementation

## User Flows

### Initial Setup (Admin)

```bash
# Generate root key (one-time)
s2 keygen
# → public_key=ROOT_PUB private_key=ROOT_PRIV

# Configure server with ROOT_PRIV
# Server derives ROOT_PUB for token verification

# Configure CLI for admin operations
s2 config set root_key ROOT_PRIV
```

### Issue Token for Client

```bash
# Client generates their keypair
s2 keygen
# → public_key=CLIENT_PUB private_key=CLIENT_PRIV

# Admin issues token (server call)
s2 issue-access-token --public-key CLIENT_PUB --basins "tenant-a/*" --ops read,append
# → outputs biscuit token

# Client configures
s2 config set signing_key CLIENT_PRIV
s2 config set token <biscuit-token>

# Client can now use all commands
s2 ls s2://tenant-a/
s2 append s2://tenant-a/logs
```

### Delegate Access (Offline)

```bash
# Alice has a token, wants to give Bob limited access
# Bob generates keypair
s2 keygen  # → BOB_PUB, BOB_PRIV

# Alice attenuates her token for Bob (no server call)
s2 issue-access-token --public-key BOB_PUB --basins "tenant-a/shared/*" --ops read
# → outputs attenuated token

# Bob configures
s2 config set signing_key BOB_PRIV
s2 config set token <attenuated-token>

# Bob can only read tenant-a/shared/*
```

## Error Messages

```
Error: Missing authentication.
  Run `s2 config set signing_key <key>` and `s2 config set token <token>`
  Or use legacy auth: `s2 config set access_token <token>`

Error: signing_key configured but token is missing.
  Both signing_key and token are required for signed auth.

Error: Invalid signing key: base58 decode failed
  Ensure the key is a valid base58-encoded P-256 private key (32 bytes).

Error: Cannot issue token: neither root_key nor token configured.
  Configure root_key for server issuance, or token for offline attenuation.
```
