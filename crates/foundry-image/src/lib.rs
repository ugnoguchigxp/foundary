use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use image::ImageReader;
use serde::Serialize;
use thiserror::Error;

static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy)]
pub struct EncodeOptions {
    pub lossless: bool,
    pub quality: Option<f32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversion {
    pub input: PathBuf,
    pub output: PathBuf,
    pub input_bytes: u64,
    pub output_bytes: u64,
}

#[derive(Debug, Error)]
pub enum ImageError {
    #[error("input file does not exist: {}", .0.display())]
    InputNotFound(PathBuf),
    #[error("only JPG, JPEG, and PNG input is supported: {}", .0.display())]
    UnsupportedInput(PathBuf),
    #[error("output already exists: {}", .0.display())]
    OutputExists(PathBuf),
    #[error("--quality must be between 0 and 100")]
    InvalidQuality,
    #[error("cannot decode image: {0}")]
    Decode(#[from] image::ImageError),
    #[error("file system error: {0}")]
    Io(#[from] std::io::Error),
}

pub fn default_output(input: &Path) -> PathBuf {
    input.with_extension("webp")
}

pub fn convert_to_webp(
    input: &Path,
    output: &Path,
    options: EncodeOptions,
) -> Result<Conversion, ImageError> {
    if !input.is_file() {
        return Err(ImageError::InputNotFound(input.to_path_buf()));
    }
    if !is_supported_input(input) {
        return Err(ImageError::UnsupportedInput(input.to_path_buf()));
    }
    if output.exists() {
        return Err(ImageError::OutputExists(output.to_path_buf()));
    }
    if options
        .quality
        .is_some_and(|quality| !(0.0..=100.0).contains(&quality))
    {
        return Err(ImageError::InvalidQuality);
    }

    let input_bytes = fs::metadata(input)?.len();
    let reader = ImageReader::open(input)?.with_guessed_format()?;
    let image = reader.decode()?.into_rgba8();
    let encoder = webp::Encoder::from_rgba(image.as_raw(), image.width(), image.height());
    let encoded = if options.lossless {
        encoder.encode_lossless()
    } else {
        encoder.encode(options.quality.unwrap_or(80.0))
    };

    let temporary = temporary_output_path(output);
    let write_result = (|| -> Result<(), std::io::Error> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&encoded)?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary);
        return Err(ImageError::Io(error));
    }
    if let Err(error) = fs::hard_link(&temporary, output) {
        let _ = fs::remove_file(&temporary);
        if output.exists() {
            return Err(ImageError::OutputExists(output.to_path_buf()));
        }
        return Err(ImageError::Io(error));
    }
    fs::remove_file(&temporary)?;

    Ok(Conversion {
        input: absolute_path(input),
        output: absolute_path(output),
        input_bytes,
        output_bytes: fs::metadata(output)?.len(),
    })
}

fn is_supported_input(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png"
            )
        })
}

fn temporary_output_path(output: &Path) -> PathBuf {
    let file_name = output.file_name().unwrap_or_default().to_string_lossy();
    let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    output.with_file_name(format!(
        ".{file_name}.foundry-tmp-{}-{sequence}",
        std::process::id()
    ))
}

fn absolute_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
