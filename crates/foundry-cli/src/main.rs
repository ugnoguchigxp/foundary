use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use foundry_core::{Failure, Success};
use foundry_image::{EncodeOptions, ImageError, convert_to_webp, default_output};
use serde::Serialize;

#[derive(Parser)]
#[command(name = "foundry", version, about = "Shared local file-processing CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List capabilities known to this installation.
    List {
        /// Emit a machine-readable result.
        #[arg(long)]
        json: bool,
    },
    /// Diagnose which capabilities are available in this environment.
    Doctor {
        /// Emit a machine-readable result.
        #[arg(long)]
        json: bool,
    },
    /// Convert and inspect images.
    Image {
        #[command(subcommand)]
        command: ImageCommand,
    },
}

#[derive(Subcommand)]
enum ImageCommand {
    /// Convert one JPG, JPEG, or PNG file to WebP.
    Webp(WebpArgs),
}

#[derive(Args)]
struct WebpArgs {
    /// Source JPG, JPEG, or PNG image.
    input: PathBuf,

    /// Destination WebP file. Defaults to the source directory and file stem.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Preserve decoded image pixels; cannot be combined with --quality.
    #[arg(long, conflicts_with = "quality")]
    lossless: bool,

    /// Lossy WebP quality from 0 to 100. Defaults to 80.
    #[arg(long)]
    quality: Option<f32>,

    /// Emit a machine-readable result.
    #[arg(long)]
    json: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Capability {
    id: &'static str,
    description: &'static str,
    status: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorCapability {
    id: &'static str,
    kind: &'static str,
    status: &'static str,
    detail: &'static str,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::List { json } => list(json),
        Command::Doctor { json } => doctor(json),
        Command::Image {
            command: ImageCommand::Webp(args),
        } => webp(args),
    }
}

fn list(json: bool) -> ExitCode {
    let capabilities = vec![Capability {
        id: "image.webp",
        description: "Convert one JPG, JPEG, or PNG image to WebP",
        status: "ready",
    }];
    if json {
        print_json(&Success::new("list", capabilities, Vec::new()));
    } else {
        println!("image.webp\tConvert one JPG, JPEG, or PNG image to WebP");
    }
    ExitCode::SUCCESS
}

fn doctor(json: bool) -> ExitCode {
    let capabilities = vec![DoctorCapability {
        id: "image",
        kind: "native",
        status: "ready",
        detail: "Built-in JPG, PNG, and WebP support is available",
    }];
    if json {
        print_json(&Success::new("doctor", capabilities, Vec::new()));
    } else {
        println!("Foundry {}", env!("CARGO_PKG_VERSION"));
        println!();
        println!("Native");
        println!("  image       READY  Built-in JPG, PNG, and WebP support is available");
    }
    ExitCode::SUCCESS
}

fn webp(args: WebpArgs) -> ExitCode {
    let output = args.output.unwrap_or_else(|| default_output(&args.input));
    match convert_to_webp(
        &args.input,
        &output,
        EncodeOptions {
            lossless: args.lossless,
            quality: args.quality,
        },
    ) {
        Ok(result) => {
            let mut warnings = Vec::new();
            if result.output_bytes >= result.input_bytes {
                warnings.push("WebP output is not smaller than the input".to_owned());
            }
            if args.json {
                print_json(&Success::new("image.webp", result, warnings));
            } else {
                let change = percent_change(result.input_bytes, result.output_bytes);
                println!(
                    "{} -> {} ({} -> {}, {change})",
                    result.input.display(),
                    result.output.display(),
                    format_bytes(result.input_bytes),
                    format_bytes(result.output_bytes),
                );
                for warning in warnings {
                    eprintln!("warning: {warning}");
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            let (code, message) = error_detail(&error);
            if args.json {
                print_json(&Failure::new("image.webp", code, message));
            } else {
                eprintln!("error: {message}");
            }
            ExitCode::from(1)
        }
    }
}

fn error_detail(error: &ImageError) -> (&'static str, String) {
    let code = match error {
        ImageError::InputNotFound(_) => "INPUT_NOT_FOUND",
        ImageError::UnsupportedInput(_) => "INVALID_INPUT",
        ImageError::OutputExists(_) => "OUTPUT_EXISTS",
        ImageError::InvalidQuality => "INVALID_ARGUMENT",
        ImageError::Decode(_) => "INVALID_INPUT",
        ImageError::Io(_) => "IO_ERROR",
    };
    (code, error.to_string())
}

fn print_json(value: &impl Serialize) {
    println!(
        "{}",
        serde_json::to_string(value).expect("JSON result serialization must succeed")
    );
}

fn percent_change(input: u64, output: u64) -> String {
    if input == 0 {
        return "n/a".to_owned();
    }
    let percent = (1.0 - output as f64 / input as f64) * 100.0;
    if percent >= 0.0 {
        format!("{percent:.0}% smaller")
    } else {
        format!("{:.0}% larger", percent.abs())
    }
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    if bytes < 1024 {
        format!("{bytes} B")
    } else if (bytes as f64) < MIB {
        format!("{:.1} KiB", bytes as f64 / KIB)
    } else {
        format!("{:.1} MiB", bytes as f64 / MIB)
    }
}
