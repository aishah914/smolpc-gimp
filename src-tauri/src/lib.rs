#![cfg_attr(mobile, tauri::mobile_entry_point)]

mod mcp;
mod llm_client;
mod macros; 
mod plan_schema;
mod plan_validate;
mod plan_execute;
mod commands;
mod plan_llm;

use serde_json::{json, Value};
use std::process::Command;

use serde::Serialize;
#[derive(Serialize)]
struct HealthStatus {
    ollama_reachable: bool,
    mcp_connected: bool,
    tools_count: u32,
    image_open_ok: bool,
    errors: Vec<String>,
}


#[tauri::command]
fn start_gimp_mcp_server() -> Result<(), String> {
    // Update this path to where gimp_mcp_server.py lives on your machine
    // Example for Windows:
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

#[tauri::command]
fn mcp_list_tools() -> Result<Value, String> {
    mcp::list_tools()
}

#[tauri::command]
fn mcp_call_tool(name: String, arguments: Value) -> Result<Value, String> {
    mcp::call_tool(&name, arguments)
}

#[tauri::command]
async fn assistant_request(prompt: String) -> Result<Value, String> {
    let lower_prompt = prompt.to_lowercase();

    // Fast Path: Describe Image
    if lower_prompt.contains("describe") && lower_prompt.contains("image") {
        return mcp::call_tool("get_image_metadata", json!({}));
    }

    // Fast Path: Drawing
    if lower_prompt.contains("draw") && lower_prompt.contains("line") {
        // You mentioned macros work, so call them directly here
        return macro_draw_line(50, 50, 200, 200); 
    }

    // Fast Path: Basic Edits
    if lower_prompt.contains("crop") && lower_prompt.contains("square") {
        return macro_crop_square();
    }
    // STEP 1: Tool selection, small prompt for Ollama
    let selector_prompt = format!(
        r#"
You are a tool selector for a GIMP assistant.

Decide which single tool is best for the user's request.

Tools:
- "get_gimp_info": GIMP version, platform, system, install, environment.
- "get_image_metadata": what image is open, details of the current image, size, dimensions, file name, layers.
- "call_api": editing the image (resize, crop, rotate, flip, draw, filters, colors, etc).
- "none": no tool needed, just answer in natural language.

Return ONLY JSON in this format:
{{"tool": "get_gimp_info" | "get_image_metadata" | "call_api" | "none", "reason": "short reason"}}

The response MUST:
- Start with '{{'
- Contain only JSON
- Have no explanation, no prose, no backticks, no prefix

User request: {user}
"#,
        user = prompt
    );

    let selection_raw = llm_client::chat(&selector_prompt).await?;

    // Strip anything before the first '{' to handle "Here is the response: { ... }"
    let selection_str = if let Some(idx) = selection_raw.find('{') {
        &selection_raw[idx..]
    } else {
        selection_raw.as_str()
    };

    let selection: Value = serde_json::from_str(selection_str).map_err(|e| {
        format!(
            "Failed to parse tool selection JSON: {e}\nLLM output was: {selection_raw}"
        )
    })?;

    let selected_tool = selection
        .get("tool")
        .and_then(|t| t.as_str())
        .unwrap_or("none")
        .to_string();

    // If selector says no tool, just answer with plain text and return
    if selected_tool == "none" {
        let answer_prompt = format!(
            "You are a helpful assistant that knows about GIMP.\n\
             Answer the user's question in natural language. Do not mention tools.\n\n\
             User: {user}\nAssistant:",
            user = prompt
        );

        let reply_text = llm_client::chat(&answer_prompt).await?;

        let plan = json!({
            "thought": "Tool selector chose 'none'. I answered without calling MCP tools.",
            "tool_selection": selection,
            "steps": []
        });

        return Ok(json!({
            "reply": reply_text,
            "plan": plan,
            "tool_results": []
        }));
    }

    // STEP 2: Build a plan
    // For call_api we ask LLM to generate a detailed plan.
    // For get_gimp_info and get_image_metadata we make a simple one step plan.
    let mut plan: Value = if selected_tool == "call_api" {
        let planning_prompt = format!(
            r#"
You write Python console commands to control GIMP 3 via the PyGObject console.

User request: {user}

Respond ONLY with valid JSON in this format:

{{
  "thought": "short explanation",
  "steps": [
    {{
      "tool": "call_api",
      "arguments": {{
        "api_path": "exec",
        "args": [
          "pyGObject-console",
          [
            "images = Gimp.get_images()",
            "image = images[0]",
            "layers = image.get_layers()",
            "layer = layers[0]",
            "drawable = layer",
            "... extra commands you need ...",
            "Gimp.displays_flush()"
          ]
        ],
        "kwargs": {{}}
      }}
    }}
  ]
}}

Rules:
- The response MUST start with '{{' and contain only JSON.
- Always use api_path: "exec".
- Always use args[0]: "pyGObject-console".
- args[1] must be a list of valid Python statements.
- Always include these exact base lines at the top, unchanged:

  "images = Gimp.get_images()",
  "image = images[0]",
  "layers = image.get_layers()",
  "layer = layers[0]",
  "drawable = layer",

- Never invent attributes or properties like image.active_image or image.layers.
- To resize the whole image, use: image.scale(new_width, new_height)
- To get the first layer, use: layers = image.get_layers(); layer = layers[0]
- Always finish with a line: "Gimp.displays_flush()".
- Only use the call_api tool in steps.
"#,
            user = prompt
        );

        let plan_raw = llm_client::chat(&planning_prompt).await?;

        // Strip any prefix before JSON
        let plan_str = if let Some(idx) = plan_raw.find('{') {
            &plan_raw[idx..]
        } else {
            plan_raw.as_str()
        };

        serde_json::from_str(plan_str).map_err(|e| {
            format!("Failed to parse plan JSON: {e}\nLLM output was: {plan_raw}")
        })?
    } else {
        // Simple one step plan
        json!({
            "thought": format!("Tool selector chose {tool}. I will call it once.", tool = selected_tool),
            "steps": [
                {
                    "tool": selected_tool,
                    "arguments": {}
                }
            ]
        })
    };

    // Attach the tool selection into the plan for debugging
    if let Value::Object(ref mut map) = plan {
        map.insert("tool_selection".to_string(), selection.clone());
    }

    // STEP 3: Execute each step via MCP
    let mut tool_results: Vec<Value> = Vec::new();

    if let Some(steps) = plan.get("steps").and_then(|s| s.as_array()) {
        for step in steps {
            let tool_name = step
                .get("tool")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();

            let arguments = step
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));

            let result = mcp::call_tool(&tool_name, arguments.clone())
                .map(|val| val)
                .unwrap_or_else(|err| {
                    json!({
                        "isError": true,
                        "content": [
                            { "text": format!("MCP transport error: {err}"), "type": "text" }
                        ]
                    })
                });

            tool_results.push(json!({
                "tool": tool_name,
                "arguments": arguments,
                "result": result
            }));
        }
    }

    // STEP 4: Default reply from the plan's "thought"
    let mut reply_text = plan
        .get("thought")
        .and_then(|t| t.as_str())
        .unwrap_or("I created a tool plan for your request.")
        .to_string();

    // STEP 5: SPECIAL CASE: get_gimp_info -> summarise version + platform
    for tr in &tool_results {
        if tr.get("tool").and_then(|t| t.as_str()) == Some("get_gimp_info") {
            let result_val = tr.get("result").cloned().unwrap_or_else(|| json!({}));

            let is_error = result_val
                .get("isError")
                .and_then(|e| e.as_bool())
                .unwrap_or(false);

            let text_opt = result_val
                .get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|first| first.get("text"))
                .and_then(|t| t.as_str());

            if is_error {
                if let Some(msg) = text_opt {
                    reply_text = format!(
                        "I could not get GIMP info: {}. Please make sure GIMP is open and the MCP Server plugin is running.",
                        msg
                    );
                } else {
                    reply_text =
                        "I could not get GIMP info because the MCP tool reported an error."
                            .to_string();
                }
                continue;
            }

            if let Some(text_json) = text_opt {
                if let Ok(info) = serde_json::from_str::<Value>(text_json) {
                    let version_str = info
                        .get("version")
                        .and_then(|v| v.get("detected_version"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown version");

                    let platform_str = info
                        .get("system")
                        .and_then(|s| s.get("platform"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown platform");

                    reply_text = format!(
                        "You are using GIMP {version} on {platform}.",
                        version = version_str,
                        platform = platform_str
                    );
                }
            }
        }
    }

    // STEP 6: SPECIAL CASE: get_image_metadata -> summarise current image
    for tr in &tool_results {
        if tr.get("tool").and_then(|t| t.as_str()) == Some("get_image_metadata") {
            let result_val = tr.get("result").cloned().unwrap_or_else(|| json!({}));

            let is_error = result_val
                .get("isError")
                .and_then(|e| e.as_bool())
                .unwrap_or(false);

            let text_opt = result_val
                .get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|first| first.get("text"))
                .and_then(|t| t.as_str());

            if is_error {
                if let Some(msg) = text_opt {
                    reply_text = format!(
                        "I could not get image metadata: {}. Please make sure an image is open in GIMP.",
                        msg
                    );
                } else {
                    reply_text =
                        "I could not get image metadata because the MCP tool reported an error."
                            .to_string();
                }
                continue;
            }

            if let Some(text_json) = text_opt {
                if let Ok(meta) = serde_json::from_str::<Value>(text_json) {
                    let basic = meta.get("basic").unwrap_or(&Value::Null);
                    let file = meta.get("file").unwrap_or(&Value::Null);

                    let width = basic
                        .get("width")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let height = basic
                        .get("height")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let base_type = basic
                        .get("base_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown");
                    let basename = file
                        .get("basename")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown image");

                    reply_text = format!(
                        "Your current image \"{name}\" is {w}×{h} pixels with base type {base}.",
                        name = basename,
                        w = width,
                        h = height,
                        base = base_type,
                    );
                }
            }
        }
    }

    // STEP 7: SPECIAL CASE: call_api -> summarise edit actions using real metadata
    for tr in &tool_results {
        if tr.get("tool").and_then(|t| t.as_str()) == Some("call_api") {
            let arguments_val = tr.get("arguments").cloned().unwrap_or_else(|| json!({}));
            let api_path = arguments_val
                .get("api_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let result_val = tr.get("result").cloned().unwrap_or_else(|| json!({}));

            let is_error_flag = result_val
                .get("isError")
                .and_then(|e| e.as_bool())
                .unwrap_or(false);

            let text_msg = result_val
                .get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|first| first.get("text"))
                .and_then(|t| t.as_str());

            let structured_result = result_val
                .get("structuredContent")
                .and_then(|sc| sc.get("result"))
                .and_then(|r| r.as_str());

            let looks_like_error =
                text_msg.map_or(false, |t| t.starts_with("Error:"))
                    || structured_result.map_or(false, |t| t.starts_with("Error:"));

            if is_error_flag || looks_like_error {
                let msg = structured_result
                    .or(text_msg)
                    .unwrap_or("Unknown error");

                reply_text = format!(
                    "I tried to call '{api}' but GIMP reported an error: {msg}",
                    api = api_path,
                );
                continue;
            }

            let after_meta_raw = mcp::call_tool("get_image_metadata", json!({}))
                .unwrap_or_else(|err| json!({
                    "isError": true,
                    "content": [
                        { "text": format!("Failed to fetch image metadata after edit: {err}"), "type": "text" }
                    ]
                }));

            println!("DEBUG: after_meta_raw: {:#?}", after_meta_raw);

            let after_is_error = after_meta_raw
                .get("isError")
                .and_then(|e| e.as_bool())
                .unwrap_or(false);

            let after_text_opt = after_meta_raw
                .get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|first| first.get("text"))
                .and_then(|t| t.as_str());

            if after_is_error {
                reply_text = format!(
                    "I called '{api}', but I could not read the updated image metadata. Please check GIMP to see the result.",
                    api = api_path
                );
                continue;
            }

            if let Some(after_text_json) = after_text_opt {
                if let Ok(meta) = serde_json::from_str::<Value>(after_text_json) {
                    let basic = meta.get("basic").unwrap_or(&Value::Null);
                    let file = meta.get("file").unwrap_or(&Value::Null);

                    let width = basic
                        .get("width")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let height = basic
                        .get("height")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);
                    let base_type = basic
                        .get("base_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown");
                    let basename = file
                        .get("basename")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown image");

                    if api_path.contains("scale") || api_path == "exec" {
                        reply_text = format!(
                            "I called '{api}' on the current image. After that, \"{name}\" is now {w}×{h} pixels with base type {base}.",
                            api = api_path,
                            name = basename,
                            w = width,
                            h = height,
                            base = base_type,
                        );
                    } else if api_path.contains("crop") {
                        reply_text = format!(
                            "I called '{api}' to crop the image. It is now {w}×{h} pixels (\"{name}\", base type {base}).",
                            api = api_path,
                            name = basename,
                            w = width,
                            h = height,
                            base = base_type,
                        );
                    } else {
                        reply_text = format!(
                            "I called '{api}' on the image. It is currently {w}×{h} pixels (\"{name}\", base type {base}).",
                            api = api_path,
                            name = basename,
                            w = width,
                            h = height,
                            base = base_type,
                        );
                    }
                } else {
                    reply_text = if api_path.is_empty() {
                        "I performed an edit using call_api, but could not parse the updated metadata.".to_string()
                    } else {
                        format!(
                            "I performed an edit using '{api}', but could not parse the updated metadata.",
                            api = api_path
                        )
                    };
                }
            } else {
                reply_text = if api_path.is_empty() {
                    "I performed an edit using call_api, but there was no metadata response."
                        .to_string()
                } else {
                    format!(
                        "I performed an edit using '{api}', but there was no metadata response.",
                        api = api_path
                    )
                };
            }
        }
    }

    Ok(json!({
        "reply": reply_text,
        "plan": plan,
        "tool_results": tool_results
    }))
}

