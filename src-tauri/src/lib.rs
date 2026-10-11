mod commands;
mod drivers;
mod errors;
mod models;
mod services;
mod state;
mod vault;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = AppState::new();

    tauri::Builder::default()
        .manage(app_state)
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            // Profiles & connections
            commands::connection::get_connection_profiles,
            commands::connection::save_connection_profile,
            commands::connection::delete_connection_profile,
            commands::connection::test_connection,
            commands::connection::connect_database,
            commands::connection::disconnect_database,
            // Introspection
            commands::schema::list_tables,
            commands::schema::get_table_schema,
            commands::schema::get_full_schema,
            commands::schema::get_table_ddl,
            commands::schema::get_server_version,
            commands::schema::get_database_stats,
            commands::schema::add_foreign_key,
            // Data & queries
            commands::data::get_table_data,
            commands::data::execute_query,
            commands::data::explain_query,
            commands::data::preview_staged_changes,
            commands::data::commit_staged_changes,
            // Maintenance, search, export/import
            commands::ops::run_maintenance,
            commands::ops::search_text_in_database,
            commands::ops::export_table,
            commands::ops::import_csv,
            commands::ops::backup_database,
            commands::ops::get_data_dir,
            // Logs & audit
            commands::audit::get_sql_logs,
            commands::audit::clear_sql_logs,
            commands::audit::audit_storage_degraded,
            commands::audit::get_audit_log,
            commands::audit::get_audit_stats,
            commands::audit::clear_audit_log,
            commands::audit::export_audit_log,
            // Snippets
            commands::snippets::list_snippets,
            commands::snippets::save_snippet,
            commands::snippets::delete_snippet,
            // AI assistant
            commands::ai::ai_get_config,
            commands::ai::ai_save_config,
            commands::ai::ai_generate,
        ])
        .run(tauri::generate_context!())
        .expect("error while running TablePro application");
}
