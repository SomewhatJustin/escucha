use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
#[command(name = "escucha", about = "Hold-to-talk speech-to-text for Linux")]
struct Cli {
    /// List available input devices
    #[arg(long)]
    list_devices: bool,

    /// Launch the toolbar (system tray) app
    #[arg(long)]
    gui: bool,

    /// Run environment checks and print a diagnostic report
    #[arg(long)]
    check: bool,

    /// Run structured diagnostics and print JSON output
    #[arg(long)]
    diagnose: bool,

    /// Run headless smoke test flow and print JSON output
    #[arg(long)]
    smoke_test: bool,
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    if cli.diagnose {
        let ok = escucha::diagnostics::run_and_print("diagnose", false)?;
        if !ok {
            std::process::exit(1);
        }
    } else if cli.smoke_test {
        let ok = escucha::diagnostics::run_and_print("smoke-test", true)?;
        if !ok {
            std::process::exit(1);
        }
    } else if cli.check {
        let report = escucha::preflight::check_environment();
        print!("{report}");
        if report.has_critical_failures() {
            std::process::exit(1);
        }
    } else if cli.list_devices {
        escucha::input::list_devices_cli()?;
    } else if cli.gui {
        escucha::gui::run_gui()?;
    } else {
        escucha::service::run_daemon()?;
    }

    Ok(())
}
