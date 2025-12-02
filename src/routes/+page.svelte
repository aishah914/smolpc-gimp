<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  let status = "GIMP MCP server not started yet.";

  async function onStartServer() {
    status = "Starting GIMP MCP server...";
    try {
      await invoke("start_gimp_mcp_server");
      status = "GIMP MCP server started (Python process spawned).";
    } catch (e) {
      console.error(e);
      status = "Failed to start GIMP MCP server: " + e;
    }
  }
</script>

<main style="padding: 2rem; font-family: system-ui;">
  <h1>SmolPC – GIMP AI Assistant (Phase 2)</h1>

  <button on:click={onStartServer}>
    Start GIMP MCP Server
  </button>

  <p style="margin-top: 1rem; white-space: pre-wrap;">
    {status}
  </p>

  <p style="margin-top: 2rem; font-size: 0.9rem; opacity: 0.7;">
    Make sure:
    <br />1. GIMP 3.0.6 is open.
    <br />2. In GIMP, go to Tools → Start MCP Server.
    <br />3. Then click the button above to start the Python MCP bridge.
  </p>
</main>
