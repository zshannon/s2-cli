use assert_cmd::Command;
use predicates::prelude::*;
use serial_test::serial;

fn s2() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("s2"))
}

#[test]
fn invalid_uri_scheme() {
    s2().args(["get-stream-config", "foo://invalid/stream"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("s2://"));
}

#[test]
fn missing_stream_in_uri() {
    s2().args(["get-stream-config", "s2://basin-only"])
        .assert()
        .failure();
}

#[test]
fn invalid_basin_name() {
    s2().args(["create-basin", "-invalid-name"])
        .assert()
        .failure();
}

#[test]
#[serial]
fn missing_access_token() {
    // Clear any config values that might interfere
    let _ = s2().args(["config", "unset", "signing_key"]).assert();
    let _ = s2().args(["config", "unset", "token"]).assert();
    let _ = s2().args(["config", "unset", "access_token"]).assert();

    let mut cmd = s2();
    cmd.env_remove("S2_ACCESS_TOKEN");
    cmd.env_remove("S2_TOKEN");
    cmd.env_remove("S2_SIGNING_KEY");
    cmd.args(["list-basins"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("access token"));
}

#[test]
fn unknown_subcommand() {
    s2().args(["unknown-command"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
#[serial]
fn config_list() {
    s2().args(["config", "list"]).assert().success();
}

#[test]
#[serial]
fn config_set_and_get() {
    s2().args(["config", "set", "compression", "zstd"])
        .assert()
        .success();
    s2().args(["config", "get", "compression"])
        .assert()
        .success()
        .stdout(predicate::str::contains("zstd"));
    s2().args(["config", "unset", "compression"])
        .assert()
        .success();
}

#[test]
#[serial]
fn config_get_invalid_key() {
    s2().args(["config", "get", "invalid_key"])
        .assert()
        .failure();
}

#[test]
#[serial]
fn config_set_invalid_key() {
    s2().args(["config", "set", "invalid_key", "value"])
        .assert()
        .failure();
}

#[test]
fn keygen_outputs_keypair() {
    s2().args(["keygen"])
        .assert()
        .success()
        .stdout(predicate::str::contains("public_key="))
        .stdout(predicate::str::contains("private_key="));
}

#[test]
fn keygen_produces_valid_base58() {
    let output = s2()
        .args(["keygen"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8(output).unwrap();

    // Extract public_key value
    let public_key = output_str
        .lines()
        .find(|l| l.starts_with("public_key="))
        .unwrap()
        .strip_prefix("public_key=")
        .unwrap();

    // Extract private_key value
    let private_key = output_str
        .lines()
        .find(|l| l.starts_with("private_key="))
        .unwrap()
        .strip_prefix("private_key=")
        .unwrap();

    // Both should be valid base58
    assert!(bs58::decode(public_key).into_vec().is_ok(), "public_key should be valid base58");
    assert!(bs58::decode(private_key).into_vec().is_ok(), "private_key should be valid base58");

    // Public key should be 33 bytes (compressed P-256)
    let pub_bytes = bs58::decode(public_key).into_vec().unwrap();
    assert_eq!(pub_bytes.len(), 33, "public_key should be 33 bytes");

    // Private key should be 32 bytes (P-256 scalar)
    let priv_bytes = bs58::decode(private_key).into_vec().unwrap();
    assert_eq!(priv_bytes.len(), 32, "private_key should be 32 bytes");
}

#[test]
#[serial]
fn config_set_signing_key() {
    // Generate a key first
    let output = s2()
        .args(["keygen"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8(output).unwrap();
    let private_key = output_str
        .lines()
        .find(|l| l.starts_with("private_key="))
        .unwrap()
        .strip_prefix("private_key=")
        .unwrap();

    // Set and get signing_key
    s2().args(["config", "set", "signing_key", private_key])
        .assert()
        .success();
    s2().args(["config", "get", "signing_key"])
        .assert()
        .success()
        .stdout(predicate::str::contains(private_key));
    s2().args(["config", "unset", "signing_key"])
        .assert()
        .success();
}

#[test]
#[serial]
fn config_set_token() {
    s2().args(["config", "set", "token", "test-biscuit-token"])
        .assert()
        .success();
    s2().args(["config", "get", "token"])
        .assert()
        .success()
        .stdout(predicate::str::contains("test-biscuit-token"));
    s2().args(["config", "unset", "token"])
        .assert()
        .success();
}

#[test]
#[serial]
fn config_set_root_key() {
    // Generate a key first
    let output = s2()
        .args(["keygen"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8(output).unwrap();
    let private_key = output_str
        .lines()
        .find(|l| l.starts_with("private_key="))
        .unwrap()
        .strip_prefix("private_key=")
        .unwrap();

    // Set and get root_key
    s2().args(["config", "set", "root_key", private_key])
        .assert()
        .success();
    s2().args(["config", "get", "root_key"])
        .assert()
        .success()
        .stdout(predicate::str::contains(private_key));
    s2().args(["config", "unset", "root_key"])
        .assert()
        .success();
}

#[test]
fn issue_access_token_requires_id_or_public_key() {
    let mut cmd = s2();
    cmd.env("S2_ACCESS_TOKEN", "fake-token");
    cmd.args(["issue-access-token"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--id").or(predicate::str::contains("--public-key")));
}

#[test]
#[serial]
fn signing_key_without_token_fails() {
    // Clear any config values that might interfere
    let _ = s2().args(["config", "unset", "signing_key"]).assert();
    let _ = s2().args(["config", "unset", "token"]).assert();
    let _ = s2().args(["config", "unset", "access_token"]).assert();

    // Generate a valid signing key
    let output = s2()
        .args(["keygen"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let output_str = String::from_utf8(output).unwrap();
    let private_key = output_str
        .lines()
        .find(|l| l.starts_with("private_key="))
        .unwrap()
        .strip_prefix("private_key=")
        .unwrap();

    let mut cmd = s2();
    cmd.env_remove("S2_ACCESS_TOKEN");
    cmd.env_remove("S2_TOKEN");
    cmd.env("S2_SIGNING_KEY", private_key);
    cmd.args(["list-basins"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("token is missing"));
}

#[test]
#[serial]
fn token_without_signing_key_fails() {
    // Clear any config values that might interfere
    let _ = s2().args(["config", "unset", "signing_key"]).assert();
    let _ = s2().args(["config", "unset", "token"]).assert();
    let _ = s2().args(["config", "unset", "access_token"]).assert();

    let mut cmd = s2();
    cmd.env_remove("S2_ACCESS_TOKEN");
    cmd.env_remove("S2_SIGNING_KEY");
    cmd.env("S2_TOKEN", "fake-biscuit-token");
    cmd.args(["list-basins"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("signing_key is missing"));
}
