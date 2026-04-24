use std::{collections::HashMap, path::PathBuf, time::Duration};

use biscuit_auth::{KeyPair, PrivateKey, builder::{Algorithm, BiscuitBuilder}};
use config::{Config, FileFormat};
use s2_sdk::{
    self as sdk,
    types::{AccountEndpoint, BasinEndpoint, S2Config, S2Endpoints},
};
use serde::{Deserialize, Serialize};

use crate::error::{CliConfigError, CliError};

/// Create an admin Biscuit token on-the-fly using the root key.
/// This enables bootstrap mode where the admin can operate without a pre-existing token.
/// Returns (base64_token, signing_key).
fn create_admin_token(root_key_str: &str) -> Result<(String, sdk::types::SigningKey), CliError> {
    use base64ct::Encoding;
    use p256::ecdsa::SigningKey;

    // Parse root key from base58
    let key_bytes = bs58::decode(root_key_str)
        .into_vec()
        .map_err(|e| CliConfigError::InvalidSigningKey(format!("base58 decode: {}", e)))?;

    if key_bytes.len() != 32 {
        return Err(CliConfigError::InvalidSigningKey(format!(
            "expected 32 bytes, got {}",
            key_bytes.len()
        ))
        .into());
    }

    // Create P-256 signing key
    let p256_key = SigningKey::from_bytes((&key_bytes[..]).into())
        .map_err(|e| CliConfigError::InvalidSigningKey(e.to_string()))?;

    // Derive public key for the token
    let public_key = p256_key.verifying_key();
    let public_key_base58 = bs58::encode(public_key.to_encoded_point(true).as_bytes()).into_string();

    // Create Biscuit keypair from root key
    let biscuit_private = PrivateKey::from_bytes(&key_bytes, Algorithm::Secp256r1)
        .map_err(|e| CliConfigError::InvalidSigningKey(format!("biscuit key: {}", e)))?;
    let biscuit_keypair = KeyPair::from(&biscuit_private);

    // Build admin Biscuit with full permissions
    let mut builder = BiscuitBuilder::new();

    // Bind to root key's public key
    builder = builder
        .fact(format!("public_key(\"{}\")", public_key_base58).as_str())
        .map_err(|e| CliConfigError::InvalidSigningKey(format!("biscuit fact: {}", e)))?;

    // Set expiration (1 hour from now)
    let expires_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600;
    builder = builder
        .fact(format!("expires({})", expires_ts).as_str())
        .map_err(|e| CliConfigError::InvalidSigningKey(format!("biscuit expires: {}", e)))?;
    builder = builder
        .check(format!("check if time($t), $t < {}", expires_ts).as_str())
        .map_err(|e| CliConfigError::InvalidSigningKey(format!("biscuit check: {}", e)))?;

    // Grant full admin permissions (all op_groups read+write)
    builder = builder.fact("op_group(\"account\", \"read\")").unwrap();
    builder = builder.fact("op_group(\"account\", \"write\")").unwrap();
    builder = builder.fact("op_group(\"basin\", \"read\")").unwrap();
    builder = builder.fact("op_group(\"basin\", \"write\")").unwrap();
    builder = builder.fact("op_group(\"stream\", \"read\")").unwrap();
    builder = builder.fact("op_group(\"stream\", \"write\")").unwrap();

    // No resource restrictions (full access)
    builder = builder.fact("basin_scope(\"prefix\", \"\")").unwrap();
    builder = builder.fact("stream_scope(\"prefix\", \"\")").unwrap();
    builder = builder.fact("access_token_scope(\"prefix\", \"\")").unwrap();

    // Build and serialize
    let biscuit = builder
        .build(&biscuit_keypair)
        .map_err(|e| CliConfigError::InvalidSigningKey(format!("biscuit build: {}", e)))?;

    let token_bytes = biscuit
        .to_vec()
        .map_err(|e| CliConfigError::InvalidSigningKey(format!("biscuit serialize: {}", e)))?;
    let token_base64 = base64ct::Base64Url::encode_string(&token_bytes);

    // Create SDK signing key
    let signing_key = sdk::types::SigningKey::from_base58(root_key_str)
        .map_err(|e| CliConfigError::InvalidSigningKey(e.to_string()))?;

    Ok((token_base64, signing_key))
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, strum::Display, strum::EnumString)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum Compression {
    Gzip,
    Zstd,
}

