#![cfg_attr(mobile, tauri::mobile_entry_point)]

mod mcp;

use serde_json::Value;

#[tauri::command]
fn mcp_list_tools() -> Result<Value, String> {
    mcp::list_tools()
}

#[tauri::command]
fn mcp_call_tool(name: String, arguments: Value) -> Result<Value, String> {
    mcp::call_tool(&name, arguments)
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            mcp_list_tools,
            mcp_call_tool,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
