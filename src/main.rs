use crate::app::App;
use crate::contract::DraftRequest;
use crate::db::Store;
use crate::ollama::{OllamaClient, OllamaConfig};
use clap::Parser;

pub mod app;
pub mod contract;
pub mod db;
pub mod draft;
pub mod event;
pub mod ollama;
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

    /// Process one brain dump through local Ollama, print the drafts as JSON, and exit.
    #[arg(long)]
    dump: Option<String>,
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let cli = Cli::parse();
    color_eyre::install()?;

    if let Some(dump) = cli.dump {
        let client = OllamaClient::new(OllamaConfig::default())?;
        let request = DraftRequest::new(dump, None)?;
        let batch = client.draft(&request).await?;
        println!("{}", serde_json::to_string_pretty(&batch)?);
        return Ok(());
    }

    let app = if cli.demo {
        App::demo()
    } else {
        App::new(Store::open()?)?
    };

    let terminal = ratatui::init();
    let result = app.run(terminal).await;
    ratatui::restore();
    result
}
