pub mod analyze;
pub mod api;
pub mod checklist;
pub mod checks;
pub mod config;
pub mod diff;
pub mod history;
pub mod html_report;
pub mod metadata;
pub mod package;
pub mod pipeline;
pub mod probe;
pub mod report;
pub mod scanner;
pub mod studio;
pub mod suggestions;
pub mod validate;
pub mod watch;
pub mod webhook;

pub use config::{load_config, AppConfig, Profile};
pub use pipeline::{run_batch, BatchOptions};
pub use report::BatchReport;

pub const DEFAULT_CONFIG_PATH: &str = "src/sample.toml";
pub const DEFAULT_HISTORY_DIR: &str = ".mediaqa/history";
pub const DEFAULT_STUDIO_DIR: &str = ".mediaqa/studio";