impl From<Compression> for sdk::types::Compression {
    fn from(value: Compression) -> Self {
        match value {
            Compression::Gzip => sdk::types::Compression::Gzip,
            Compression::Zstd => sdk::types::Compression::Zstd,
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CliConfig {
    pub access_token: Option<String>,
    pub signing_key: Option<String>,
    pub token: Option<String>,
    pub root_key: Option<String>,
    pub account_endpoint: Option<String>,
    pub basin_endpoint: Option<String>,
    pub compression: Option<Compression>,
}

#[cfg(target_os = "windows")]
fn config_path() -> Result<PathBuf, CliConfigError> {
    let mut path = dirs::config_dir().ok_or(CliConfigError::DirNotFound)?;
    path.push("s2");
    path.push("config.toml");
    Ok(path)
}

#[cfg(not(target_os = "windows"))]
fn config_path() -> Result<PathBuf, CliConfigError> {
    let mut path = dirs::home_dir().ok_or(CliConfigError::DirNotFound)?;
    path.push(".config");
    path.push("s2");
    path.push("config.toml");
    Ok(path)
}

/// Load the raw TOML as a map of profiles. Falls back to treating a flat
/// (legacy) config as the "default" profile.
fn load_all_profiles() -> Result<HashMap<String, CliConfig>, CliConfigError> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let contents = std::fs::read_to_string(&path).map_err(CliConfigError::Write)?;

    // Try profiled format first (map of sections).
    if let Ok(profiles) = toml::from_str::<HashMap<String, CliConfig>>(&contents) {
        return Ok(profiles);
    }

    // Fall back to flat (legacy) format — treat as "default" profile.
    let config: CliConfig = toml::from_str(&contents)
        .map_err(|e| config::ConfigError::FileParse { uri: Some(path.display().to_string()), cause: Box::new(e) })?;
    let mut map = HashMap::new();
    map.insert("default".to_string(), config);
    Ok(map)
}

pub fn load_config_file(profile: &str) -> Result<CliConfig, CliConfigError> {
    let profiles = load_all_profiles()?;
    Ok(profiles.into_iter()
        .find(|(k, _)| k == profile)
        .map(|(_, v)| v)
        .unwrap_or_default())
}

pub fn load_cli_config(profile: &str) -> Result<CliConfig, CliConfigError> {
    let file_config = load_config_file(profile)?;

    // Serialize the profile config back to TOML, then layer env vars on top
    // via the `config` crate so S2_* env vars override profile values.
    let file_toml = toml::to_string(&file_config).map_err(CliConfigError::Serialize)?;

    let builder = Config::builder()
        .add_source(config::File::from_str(&file_toml, FileFormat::Toml))
        .add_source(config::Environment::with_prefix("S2"));
    Ok(builder.build()?.try_deserialize::<CliConfig>()?)
}

#[derive(
    Debug, Clone, Copy, clap::ValueEnum, strum::Display, strum::EnumString, strum::VariantNames,
)]
#[clap(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ConfigKey {
    AccessToken,
    SigningKey,
    Token,
    RootKey,
    AccountEndpoint,
    BasinEndpoint,
    Compression,
}

impl CliConfig {
    pub fn get(&self, key: ConfigKey) -> Option<String> {
        match key {
            ConfigKey::AccessToken => self.access_token.clone(),
            ConfigKey::SigningKey => self.signing_key.clone(),
            ConfigKey::Token => self.token.clone(),
            ConfigKey::RootKey => self.root_key.clone(),
            ConfigKey::AccountEndpoint => self.account_endpoint.clone(),
            ConfigKey::BasinEndpoint => self.basin_endpoint.clone(),
            ConfigKey::Compression => self.compression.map(|c| c.to_string()),
        }
    }

    pub fn set(&mut self, key: ConfigKey, value: String) -> Result<(), CliConfigError> {
        match key {
            ConfigKey::AccessToken => self.access_token = Some(value),
            ConfigKey::SigningKey => self.signing_key = Some(value),
            ConfigKey::Token => self.token = Some(value),
            ConfigKey::RootKey => self.root_key = Some(value),
            ConfigKey::AccountEndpoint => self.account_endpoint = Some(value),
            ConfigKey::BasinEndpoint => self.basin_endpoint = Some(value),
            ConfigKey::Compression => {
                self.compression = Some(
                    value
                        .parse()
                        .map_err(|_| CliConfigError::InvalidValue(key.to_string(), value))?,
                );
            }
        }
        Ok(())
    }

    pub fn unset(&mut self, key: ConfigKey) {
        match key {
            ConfigKey::AccessToken => self.access_token = None,
            ConfigKey::SigningKey => self.signing_key = None,
            ConfigKey::Token => self.token = None,
            ConfigKey::RootKey => self.root_key = None,
            ConfigKey::AccountEndpoint => self.account_endpoint = None,
            ConfigKey::BasinEndpoint => self.basin_endpoint = None,
            ConfigKey::Compression => self.compression = None,
        }
    }
}

