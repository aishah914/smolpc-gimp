#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::Command;

#[tauri::command]
fn start_gimp_mcp_server() -> Result<(), String> {
    // TODO: if this path changes on your machine, update it here.
    let gimp_mcp_dir = r"C:\Users\User\dev\gimp-mcp";

    Command::new("uv")
        .arg("run")
        .arg("--directory")
        .arg(gimp_mcp_dir)
        .arg("gimp_mcp_server.py")
        .spawn()
        .map_err(|e| format!("Failed to start gimp_mcp_server.py: {e}"))?;

    Ok(())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            start_gimp_mcp_server,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
