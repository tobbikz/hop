//! tui-sessions — SSH / RDP session manager (PTY + external RDP).

mod app;

use anyhow::Result;
use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "tui-sessions",
    version,
    about = "Keyboard-first SSH/RDP session manager for the terminal",
    long_about = None
)]
struct Cli {
    /// Print environment diagnostics and exit
    #[arg(long)]
    doctor: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_logging()?;
    if cli.doctor {
        print_doctor();
        return Ok(());
    }
    app::run().await
}

fn init_logging() -> Result<()> {
    if let Some(mut dir) = dirs::cache_dir() {
        dir.push("tui-sessions");
        let _ = std::fs::create_dir_all(&dir);
        let file = std::fs::File::create(dir.join("tui-sessions.log"))?;
        tracing_subscriber::fmt()
            .with_writer(file)
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .init();
    }
    Ok(())
}

fn mark(ok: bool) -> &'static str {
    if ok {
        "ok"
    } else {
        "missing"
    }
}

fn print_doctor() {
    use ide_sessions::{
        rdp_client_available, secrets_file_path, sessions_path, ssh_available, sshpass_available,
    };

    let config = ide_core::config_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "unavailable".into());
    let term = std::env::var("TERM").unwrap_or_else(|_| "unset".into());
    let colorterm = std::env::var("COLORTERM").unwrap_or_else(|_| "unset".into());

    println!("tui-sessions {}", env!("CARGO_PKG_VERSION"));
    println!("os: {} {}", std::env::consts::OS, std::env::consts::ARCH);
    println!("config dir: {config}");
    println!(
        "sessions: {}",
        sessions_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "unavailable".into())
    );
    println!(
        "secrets: {}",
        secrets_file_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "unavailable".into())
    );
    println!("TERM: {term}");
    println!("COLORTERM: {colorterm}");
    println!("ssh: {}", mark(ssh_available()));
    println!("sshpass: {}", mark(sshpass_available()));
    println!(
        "rdp client: {}",
        if rdp_client_available("") {
            "ok"
        } else {
            "missing"
        }
    );
}
