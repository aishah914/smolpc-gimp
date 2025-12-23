<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  type AssistantResponse = {
  reply: string;
  plan: any;
  tool_results: any[];
  };


  type Message = {
    role: "user" | "assistant";
    text: string;
  };

  let messages: Message[] = [];
  let input = "";
  let isSending = false;

  // Debug / dev results
  let llmTestResult = "";
  let toolsListResult = "";
  let toolCallResult = "";

  // Simple status indicators (you can wire these to real data later)
  let imageInfo = 'No image info yet';
  let gimpStatus = 'Disconnected';
  let llmStatus = 'Ready';

  // --- Chat behaviour (assistant_request) ---
  async function sendChat() {
  const trimmed = input.trim();
  if (!trimmed || isSending) return;

  const userText = trimmed;
  input = "";
  messages = [...messages, { role: "user", text: userText }];
  isSending = true;

  try {
    // Get the full JSON response from Rust
    const result = await invoke<AssistantResponse>("assistant_request", { prompt: userText });

    const replyText =
      result && typeof result === "object" && "reply" in result
        ? result.reply
        : "I created a tool plan for your request.";

    // Show reply in chat
    messages = [...messages, { role: "assistant", text: replyText }];

    // Mark LLM as active
    llmStatus = "Connected";

    // If any tools ran, update status + imageInfo
    if (Array.isArray(result.tool_results)) {
      let sawMcp = false;

      for (const tr of result.tool_results) {
        const toolName = tr?.tool;

        if (toolName === "get_image_metadata") {
          sawMcp = true;
          const metaResult = tr?.result ?? {};
          const isError = !!metaResult?.isError;

          const textJson =
            metaResult?.content?.[0]?.text && typeof metaResult.content[0].text === "string"
              ? metaResult.content[0].text
              : null;

          if (!isError && textJson) {
            try {
              const meta = JSON.parse(textJson);
              const basic = meta.basic ?? {};
              const file = meta.file ?? {};

              const width = basic.width ?? 0;
              const height = basic.height ?? 0;
              const base = basic.base_type ?? "Unknown";
              const name = file.basename ?? "unknown image";

              imageInfo = `"${name}" — ${width}×${height} px (${base})`;
            } catch (err) {
              console.error("Failed to parse image metadata JSON:", err, textJson);
              imageInfo = "Could not parse image metadata (see console).";
            }
          } else if (isError) {
            imageInfo =
              "Error getting image metadata. Make sure an image is open in GIMP and MCP is running.";
          }
        }

        if (toolName === "get_gimp_info" || toolName === "call_api") {
          sawMcp = true;
        }
      }

      if (sawMcp) {
        gimpStatus = "Connected";
      }
    }
  } catch (e) {
    console.error(e);
    messages = [
      ...messages,
      {
        role: "assistant",
        text:
          "Sorry, something went wrong talking to the assistant. Check the developer tools panel and Tauri console for errors."
      }
    ];
  } finally {
    isSending = false;
  }
}


  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void sendChat();
    }
  }

  // --- Developer tools ---

  async function testLlm() {
  try {
    llmStatus = "Checking…";
    const result = await invoke<string>("test_llm"); // make sure this command exists in Rust
    llmTestResult = result;
    llmStatus = "Connected";
  } catch (e) {
    console.error(e);
    llmTestResult = String(e);
    llmStatus = "Error – see developer tools";
  }
}

  async function listTools() {
  try {
    gimpStatus = "Checking MCP…";
    const result = await invoke<string>("mcp_list_tools"); // must match Rust command name
    toolsListResult = result;
    gimpStatus = "Connected";
  } catch (e) {
    console.error(e);
    toolsListResult = String(e);
    gimpStatus = "Disconnected (error)";
  }
}


  async function callExampleTool() {
    try {
      // TODO: replace with an actual tool name and params that work in your setup
      const result = await invoke<string>("mcp_call_tool", 
      {
        tool: "example_tool",
        params: {}
      });
      toolCallResult = result;
    } catch (e) {
      console.error(e);
      toolCallResult = String(e);
    }
  }
</script>

<div class="app-shell">
  <header class="app-header">
    <div>
      <h1>SmolPC · GIMP AI Assistant</h1>
      <span>Offline LLM + MCP · Local-only reasoning</span>
    </div>
    <div class="badge">
      <span>Prototype</span>
    </div>
  </header>

  <main class="app-main">
    <!-- LEFT: Assistant chat -->
    <section class="app-card">
      <div class="chat-header">
        Assistant chat
        <span class="status-text">
          {isSending ? "· Thinking…" : "· Ready"}
        </span>
      </div>

      <div class="chat-window">
        {#if messages.length === 0}
          <div class="chat-message assistant">
            <div class="chat-message-header">Assistant</div>
            <div>
              Hi! I’m your GIMP assistant. Describe what you want to change in the current image,
              and I’ll plan and apply edits using local tools.
            </div>
          </div>
        {/if}

        {#each messages as msg, i}
          <div class={`chat-message ${msg.role}`}>
            <div class="chat-message-header">
              {msg.role === "user" ? "You" : "Assistant"}
            </div>
            <div>{msg.text}</div>
          </div>
        {/each}
      </div>

      <div class="chat-input-row">
        <textarea
          placeholder="Describe what you want to do in GIMP…"
          bind:value={input}
          on:keydown={handleKeydown}
        ></textarea>
        <button class="button" on:click={sendChat} disabled={isSending || !input.trim()}>
          {isSending ? "Sending…" : "Send"}
        </button>
      </div>
    </section>

    <!-- RIGHT: Sidebar (status + dev tools) -->
    <aside class="app-card">
      <div class="sidebar-section">
        <h2>Image & connection</h2>
        <div class="sidebar-kv">
          <div><strong>Image:</strong> {imageInfo}</div>
          <div><strong>GIMP/MCP:</strong> {gimpStatus}</div>
          <div><strong>LLM:</strong> {llmStatus}</div>
        </div>
      </div>

      <div class="sidebar-section">
        <h2>Developer tools</h2>
        <details class="dev-panel">
          <summary>LLM test</summary>
          <button class="button" style="margin-top: 0.4rem;" on:click={testLlm}>
            Test LLM
          </button>
          {#if llmTestResult}
            <pre>{llmTestResult}</pre>
          {/if}
        </details>

        <details class="dev-panel" open>
          <summary>Tools / list result</summary>
          <button class="button" style="margin-top: 0.4rem;" on:click={listTools}>
            List tools
          </button>
          {#if toolsListResult}
            <pre>{toolsListResult}</pre>
          {/if}
        </details>

        <details class="dev-panel">
          <summary>Tool call result</summary>
          <button class="button" style="margin-top: 0.4rem;" on:click={callExampleTool}>
            Call example tool
          </button>
          {#if toolCallResult}
            <pre>{toolCallResult}</pre>
          {/if}
        </details>
      </div>
    </aside>
  </main>
</div>