pub fn save_cli_config(config: &CliConfig, profile: &str) -> Result<PathBuf, CliConfigError> {
    let path = config_path()?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(CliConfigError::Write)?;
    }

    let mut profiles = load_all_profiles().unwrap_or_default();
    profiles.insert(profile.to_string(), config.clone());

    let toml = toml::to_string(&profiles).map_err(CliConfigError::Serialize)?;
    std::fs::write(&path, toml).map_err(CliConfigError::Write)?;

    Ok(path)
}

pub fn set_config_value(key: ConfigKey, value: String, profile: &str) -> Result<PathBuf, CliConfigError> {
    let mut config = load_config_file(profile).unwrap_or_default();
    config.set(key, value)?;
    save_cli_config(&config, profile)
}

pub fn unset_config_value(key: ConfigKey, profile: &str) -> Result<PathBuf, CliConfigError> {
    let mut config = load_config_file(profile).unwrap_or_default();
    config.unset(key);
    save_cli_config(&config, profile)
}

pub fn sdk_config(config: &CliConfig) -> Result<S2Config, CliError> {
    // Determine auth mode:
    // 1. root_key alone -> create admin Biscuit on-the-fly (bootstrap mode)
    // 2. token + signing_key -> new auth
    // 3. access_token -> legacy auth

    let compression: sdk::types::Compression = config
        .compression
        .map(Into::into)
        .unwrap_or(sdk::types::Compression::None);

    // Root key bootstrap mode: create admin Biscuit on-the-fly
    if let Some(ref root_key_str) = config.root_key {
        if config.token.is_none() && config.access_token.is_none() {
            let (admin_token, signing_key) = create_admin_token(root_key_str)?;

            let mut sdk_config = S2Config::new(&admin_token)
                .with_user_agent("s2-cli")
                .map_err(|e| CliError::EndpointsFromEnv(e.to_string()))?
                .with_request_timeout(Duration::from_secs(30))
                .with_compression(compression)
                .with_signing_key(signing_key);

            if let (Some(account), Some(basin)) = (&config.account_endpoint, &config.basin_endpoint) {
                let account_endpoint = AccountEndpoint::new(account)
                    .map_err(|e| CliError::EndpointsFromEnv(e.to_string()))?;
                let basin_endpoint = BasinEndpoint::new(basin)
                    .map_err(|e| CliError::EndpointsFromEnv(e.to_string()))?;
                let endpoints = S2Endpoints::new(account_endpoint, basin_endpoint)
                    .map_err(|e| CliError::EndpointsFromEnv(e.to_string()))?;
                sdk_config = sdk_config.with_endpoints(endpoints);
            }

            return Ok(sdk_config);
        }
    }

    // Validate signing_key + token pairing (both required together for new auth)
    match (&config.signing_key, &config.token) {
        (Some(_), None) => {
            return Err(CliConfigError::MissingToken.into());
        }
        (None, Some(_)) => {
            return Err(CliConfigError::MissingSigningKey.into());
        }
        _ => {}
    }

    // New auth: token + signing_key; Legacy: access_token
    let bearer_token = config
        .token
        .as_ref()
        .or(config.access_token.as_ref())
        .ok_or(CliConfigError::MissingAccessToken)?;

    let mut sdk_config = S2Config::new(bearer_token)
        .with_user_agent("s2-cli")
        .map_err(|e| CliError::EndpointsFromEnv(e.to_string()))?
        .with_request_timeout(Duration::from_secs(30))
        .with_compression(compression);

    // Add signing key if configured (enables RFC 9421 request signing)
    if let Some(ref signing_key_str) = config.signing_key {
        let signing_key = sdk::types::SigningKey::from_base58(signing_key_str)
            .map_err(|e| CliConfigError::InvalidSigningKey(e.to_string()))?;
        sdk_config = sdk_config.with_signing_key(signing_key);
    }

    match (&config.account_endpoint, &config.basin_endpoint) {
        (Some(account), Some(basin)) => {
            let account_endpoint = AccountEndpoint::new(account)
                .map_err(|e| CliError::EndpointsFromEnv(e.to_string()))?;
            let basin_endpoint = BasinEndpoint::new(basin)
                .map_err(|e| CliError::EndpointsFromEnv(e.to_string()))?;
            let endpoints = S2Endpoints::new(account_endpoint, basin_endpoint)
                .map_err(|e| CliError::EndpointsFromEnv(e.to_string()))?;
            sdk_config = sdk_config.with_endpoints(endpoints);
        }
        (Some(_), None) => {
            eprintln!(
                "Warning: account endpoint is set but basin endpoint is not. \
                 Both must be set to use custom endpoints. Using default endpoints"
            );
        }
        (None, Some(_)) => {
            eprintln!(
                "Warning: basin endpoint is set but account endpoint is not. \
                 Both must be set to use custom endpoints. Using default endpoints"
            );
        }
        (None, None) => {}
    }

    Ok(sdk_config)
}
