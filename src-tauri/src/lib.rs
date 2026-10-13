pub mod models;
pub mod config;
pub mod ppt_parser;
pub mod tts_service;
pub mod video_composer;
pub mod project_manager;
pub mod commands;

use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_app_config,
            update_app_config,
            create_project_cmd,
            save_project_cmd,
            load_project_cmd,
            detect_first_role_cmd,
            synthesize_slide_cmd,
            compose_video_cmd,
            open_path_cmd,
            list_recent_projects_cmd,
            select_ppt_file_cmd,
            select_project_file_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
