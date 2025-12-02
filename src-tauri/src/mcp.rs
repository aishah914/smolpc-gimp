use once_cell::sync::Lazy;
use serde_json::{json, Value};
use std::io::BufWriter;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Mutex;

/// Change this to your actual gimp-mcp path
const GIMP_MCP_PATH: &str = "/Users/aishah/gimp-mcp";

struct McpConnection {
    #[allow(dead_code)]
    child: Child,
    #[allow(dead_code)]
    stdin: BufWriter<ChildStdin>,
    started: bool,
}

impl McpConnection {
    fn new() -> Result<Self, String> {
        eprintln!("[MCP] Starting gimp-mcp server (stub mode)...");

        let mut child = Command::new("uv")
            .arg("run")
            .arg("--directory")
            .arg(GIMP_MCP_PATH)
            .arg("gimp_mcp_server.py")
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit()) // just print server logs to your terminal
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("Failed to start gimp-mcp server: {e}"))?;

        eprintln!("[MCP] gimp-mcp server started (pid={})", child.id());

        let stdin = child
            .stdin
            .take()
            .ok_or("Failed to open stdin for gimp-mcp")?;

        Ok(Self {
            child,
            stdin: BufWriter::new(stdin),
            started: true,
        })
    }
}

// Global singleton connection guarded by a mutex
static MCP: Lazy<Mutex<Option<McpConnection>>> = Lazy::new(|| Mutex::new(None));

fn with_connection<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce(&mut McpConnection) -> Result<R, String>,
{
    let mut guard = MCP
        .lock()
        .map_err(|_| "MCP mutex poisoned".to_string())?;

    if guard.is_none() {
        *guard = Some(McpConnection::new()?);
    }

    let conn = guard.as_mut().unwrap();
    f(conn)
}

/// Stub for tools/list - returns fake data but proves the pipeline works
pub fn list_tools() -> Result<Value, String> {
    with_connection(|conn| {
        if !conn.started {
            return Err("MCP server not started".into());
        }
        Ok(json!({
            "status": "ok",
            "note": "MCP stdio wiring TODO - this is stub data",
            "tools": [
                {
                    "name": "get_gimp_info",
                    "description": "Stubbed tool while we finish MCP integration."
                },
                {
                    "name": "example_tool",
                    "description": "Another fake tool for testing."
                }
            ]
        }))
    })
}

/// Stub for tools/call - just echoes what the frontend asked to call
pub fn call_tool(name: &str, arguments: Value) -> Result<Value, String> {
    with_connection(|conn| {
        if !conn.started {
            return Err("MCP server not started".into());
        }
        Ok(json!({
            "status": "ok",
            "note": "Tool call stub - MCP IO still TODO",
            "called_tool": name,
            "arguments": arguments
        }))
    })
}
