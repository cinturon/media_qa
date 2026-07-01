mod commands;

use commands::WatchState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(WatchState::default())
        .invoke_handler(tauri::generate_handler![
            commands::run_local_qc,
            commands::start_watch_folder,
            commands::stop_watch_folder,
            commands::get_watch_status,
            commands::list_profiles,
            commands::create_profile,
            commands::delete_profile,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Media QA Studio desktop app");
}
