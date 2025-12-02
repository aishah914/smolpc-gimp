<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

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
        arguments: {}, // no args for that tool
      });
      callResultJson = JSON.stringify(result, null, 2);
    } catch (err) {
      errorMsg = String(err);
    }
  }
</script>

<main style="padding: 1.5rem; font-family: system-ui;">
  <h1>GIMP MCP stdio test</h1>

  <div style="display: flex; gap: 1rem; margin: 1rem 0;">
    <button on:click={listTools}>List MCP tools</button>
    <button on:click={callGetGimpInfo}>Call get_gimp_info()</button>
  </div>

  {#if errorMsg}
    <p style="color: red;">{errorMsg}</p>
  {/if}

  <section style="margin-top: 1rem;">
    <h2>tools/list result</h2>
    <pre>{toolsJson}</pre>
  </section>

  <section style="margin-top: 1rem;">
    <h2>get_gimp_info result</h2>
    <pre>{callResultJson}</pre>
  </section>
</main>
