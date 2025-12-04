#![cfg_attr(mobile, tauri::mobile_entry_point)]

mod mcp;
mod llm_client;

use serde_json::{json, Value};

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
    // 1) Build a planning prompt that forces JSON output
    let system_prompt = r#"
You are an AI assistant that controls GIMP using tools exposed by an MCP server.

Respond ONLY with valid JSON. DO NOT include any explanation outside of JSON.
The JSON format MUST be:

{
  "thought": "Short explanation of what you will do",
  "steps": [
    {
      "tool": "tool_name",
      "arguments": {
        // JSON arguments for the tool
      }
    }
  ]
}

Tools you can use (for now):

- get_gimp_info
  Description: Get information about the GIMP environment, version, and setup.
  Arguments: {}

- get_image_metadata
  Description: Get metadata about the currently open image: width, height, file name, color mode, etc.
  Arguments: {}

- call_api
  Description: Execute Python code inside GIMP via the PyGObject console. Use this for editing actions like crop, resize, rotate, flip, changing colors, applying filters, or exporting.
  Arguments:
    {
      "api_path": "string, required. Use \"exec\" for Python execution",
      "args": [
        "pyGObject-console",
        [
          "python_code_line_1",
          "python_code_line_2",
          "... more lines ..."
        ]
      ],
      "kwargs": "object, usually {}"
    }

  Rules for call_api:
  - Always use api_path: "exec"
  - Always use args[0] = "pyGObject-console"
  - args[1] must be an array of Python code strings
  - Typical pattern for working on the active image:
    [
      "images = Gimp.get_images()",
      "image = images[0]",
      "layers = image.get_layers()",
      "layer = layers[0]",
      "drawable = layer"
    ]
  - After drawing or editing operations, always call:
    "Gimp.displays_flush()"


- example_tool
  Description: A fake example tool used for testing. You can give it any JSON arguments.

Rules:
- Always return a JSON object with "thought" and "steps".
- "steps" must be an array (possibly empty).
- Every step must have "tool" (string) and "arguments" (object).
- If no tool is needed, return "steps": [].

Examples:

User request: "What version of GIMP am I using?"
Plan:
{
  "thought": "I will query GIMP for its version information.",
  "steps": [
    { "tool": "get_gimp_info", "arguments": {} }
  ]
}

User request: "Tell me about the current image"
Plan:
{
  "thought": "I will inspect metadata of the current image.",
  "steps": [
    { "tool": "get_image_metadata", "arguments": {} }
  ]
}

User request: "Resize the image to 500 by 300"
Plan:
{
  "thought": "I will resize the current image using Python code via the GIMP console.",
  "steps": [
    {
      "tool": "call_api",
      "arguments": {
        "api_path": "exec",
        "args": [
          "pyGObject-console",
          [
            "images = Gimp.get_images()",
            "image = images[0]",
            "image.scale(500, 300)",
            "Gimp.displays_flush()"
          ]
        ],
        "kwargs": {}
      }
    }
  ]
}


User request: "Crop the image to a 200x200 square"
Plan:
{
  "thought": "I will crop the current image using Python code via the GIMP console.",
  "steps": [
    {
      "tool": "call_api",
      "arguments": {
        "api_path": "exec",
        "args": [
          "pyGObject-console",
          [
            "images = Gimp.get_images()",
            "image = images[0]",
            "image.crop(200, 200, 0, 0)",
            "Gimp.displays_flush()"
          ]
        ],
        "kwargs": {}
      }
    }
  ]
}

"#;

    let planning_prompt = format!(
        "{system}\nUser request: {user}",
        system = system_prompt,
        user = prompt
    );

    // 2) Ask the LLM for a JSON plan
    let plan_raw = llm_client::chat(&planning_prompt).await?;

    // 3) Parse the JSON plan
    let plan: Value = serde_json::from_str(&plan_raw)
        .map_err(|e| format!("Failed to parse plan JSON: {e}\nLLM output was: {plan_raw}"))?;

    // 4) Execute each step via MCP
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
                .unwrap_or_else(|err| json!({
                    "isError": true,
                    "content": [
                        { "text": format!("MCP transport error: {err}"), "type": "text" }
                    ]
                }));

            tool_results.push(json!({
                "tool": tool_name,
                "arguments": arguments,
                "result": result
            }));
        }
    }

    // 5) Default reply from the plan's "thought"
    let mut reply_text = plan
        .get("thought")
        .and_then(|t| t.as_str())
        .unwrap_or("I created a tool plan for your request.")
        .to_string();

    // 6) SPECIAL CASE: get_gimp_info -> summarise version + platform
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
                        "I couldn't get GIMP info: {}. Please make sure GIMP is open and the MCP Server plugin is running.",
                        msg
                    );
                } else {
                    reply_text =
                        "I couldn't get GIMP info because the MCP tool reported an error."
                            .to_string();
                }
                // If this is an error, we don't try to parse JSON
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

    // 7) SPECIAL CASE: get_image_metadata -> summarise current image
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

            // error case
            if is_error {
                if let Some(msg) = text_opt {
                    reply_text = format!(
                        "I couldn't get image metadata: {}. Please make sure an image is open in GIMP.",
                        msg
                    );
                } else {
                    reply_text =
                        "I couldn't get image metadata because the MCP tool reported an error."
                            .to_string();
                }
                continue;
            }

            // happy path: parse the metadata JSON (basic + file), like the one you pasted
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

        // 8) SPECIAL CASE: call_api -> summarise edit actions using real metadata
    for tr in &tool_results {
        if tr.get("tool").and_then(|t| t.as_str()) == Some("call_api") {
            let arguments_val = tr.get("arguments").cloned().unwrap_or_else(|| json!({}));
            let api_path = arguments_val
                .get("api_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");

        // Owned Vec<Value> to avoid lifetime issues
            let args_vec: Vec<Value> = arguments_val
                .get("args")
                .and_then(|a| a.as_array())
                .cloned()
                .unwrap_or_else(Vec::new);
            let result_val = tr.get("result").cloned().unwrap_or_else(|| json!({}));

        // Check the isError flag from MCP
            let is_error_flag = result_val
                .get("isError")
                .and_then(|e| e.as_bool())
                .unwrap_or(false);

        // Pull out text message and structured result, if present
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

        // Treat anything that starts with "Error:" as an error,
        // even if isError == false.
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

        // After a successful call_api, ask GIMP what the image looks like now
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
            // We did an edit, but we can't read back metadata cleanly
                reply_text = format!(
                    "I called '{api}', but I couldn't read the updated image metadata. Please check GIMP to see the result.",
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
                        "I performed an edit using call_api, but couldn't parse the updated metadata."
                            .to_string()
                    } else {
                        format!(
                            "I performed an edit using '{api}', but couldn't parse the updated metadata.",
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

    // 9) Return everything back to the frontend
    Ok(json!({
        "reply": reply_text,
        "plan": plan,
        "tool_results": tool_results
    }))
    
    
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            mcp_list_tools,
            mcp_call_tool,
            assistant_request,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
