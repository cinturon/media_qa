pub mod analyze;
pub mod api;
pub mod batch;
pub mod captions;
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

pub use config::{
    create_user_profile, custom_profile_names, delete_user_profile, list_profile_summaries,
    load_config, load_default_config, load_merged_config, resolve_config_path, AppConfig,
    CreateProfileInput, Profile, ProfileRules, ProfileSummary,
};
pub use pipeline::{run_batch, run_multi_profile_sequential_with_progress, BatchOptions};
pub use report::{BatchReport, FileReport, MultiProfileReport};
pub use watch::{
    watch_folder, watch_folder_cancellable, watch_folder_cancellable_multi, WatchOptions,
};

pub const DEFAULT_CONFIG_PATH: &str = "src/sample.toml";
pub const DEFAULT_HISTORY_DIR: &str = ".mediaqa/history";
pub const DEFAULT_STUDIO_DIR: &str = ".mediaqa/studio";
