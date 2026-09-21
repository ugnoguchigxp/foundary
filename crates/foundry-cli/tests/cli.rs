use std::fs;

use assert_cmd::Command;
use image::{GenericImageView, ImageBuffer, Rgb, Rgba};
use predicates::prelude::*;
use tempfile::tempdir;

fn foundry() -> Command {
    Command::cargo_bin("foundry").expect("Foundry binary is built for integration tests")
}

fn write_png(path: &std::path::Path) {
    let image = ImageBuffer::from_pixel(32, 24, Rgba([32_u8, 128, 255, 180]));
    image.save(path).expect("fixture PNG is written")
}

fn write_jpeg(path: &std::path::Path) {
    let image = ImageBuffer::from_pixel(19, 17, Rgb([200_u8, 80, 16]));
    image.save(path).expect("fixture JPEG is written");
}

#[test]
fn list_json_describes_webp_capability() {
    let output = foundry().args(["list", "--json"]).output().unwrap();
    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["command"], "list");
    assert_eq!(result["ok"], true);
    assert_eq!(result["data"][0]["id"], "image.webp");
}

#[test]
fn doctor_json_reports_native_image_support() {
    let output = foundry().args(["doctor", "--json"]).output().unwrap();
    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["command"], "doctor");
    assert_eq!(result["data"][0]["status"], "ready");
}

#[test]
fn rust_doctor_reports_toolchain_and_native_tool_checks() {
    let output = foundry()
        .args(["doctor", "rust", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["command"], "doctor.rust");
    assert!(
        result["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| { entry["id"] == "rustup" && entry["kind"] == "rust" })
    );
}

#[test]
fn cache_status_json_is_read_only_inventory() {
    let output = foundry()
        .args(["cache", "status", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["command"], "cache.status");
    assert!(
        result["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| { entry["id"] == "cargo.registry" && entry["path"].is_string() })
    );
}

#[test]
fn rust_environment_is_opt_in_and_does_not_set_a_shared_target_directory() {
    foundry()
        .args(["env", "rust", "--shell", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("SCCACHE_DIR"))
        .stdout(predicate::str::contains("RUSTC_WRAPPER"))
        .stdout(predicate::str::contains("CARGO_TARGET_DIR").not());
}

#[test]
fn rust_setup_writes_only_a_foundry_owned_profile() {
    let directory = tempdir().unwrap();
    let output = foundry()
        .env("FOUNDRY_CONFIG_HOME", directory.path())
        .args(["setup", "rust", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["command"], "setup.rust");
    let profile = directory.path().join("rust-build.zsh");
    assert!(profile.is_file());
    assert!(fs::read_to_string(profile).unwrap().contains("SCCACHE_DIR"));
}

#[test]
fn project_environment_uses_an_isolated_external_target() {
    let directory = tempdir().unwrap();
    fs::write(directory.path().join("Cargo.toml"), "[workspace]\n").unwrap();
    fs::write(
        directory.path().join("foundry-rust.toml"),
        "schema_version = 1\nproject_id = \"fixture\"\ntoolchain = \"1.92.0\"\n\n[[build_units]]\nid = \"workspace\"\nmanifest_path = \"Cargo.toml\"\n",
    )
    .unwrap();
    let cache = directory.path().join("cache");
    let output = foundry()
        .env("FOUNDRY_CACHE_HOME", &cache)
        .args(["env", "rust", "--project"])
        .arg(directory.path())
        .args(["--unit", "workspace"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("CARGO_TARGET_DIR"));
    assert!(stdout.contains("rust-build/targets/fixture/"));
    assert!(stdout.contains("/workspace/aarch64-apple-darwin"));
}

#[test]
fn project_environment_rejects_unsafe_manifest_paths() {
    let directory = tempdir().unwrap();
    fs::write(
        directory.path().join("foundry-rust.toml"),
        "schema_version = 1\nproject_id = \"fixture\"\ntoolchain = \"1.92.0\"\n\n[[build_units]]\nid = \"workspace\"\nmanifest_path = \"../Cargo.toml\"\n",
    )
    .unwrap();
    foundry()
        .args(["env", "rust", "--project"])
        .arg(directory.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsafe manifest path"));
}

#[test]
fn rust_run_applies_the_declared_toolchain_and_isolated_target() {
    let directory = tempdir().unwrap();
    fs::write(directory.path().join("Cargo.toml"), "[workspace]\n").unwrap();
    fs::write(
        directory.path().join("foundry-rust.toml"),
        "schema_version = 1\nproject_id = \"fixture\"\ntoolchain = \"1.92.0\"\n\n[[build_units]]\nid = \"workspace\"\nmanifest_path = \"Cargo.toml\"\n",
    )
    .unwrap();

    foundry()
        .env("FOUNDRY_CACHE_HOME", directory.path().join("cache"))
        .args(["run", "rust", "--project"])
        .arg(directory.path())
        .args(["--unit", "workspace", "--", "/usr/bin/env"])
        .assert()
        .success()
        .stdout(predicate::str::contains("RUSTUP_TOOLCHAIN=1.92.0"))
        .stdout(predicate::str::contains("CARGO_INCREMENTAL=0"))
        .stdout(predicate::str::contains("rust-build/targets/fixture/"));
}

#[test]
fn converts_png_to_webp_and_reports_absolute_paths() {
    let directory = tempdir().unwrap();
    let input = directory.path().join("日本語 source.png");
    let output = directory.path().join("converted.webp");
    write_png(&input);

    let process = foundry()
        .args(["image", "webp"])
        .arg(&input)
        .args(["--output", output.to_str().unwrap(), "--lossless", "--json"])
        .output()
        .unwrap();
    assert!(process.status.success());

    let result: serde_json::Value = serde_json::from_slice(&process.stdout).unwrap();
    assert_eq!(result["ok"], true);
    assert_eq!(result["command"], "image.webp");
    assert_eq!(
        result["data"]["output"],
        output.canonicalize().unwrap().to_string_lossy().as_ref()
    );
    let converted = image::open(&output).expect("output is decodable WebP");
    assert_eq!(converted.dimensions(), (32, 24));
    assert_eq!(
        fs::metadata(&input).unwrap().len(),
        result["data"]["inputBytes"].as_u64().unwrap()
    );
}

#[test]
fn converts_jpeg_to_default_webp_output() {
    let directory = tempdir().unwrap();
    let input = directory.path().join("photo.jpg");
    let output = directory.path().join("photo.webp");
    write_jpeg(&input);

    foundry()
        .args(["image", "webp"])
        .arg(&input)
        .assert()
        .success();

    let converted = image::open(&output).expect("output is decodable WebP");
    assert_eq!(converted.dimensions(), (19, 17));
}

#[test]
fn preserves_existing_output() {
    let directory = tempdir().unwrap();
    let input = directory.path().join("source.png");
    let output = directory.path().join("source.webp");
    write_png(&input);
    fs::write(&output, b"keep me").unwrap();

    foundry()
        .args(["image", "webp"])
        .arg(&input)
        .args(["--json"])
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains("OUTPUT_EXISTS"));
    assert_eq!(fs::read(&output).unwrap(), b"keep me");
}

#[test]
fn reports_missing_input_as_json_failure() {
    let directory = tempdir().unwrap();
    let missing = directory.path().join("missing.png");

    foundry()
        .args(["image", "webp"])
        .arg(missing)
        .args(["--json"])
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains("INPUT_NOT_FOUND"))
        .stderr(predicate::str::is_empty());
}
