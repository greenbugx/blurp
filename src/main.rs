use crate::app::App;
use clap::Parser;

pub mod app;
pub mod draft;
pub mod event;
pub mod state;
pub mod ui;

#[derive(Debug, Parser)]
#[command(
    name = "blurp",
    version,
    about = "Turn messy thoughts into editable message drafts."
)]
struct Cli {
    /// Start with clearly labeled in-memory sample drafts.
    #[arg(long)]
    demo: bool,
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let cli = Cli::parse();
    color_eyre::install()?;
    let terminal = ratatui::init();
    let app = if cli.demo { App::demo() } else { App::new() };
    let result = app.run(terminal).await;
    ratatui::restore();
    result
}