#[tauri::command]
async fn health_check() -> HealthStatus {
    let mut errors = Vec::new();

    // --- Check 1: Ollama reachable ---
    let ollama_reachable = match reqwest::get("http://localhost:11434").await {
        Ok(_) => true,
        Err(e) => {
            errors.push(format!("Ollama not reachable: {}", e));
            false
        }
    };

    // --- Check 2: MCP server reachable ---
    let mcp_connected = true; // TEMP: replace with real MCP init/ping later

    // --- Check 3: MCP tools available ---
    let tools_count = 0; // TEMP: replace with list_tools() later

    // --- Check 4: GIMP image open ---
    let image_open_ok = false; // TEMP: replace with get_image_metadata() later

    HealthStatus {
        ollama_reachable,
        mcp_connected,
        tools_count,
        image_open_ok,
        errors,
    }
}

#[tauri::command]
async fn test_basic_mcp() -> String {
    let mut output = String::new();

    // --- Test 1: list MCP tools ---
    match mcp_list_tools() {
        Ok(tools) => {
            output.push_str("MCP tools:\n");
            output.push_str(&format!("{:#?}\n\n", tools));
        }
        Err(e) => {
            output.push_str(&format!("❌ list_tools failed: {}\n\n", e));
        }
    }

    // --- Test 2: get image metadata ---
    match mcp_list_tools(){
        Ok(meta) => {
            output.push_str("Image metadata:\n");
            output.push_str(&format!("{:#?}\n", meta));
        }
        Err(e) => {
            output.push_str(&format!("❌ get_image_metadata failed: {}\n", e));
        }
    }

    output
}

