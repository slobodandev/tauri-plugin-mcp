#!/usr/bin/env node
// Minimal one-shot MCP client for the tauri-plugin-mcp stdio server.
// Usage: node tauri-mcp-cli.mjs <toolName> '<jsonArgsOrEmpty>'   (or `__list__`)
// TAURI_MCP_IPC_PATH picks the app socket (default: DIY's /tmp/tauri-mcp-diy.sock).
// Needs `pnpm build` in this directory first.
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { fileURLToPath } from "node:url";

const [toolName, argsJson] = process.argv.slice(2);
const args = argsJson ? JSON.parse(argsJson) : {};

const transport = new StdioClientTransport({
  command: "node",
  args: [fileURLToPath(new URL("./build/index.js", import.meta.url))],
  env: { ...process.env, TAURI_MCP_IPC_PATH: process.env.TAURI_MCP_IPC_PATH ?? "/tmp/tauri-mcp-diy.sock" },
});

const client = new Client({ name: "cli", version: "0.0.1" });
await client.connect(transport);

if (toolName === "__list__") {
  const r = await client.listTools();
  console.log(JSON.stringify(r.tools.map((t) => t.name), null, 2));
} else {
  const r = await client.callTool({ name: toolName, arguments: args });
  for (const c of r.content ?? []) {
    if (c.type === "text") console.log(c.text);
    else if (c.type === "image") {
      const buf = Buffer.from(c.data, "base64");
      const out = `/tmp/tauri-shot-${Date.now()}.${c.mimeType?.includes("png") ? "png" : "jpg"}`;
      await import("node:fs/promises").then((fs) => fs.writeFile(out, buf));
      console.log("IMAGE_SAVED:" + out);
    } else {
      console.log(JSON.stringify(c));
    }
  }
}
await client.close();
process.exit(0);
