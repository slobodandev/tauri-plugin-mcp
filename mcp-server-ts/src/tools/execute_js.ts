import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { socketClient } from "./client.js";
import { createErrorResponse, createSuccessResponse, formatResultAsText, logCommandParams } from "./response-helpers.js";

export function registerExecuteJsTool(server: McpServer) {
  server.tool(
    "execute_js",
    "Executes arbitrary JavaScript in a webview and returns the value as JSON, alongside its JS `typeof`. A thrown exception comes back as an error, not as a null result. Caution: This tool is destructive and can modify the window's content, state, or trigger unintended actions. Use with careful consideration of the code being executed.",
    {
      code: z.string().describe("Required. JavaScript to evaluate. A bare expression ('document.title') returns its value; several statements need an explicit `return`, and may use `await`. A returned promise is awaited up to timeout_ms."),
      window_label: z.string().default("main").describe("The label of the webview to evaluate in. Child webviews (in-app browser tabs) use their own label, e.g. 'browser::1'. Defaults to 'main'."),
      timeout_ms: z.number().int().positive().optional().describe("The maximum time in milliseconds to allow for the JavaScript execution. If the script exceeds this timeout, its execution will be terminated, and an error may be returned."),
    },
    {
      title: "Execute JavaScript Code in Specified Application Window",
      readOnlyHint: false,
      destructiveHint: true,
      idempotentHint: false,
      openWorldHint: false,
    },
    async ({ code, window_label, timeout_ms }) => {
      try {
        // Validate required parameters
        if (!code || code.trim() === '') {
          return createErrorResponse("The code parameter is required and cannot be empty");
        }
        
        const params = { code, window_label, timeout_ms };
        logCommandParams('execute_js', params);
        
        // Use default window label if not provided
        const effectiveWindowLabel = window_label || 'main';
        
        const result = await socketClient.sendCommand('execute_js', {
          code,
          window_label: effectiveWindowLabel,
          timeout_ms
        });
        
        console.error(`Got JS execution result type: ${typeof result}`);
        
        return createSuccessResponse(formatResultAsText(result));
      } catch (error) {
        console.error('JS execution error:', error);
        return createErrorResponse(`Failed to execute JavaScript: ${(error as Error).message}`);
      }
    },
  );
} 