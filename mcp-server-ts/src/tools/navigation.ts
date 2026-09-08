import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { socketClient } from "./client.js";
import { createErrorResponse, createSuccessResponse, formatResultAsText, logCommandParams } from "./response-helpers.js";

// Every tool here resolves `window_label` against the app's whole webview
// registry, so a child webview (an in-app browser tab, label `browser::<id>`)
// is addressable the same way a top-level window is.
const windowLabel = z
  .string()
  .default("main")
  .describe("The label of the webview to act on. Child webviews (in-app browser tabs) use their own label, e.g. 'browser::1'. Defaults to 'main'.");

export function registerNavigationTools(server: McpServer) {
  server.tool(
    "load_uri",
    "Navigates a webview to an http or https URL. Other schemes (file:, data:, javascript:) are rejected by the app.",
    {
      url: z.string().describe("Required. The http(s) URL to navigate to."),
      window_label: windowLabel,
    },
    {
      title: "Navigate a Webview to a URL",
      readOnlyHint: false,
      destructiveHint: true,
      idempotentHint: true,
      openWorldHint: true,
    },
    async ({ url, window_label }) => {
      try {
        logCommandParams("load_uri", { url, window_label });
        const result = await socketClient.sendCommand("load_uri", { url, window_label });
        return createSuccessResponse(formatResultAsText(result));
      } catch (error) {
        return createErrorResponse(`Failed to navigate: ${(error as Error).message}`);
      }
    },
  );

  server.tool(
    "go_back",
    "Moves a webview one entry back in its session history. The webview cannot report whether an entry existed, so the response returns history.length instead: a length of 1 means the call was a no-op.",
    { window_label: windowLabel },
    {
      title: "Navigate Back",
      readOnlyHint: false,
      destructiveHint: false,
      idempotentHint: false,
      openWorldHint: true,
    },
    async ({ window_label }) => {
      try {
        logCommandParams("go_back", { window_label });
        const result = await socketClient.sendCommand("go_back", { window_label });
        return createSuccessResponse(formatResultAsText(result));
      } catch (error) {
        return createErrorResponse(`Failed to go back: ${(error as Error).message}`);
      }
    },
  );

  server.tool(
    "go_forward",
    "Moves a webview one entry forward in its session history. As with go_back, the response returns history.length rather than whether the move happened.",
    { window_label: windowLabel },
    {
      title: "Navigate Forward",
      readOnlyHint: false,
      destructiveHint: false,
      idempotentHint: false,
      openWorldHint: true,
    },
    async ({ window_label }) => {
      try {
        logCommandParams("go_forward", { window_label });
        const result = await socketClient.sendCommand("go_forward", { window_label });
        return createSuccessResponse(formatResultAsText(result));
      } catch (error) {
        return createErrorResponse(`Failed to go forward: ${(error as Error).message}`);
      }
    },
  );

  server.tool(
    "get_url",
    "Reads the URL a webview is currently showing.",
    { window_label: windowLabel },
    {
      title: "Read the Current Webview URL",
      readOnlyHint: true,
      destructiveHint: false,
      idempotentHint: true,
      openWorldHint: false,
    },
    async ({ window_label }) => {
      try {
        logCommandParams("get_url", { window_label });
        const result = await socketClient.sendCommand("get_url", { window_label });
        return createSuccessResponse(formatResultAsText(result));
      } catch (error) {
        return createErrorResponse(`Failed to read the URL: ${(error as Error).message}`);
      }
    },
  );
}