#[tauri::command]
fn macro_draw_line(x1: i32, y1: i32, x2: i32, y2: i32) -> Result<serde_json::Value, String> {
    // Build the payload (JSON) using the macro helper
    let payload = macros::draw_line(x1, y1, x2, y2);

    //  MCP layer expects: call_tool(name, arguments)
    // Our macro payload shape is: { "name": "...", "arguments": {...} }
    let tool_name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Macro payload missing 'name'".to_string())?;

    let arguments = payload
        .get("arguments")
        .cloned()
        .ok_or_else(|| "Macro payload missing 'arguments'".to_string())?;

    // Execute via MCP
    mcp::call_tool(tool_name, arguments)
}

#[tauri::command]
fn macro_crop_square() -> Result<serde_json::Value, String> {
    let payload = macros::crop_to_square();

    let tool_name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Macro payload missing 'name'".to_string())?;

    let arguments = payload
        .get("arguments")
        .cloned()
        .ok_or_else(|| "Macro payload missing 'arguments'".to_string())?;

    mcp::call_tool(tool_name, arguments)
}

#[tauri::command]
fn macro_resize(width: i32) -> Result<serde_json::Value, String> {
    let payload = macros::resize_width(width);

    let tool_name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Macro payload missing 'name'".to_string())?;

    let arguments = payload
        .get("arguments")
        .cloned()
        .ok_or_else(|| "Macro payload missing 'arguments'".to_string())?;

    mcp::call_tool(tool_name, arguments)
}

#[tauri::command]
fn macro_brightness_contrast(brightness: f64, contrast: f64) -> Result<serde_json::Value, String> {
    let payload = macros::brightness_contrast(brightness, contrast);

    let tool_name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Macro payload missing 'name'".to_string())?;

    let arguments = payload
        .get("arguments")
        .cloned()
        .ok_or_else(|| "Macro payload missing 'arguments'".to_string())?;

    mcp::call_tool(tool_name, arguments)
}

#[tauri::command]
fn macro_blur(radius: f64) -> Result<serde_json::Value, String> {
    let payload = macros::blur(radius);

    let tool_name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Macro payload missing 'name'".to_string())?;

    let arguments = payload
        .get("arguments")
        .cloned()
        .ok_or_else(|| "Macro payload missing 'arguments'".to_string())?;

    mcp::call_tool(tool_name, arguments)
}




pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            start_gimp_mcp_server,
            mcp_list_tools,
            mcp_call_tool,
            assistant_request,
            health_check,
            test_basic_mcp, 
            macro_draw_line, 
            macro_crop_square, 
            macro_resize,
            macro_brightness_contrast, 
            macro_blur, 
            commands::run_action_plan,

        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
