<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  // ---------------- MCP STUB STATE ----------------
  let toolsJson = "";
  let callResultJson = "";
  let errorMsg = "";

  async function listTools() {
    errorMsg = "";
    try {
      const result = await invoke("mcp_list_tools");
      toolsJson = JSON.stringify(result, null, 2);
    } catch (err) {
      errorMsg = String(err);
    }
  }

  async function callGetGimpInfo() {
    errorMsg = "";
    try {
      const result = await invoke("mcp_call_tool", {
        name: "get_gimp_info",
        arguments: {}
      });
      callResultJson = JSON.stringify(result, null, 2);
    } catch (err) {
      errorMsg = String(err);
    }
  }

  // ---------------- LLM TEST STATE ----------------
  let llmReply = "";
  let llmError = "";
  let llmLoading = false;

  async function testLlm() {
    llmReply = "";
    llmError = "";
    llmLoading = true;

    try {
      const result = await invoke("assistant_request", {
        prompt: "Say hi briefly."
      }) as { reply: string; steps: unknown[] };

      llmReply = result.reply;
    } catch (err) {
      llmError = String(err);
    } finally {
      llmLoading = false;
    }
  }
    // ---------------- SIMPLE CHAT STATE ----------------
  type Message = { role: "user" | "assistant"; content: string };

  let chatMessages: Message[] = [];
  let chatInput = "";
  let chatLoading = false;
  let chatError = "";

  async function sendChat() {
  const trimmed = chatInput.trim();
  if (!trimmed || chatLoading) return;

  chatError = "";
  const userMessage: Message = { role: "user", content: trimmed };
  chatMessages = [...chatMessages, userMessage];
  chatInput = "";
  chatLoading = true;

  try {
    const result = await invoke("assistant_request", {
      prompt: userMessage.content
    }) as {
      reply: string;
      plan: any;
      tool_results: any[];
    };

    console.log("assistant_response", result);      // <-- add this line

    const assistantMessage: Message = {
      role: "assistant",
      content: result.reply
    };

    chatMessages = [...chatMessages, assistantMessage];

    console.log("Plan:", result.plan);
    console.log("Tool Results:", result.tool_results);
  } catch (err) {
    chatError = String(err);
  } finally {
    chatLoading = false;
  }
}

async function callGetImageMetadata() {
  errorMsg = "";
  try {
    const result = await invoke("mcp_call_tool", {
      name: "get_image_metadata",
      arguments: {}
    });

    callResultJson = JSON.stringify(result, null, 2);

  } catch (err) {
    errorMsg = String(err);
  }
}

  function handleChatKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      sendChat();
    }
  }

</script>

<main style="padding: 1.5rem; font-family: system-ui; max-width: 900px; margin: 0 auto;">
  <h1>GIMP MCP stdio test</h1>

  <!-- ---------------- MCP BUTTONS ---------------- -->
  <div style="display: flex; gap: 1rem; margin: 1rem 0;">
    <button on:click={listTools}>List MCP tools</button>
    <button on:click={callGetGimpInfo}>Call get_gimp_info()</button>
    <button on:click={callGetImageMetadata}>Call get_image_metadata()</button>
  </div>

  {#if errorMsg}
    <p style="color: red;">{errorMsg}</p>
  {/if}

  <!-- ---------------- LLM TEST SECTION ---------------- -->
  <hr style="margin: 1.5rem 0;" />

  <h2>LLM test</h2>

  <button on:click={testLlm} disabled={llmLoading}>
    {llmLoading ? "Asking LLM..." : "Test LLM"}
  </button>

  {#if llmError}
    <p style="color: red;">{llmError}</p>
  {/if}

  {#if llmReply}
    <p><strong>LLM reply:</strong> {llmReply}</p>
  {/if}

  <!-- ---------------- MCP RESULTS ---------------- -->
  <section style="margin-top: 1rem;">
    <h2>tools/list result</h2>
    <pre>{toolsJson}</pre>
  </section>

  <section style="margin-top: 1rem;">
    <h2>Tool call result</h2>
    <pre>{callResultJson}</pre>
  </section>

  <!-- ---------------- CHAT UI ---------------- -->
  <hr style="margin: 1.5rem 0;" />

  <section>
    <h2>Assistant chat (LLM only, no tools yet)</h2>

    <div style="border: 1px solid #ccc; border-radius: 8px; padding: 1rem; min-height: 200px;">
      {#if chatMessages.length === 0}
        <p style="color: #666;">Ask something like: “What kind of edits can you help me do in GIMP?”</p>
      {/if}

      {#each chatMessages as msg}
        <div style="margin-bottom: 0.75rem;">
          <strong>{msg.role === "user" ? "You" : "Assistant"}:</strong>
          <div>{msg.content}</div>
        </div>
      {/each}

      {#if chatLoading}
        <p>Assistant is thinking…</p>
      {/if}
    </div>

    {#if chatError}
      <p style="color: red; margin-top: 0.5rem;">{chatError}</p>
    {/if}

    <form
      on:submit|preventDefault={sendChat}
      style="margin-top: 1rem; display: flex; gap: 0.5rem; align-items: flex-end;"
    >
      <textarea
        bind:value={chatInput}
        on:keydown={handleChatKeydown}
        rows={2}
        style="flex: 1; resize: vertical;"
        placeholder="Describe what you want to do in GIMP and press Enter…"
      ></textarea>

      <button type="submit" disabled={chatLoading}>
        {chatLoading ? "Sending..." : "Send"}
      </button>
    </form>
  </section>
</main>
