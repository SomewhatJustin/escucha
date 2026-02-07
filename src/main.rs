use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
#[command(name = "escucha", about = "Hold-to-talk speech-to-text for Linux")]
struct Cli {
    /// List available input devices
    #[arg(long)]
    list_devices: bool,

    /// Launch the troubleshooting GUI
    #[arg(long)]
    gui: bool,

    /// Run environment checks and print a diagnostic report
    #[arg(long)]
    check: bool,
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    if cli.check {
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
