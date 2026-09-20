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
