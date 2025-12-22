//! EU4 Bridge - Connect trained AI to real Europa Universalis IV.
//!
//! Phase A: Screen capture and OCR proof of concept.

mod capture;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "eu4-bridge")]
#[command(about = "Bridge between EU4 game and AI via screen capture")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all visible windows
    ListWindows,

    /// Capture a window screenshot
    Capture {
        /// Window title to search for (substring match)
        #[arg(short, long, default_value = "Europa Universalis")]
        window: String,

        /// Output file path
        #[arg(short, long, default_value = "capture.png")]
        output: String,
    },

    /// Test capture loop (capture every N seconds)
    Watch {
        /// Window title to search for
        #[arg(short, long, default_value = "Europa Universalis")]
        window: String,

        /// Interval in seconds
        #[arg(short, long, default_value = "5")]
        interval: u64,
    },
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    match cli.command {
        Commands::ListWindows => {
            let windows = capture::list_windows()?;
            println!("Found {} windows:", windows.len());
            for w in &windows {
                println!(
                    "  \"{}\" - {}x{} at ({}, {})",
                    w.title, w.width, w.height, w.x, w.y
                );
            }
        }

        Commands::Capture { window, output } => {
            let win = capture::find_window(&window)?;
            capture::capture_and_save(&win, &output)?;
            println!("Captured to {}", output);
        }

        Commands::Watch { window, interval } => {
            let win = capture::find_window(&window)?;
            println!(
                "Watching \"{}\" every {}s (Ctrl+C to stop)",
                win.title(),
                interval
            );

            let mut frame = 0;
            loop {
                let filename = format!("frame_{:04}.png", frame);
                if let Err(e) = capture::capture_and_save(&win, &filename) {
                    log::error!("Capture failed: {}", e);
                }
                frame += 1;
                std::thread::sleep(std::time::Duration::from_secs(interval));
            }
        }
    }

    Ok(())
}
