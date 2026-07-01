mod commands;

use commands::AppContext;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppContext::default())
        .invoke_handler(tauri::generate_handler![
            commands::run_local_qc,
            commands::login,
            commands::list_workspaces,
            commands::switch_workspace,
            commands::get_entitlements,
            commands::get_pricing_variant,
            commands::submit_remote_job,
            commands::poll_remote_job,
            commands::queue_offline_action,
            commands::sync_offline_queue,
            commands::get_offline_queue,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Media QA Studio desktop app");
}
