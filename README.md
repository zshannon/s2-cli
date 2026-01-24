<div align="center">
  <p>
    <!-- Light mode logo -->
    <a href="https://s2.dev#gh-light-mode-only">
      <img src="./assets/s2-black.png" height="60">
    </a>
    <!-- Dark mode logo -->
    <a href="https://s2.dev#gh-dark-mode-only">
      <img src="./assets/s2-white.png" height="60">
    </a>
  </p>

  <h1>S2 CLI</h1>

  <p>
    <!-- Crates.io -->
    <a href="https://crates.io/crates/s2-cli"><img src="https://img.shields.io/crates/v/s2-cli.svg" /></a>
    <!-- Github Actions (CI) -->
    <a href="https://github.com/s2-streamstore/s2-cli/actions?query=branch%3Amain++"><img src="https://github.com/s2-streamstore/s2-cli/actions/workflows/ci.yml/badge.svg" /></a>
    <!-- Discord (chat) -->
    <a href="https://discord.gg/vTCs7kMkAf"><img src="https://img.shields.io/discord/1209937852528599092?logo=discord" /></a>
    <!-- LICENSE -->
    <a href="./LICENSE"><img src="https://img.shields.io/github/license/s2-streamstore/s2-cli" /></a>
  </p>
</div>

Command Line Interface to interact with the
[S2 API](https://s2.dev/docs/rest/protocol).

## Getting started

1. [Install](#installation) the S2 CLI using your preferred method.

2. Configure authentication (see [Authentication](#authentication) for options):
   ```bash
   # Simple: Use an access token from the web console
   s2 config set access_token <YOUR_ACCESS_TOKEN>
   ```

3. You're ready to run S2 commands!
   ```bash
   s2 list-basins
   ```

Head over to [S2 Docs](https://s2.dev/docs/quickstart) for a quick dive into
using the CLI.

## Authentication

The CLI supports multiple authentication methods, from simple access tokens to
cryptographic request signing.

### Access Token (Legacy)

The simplest method. Generate a token from the [web console](https://s2.dev/dashboard):

```bash
s2 config set access_token <YOUR_ACCESS_TOKEN>
```

### Request Signing (Recommended)

For enhanced security, use RFC 9421 HTTP Message Signatures with Biscuit tokens.
This method cryptographically signs each request with a P-256 keypair.

1. Generate a keypair:
   ```bash
   s2 keygen
   # Output:
   # public_key=<BASE58_PUBLIC_KEY>
   # private_key=<BASE58_PRIVATE_KEY>
   ```

2. Get a token issued with your public key (via web console or existing token).

3. Configure both the token and signing key:
   ```bash
   s2 config set token <YOUR_BISCUIT_TOKEN>
   s2 config set signing_key <YOUR_PRIVATE_KEY>
   ```

Both `token` and `signing_key` must be configured together.

### Root Key (Admin Bootstrap)

For self-hosted deployments, administrators with the root key can operate
without pre-existing tokens. The CLI creates an admin token on-the-fly:

```bash
s2 config set root_key <ROOT_PRIVATE_KEY>
# Optionally set custom endpoints for self-hosted
s2 config set account_endpoint <ACCOUNT_ENDPOINT>
s2 config set basin_endpoint <BASIN_ENDPOINT>
```

### Token Delegation

You can create restricted tokens from an existing token (offline attenuation):

```bash
# Generate a keypair for the delegate
s2 keygen
# public_key=<DELEGATE_PUBLIC_KEY>
# private_key=<DELEGATE_PRIVATE_KEY>

# Issue a restricted token (requires token + signing_key configured)
s2 issue-access-token --public-key <DELEGATE_PUBLIC_KEY> \
  --basins "prefix-" --expires-in 7d --ops read,append
```

### Configuration Reference

| Key | Description |
|-----|-------------|
| `access_token` | Legacy bearer token |
| `token` | Biscuit token (requires `signing_key`) |
| `signing_key` | P-256 private key for request signing (requires `token`) |
| `root_key` | Root private key for admin bootstrap mode |
| `account_endpoint` | Custom account service endpoint |
| `basin_endpoint` | Custom basin service endpoint |
| `compression` | Request compression: `gzip` or `zstd` |

Configuration can also be set via environment variables with `S2_` prefix
(e.g., `S2_TOKEN`, `S2_SIGNING_KEY`).

## Commands and reference

You can add the `--help` flag to any command for CLI reference. Run `s2 --help`
to view all the supported commands and options.

> [!TIP]
> The `--help` command displays a verbose help message whereas the `-h` displays
> the same message in brief.

## Installation

### Using Homebrew

This method works on macOS and Linux distributions with
[Homebrew](https://brew.sh) installed.

```bash
brew install s2-streamstore/s2/s2
```

### Using Cargo

This method works on any system with [Rust](https://www.rust-lang.org/)
and [Cargo](https://doc.rust-lang.org/cargo/) installed.

```bash
cargo install --locked s2-cli
```

### From Release Binaries

Check out the [S2 CLI Releases](https://github.com/s2-streamstore/s2-cli/releases)
for prebuilt binaries for many different architectures and operating systems.

Linux and macOS users can download the release binary using:

```bash
curl -fsSL s2.dev/install.sh | bash
```

To install a specific version, you can set the `VERSION` environment variable.

```bash
export VERSION=0.5.2
curl -fsSL s2.dev/install.sh | bash
```

## Feedback

We use [Github Issues](https://github.com/s2-streamstore/s2-cli/issues) to
track feature requests and issues with the SDK. If you wish to provide feedback,
report a bug or request a feature, feel free to open a Github issue.

### Contributing

Developers are welcome to submit Pull Requests on the repository. If there is
no tracking issue for the bug or feature request corresponding to the PR, we
encourage you to open one for discussion before submitting the PR.

## Reach out to us

Join our [Discord](https://discord.gg/vTCs7kMkAf) server. We would love to hear
from you.

You can also email us at [hi@s2.dev](mailto:hi@s2.dev).

## License

This project is licensed under the [MIT License](./LICENSE).
