# Adding a Frontend Tool

This guide covers adding new client-side tools to the chat UI. Frontend tools execute in the browser (no backend required) and can make external API calls.

## Overview

Frontend tools consist of 4 parts:
1. **Tool Executor** - The function that executes the tool
2. **Tool Metadata** - UI display info (name, description, icon)
3. **Tool Definition** - OpenAI function schema sent to the LLM
4. **Execution Flag** - Enable client-side tool execution when tool is active

## Step 1: Create the Tool Executor

Add your executor to `ui/src/pages/chat/utils/toolExecutors.ts`:

```typescript
/**
 * Arguments for your tool
 */
interface MyToolArguments {
  query: string;
  // Add other parameters...
}

/**
 * Execute the my_tool tool
 */
export const myToolExecutor: ToolExecutor = async (
  toolCall,
  context
): Promise<ToolExecutionResult> => {
  if (toolCall.name !== "my_tool") {
    return {
      success: false,
      error: `Expected my_tool tool, got ${toolCall.name}`,
    };
  }

  const args = toolCall.arguments as unknown as MyToolArguments;
  const { query } = args;

  // Validate arguments
  if (!query || typeof query !== "string") {
    return {
      success: false,
      error: "No query provided",
      output: JSON.stringify({ error: "No query provided" }),
    };
  }

  const toolId = toolCall.id || `mytool-${Date.now()}`;

  try {
    // Show status to user
    context.onStatusMessage?.(toolId, "Processing...");

    // Execute the tool logic (API calls, processing, etc.)
    const result = await fetch("https://api.example.com/...");
    const data = await result.json();

    // Clear status
    context.onStatusMessage?.(toolId, "");

    // Return result (output is sent back to the LLM)
    return {
      success: true,
      output: JSON.stringify(data),
      // Optional: artifacts for UI display (charts, tables, etc.)
      // artifacts: [{ id: "...", type: "table", data: {...} }],
    };
  } catch (error) {
    context.onStatusMessage?.(toolId, "");
    const errorMsg = error instanceof Error ? error.message : String(error);
    return {
      success: false,
      error: errorMsg,
      output: JSON.stringify({ error: errorMsg }),
    };
  }
};
```

## Step 2: Register the Executor

In `ui/src/pages/chat/utils/toolExecutors.ts`, add to `defaultToolExecutors`:

```typescript
export const defaultToolExecutors: ToolExecutorRegistry = {
  // ... existing tools
  my_tool: myToolExecutor,
};
```

## Step 3: Add Tool Metadata

In `ui/src/pages/chat/utils/toolExecutors.ts`, add to `TOOL_METADATA`:

```typescript
export const TOOL_METADATA: ToolMetadata[] = [
  // ... existing tools
  {
    id: "my_tool",
    name: "My Tool",
    description: "Does something useful with external APIs",
    icon: "Wrench",  // Lucide icon name (ignored if custom icon exists)
    implemented: true,
  },
];
```

## Step 4: Add Custom Icon (Optional)

For custom icons, edit `ui/src/components/ToolIcons/ToolIcons.tsx`:

```typescript
/** My Tool icon */
export function MyToolIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} fill="none">
      <rect x="2" y="2" width="20" height="20" rx="4" stroke="currentColor" strokeWidth="1.5" />
      <text x="12" y="16" textAnchor="middle" fontSize="10" fontWeight="700" fill="currentColor">
        MT
      </text>
    </svg>
  );
}

// Add to TOOL_ICON_MAP:
export const TOOL_ICON_MAP: Record<string, ToolIconComponent> = {
  // ... existing tools
  my_tool: MyToolIcon,
};

// Add to TOOL_SHORT_NAMES:
export const TOOL_SHORT_NAMES: Record<string, string> = {
  // ... existing tools
  my_tool: "MyTool",
};
```

## Step 5: Add Tool Definition for LLM

In `ui/src/pages/chat/useChat.ts`, add the tool definition inside `streamResponse`:

```typescript
// Add my_tool as a function tool
if (enabledTools.includes("my_tool")) {
  tools.push({
    type: "function",
    name: "my_tool",
    description:
      "Describe what this tool does and when to use it. " +
      "Be specific about inputs and expected outputs.",
    parameters: {
      type: "object",
      properties: {
        query: {
          type: "string",
          description: "The search query or input",
        },
        // Add other parameters...
      },
      required: ["query"],
    },
  });
}
```

## Step 6: Enable Client-Side Execution

**IMPORTANT**: This step is often forgotten and causes tools to not execute!

In `ui/src/pages/chat/ChatPage.tsx`, add your tool to the `clientSideToolExecution` condition:

```typescript
const clientSideToolExecution =
  clientSideRAG ||
  enabledTools.includes("code_interpreter") ||
  // ... existing tools
  enabledTools.includes("my_tool");  // <-- Add this line
```

Without this, the tool definition is sent to the LLM but tool calls won't be executed.

## File Checklist

When adding a new frontend tool, update these files:

- [ ] `ui/src/pages/chat/utils/toolExecutors.ts`
  - [ ] Add executor function
  - [ ] Register in `defaultToolExecutors`
  - [ ] Add to `TOOL_METADATA`
- [ ] `ui/src/components/ToolIcons/ToolIcons.tsx`
  - [ ] Add custom icon component (optional)
  - [ ] Add to `TOOL_ICON_MAP`
  - [ ] Add to `TOOL_SHORT_NAMES`
- [ ] `ui/src/pages/chat/useChat.ts`
  - [ ] Add tool definition in `streamResponse` function
- [ ] `ui/src/pages/chat/ChatPage.tsx`
  - [ ] Add to `clientSideToolExecution` condition

## Testing

1. Run `pnpm lint:fix` and `pnpm tsc --noEmit` to check for errors
2. Start the dev server with `pnpm dev`
3. Enable the tool in the ToolsBar (hover over wrench icon)
4. Ask the LLM to use the tool
5. Verify the tool executes and returns results

## External API Considerations

When calling external APIs from the browser:

1. **CORS**: The API must allow cross-origin requests (Access-Control-Allow-Origin: *)
2. **No Auth Secrets**: Don't embed API keys in frontend code
3. **Rate Limits**: Respect the API's rate limits
4. **User-Agent**: Some APIs require a User-Agent header (use `Api-User-Agent` for browser)
5. **Terms of Service**: Ensure your usage complies with the API's TOS
