# SDK Auth Requirements

Memo for the SDK team on changes needed to support Biscuit + RFC 9421 auth.

## What We Need

The CLI needs to pass a P-256 signing key to the SDK, and the SDK signs every HTTP request per RFC 9421.

## API Change

Add a builder method to `S2Config`:

```rust
// Current
let config = S2Config::new(token);

// New - same entry, optional signing key
let config = S2Config::new(token)
    .with_signing_key(key);  // ← new method
```

When `signing_key` is present, SDK signs requests. When absent, legacy bearer-only mode.

## What the SDK Does Per Request

When `signing_key` is configured:

1. **Compute body digest** (if body present):
   ```
   Content-Digest: sha-256=:base64(sha256(body)):
   ```

2. **Build signature base string** per RFC 9421:
   ```
   "@method": POST
   "@path": /basins/foo/streams/bar/records
   "@authority": s2.example.com
   "authorization": Bearer <token>
   "content-digest": sha-256=:...:
   "@signature-params": ("@method" "@path" "@authority" "authorization" "content-digest");created=1704067200;keyid="<pubkey-base58>"
   ```

3. **Sign with P-256** (ECDSA over the base string)

4. **Add headers**:
   ```
   Authorization: Bearer <token>
   Content-Digest: sha-256=:...:  (if body)
   Signature-Input: sig1=("@method" "@path" "@authority" "authorization" "content-digest");created=<unix-ts>;keyid="<pubkey-base58>"
   Signature: sig1=:<base64-ecdsa-signature>:
   ```

## Key Types

```rust
// P-256 private key: 32-byte scalar, base58 encoded (~44 chars)
// CLI parses and passes to SDK as:
pub struct SigningKey(p256::ecdsa::SigningKey);

impl SigningKey {
    pub fn from_base58(s: &str) -> Result<Self, Error>;
    pub fn public_key_base58(&self) -> String;  // for keyid
}
```

## Dependencies to Add

```toml
p256 = { version = "0.13", features = ["ecdsa"] }
bs58 = "0.5"
sha2 = "0.10"
base64ct = "1.6"
```

For RFC 9421 parsing/formatting, either:
- `httpsig` crate (if it fits)
- Hand-roll the structured field formatting (it's not complex)

## Covered Components

Always sign these (required by server):
- `@method`
- `@path`
- `@authority`
- `authorization`
- `content-digest` (when body present)

## Timestamp

`created` parameter = current Unix timestamp. Server allows ±5 minute window.

## Error Handling

New error variant for signing failures:

```rust
pub enum S2Error {
    // ... existing
    SigningError(String),
}
```

## Testing

1. Unit test: signature base string construction matches RFC 9421 examples
2. Unit test: P-256 signature verifies with public key
3. Integration test: signed request accepted by server with auth enabled

## Questions?

Ping me if anything's unclear. The RFC 9421 spec is at https://www.rfc-editor.org/rfc/rfc9421.html - Section 2 (Signature Base) and Section 3 (Signing) are the relevant parts.
