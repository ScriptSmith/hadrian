/**
 * Convert any thrown value into a human-readable string for error toasts.
 *
 * `String(error)` produces "[object Object]" for most non-string, non-Error
 * values — including the typed error bodies that hey-api / fetch wrappers
 * surface. This helper unwraps the common shapes:
 *   - plain `Error` → `error.message`
 *   - hey-api errors with `.body` → drill into the body
 *   - API error envelopes (`{message}`, `{detail}`, `{error: string}`,
 *     `{error: {message}}`)
 *   - strings as-is
 *
 * Always returns a non-empty string so callers can pass the result straight
 * to a toast description without an additional fallback.
 */
export function formatApiError(error: unknown): string {
  if (typeof error === "string") return error || "Unknown error";
  if (error == null) return "Unknown error";

  if (error instanceof Error) {
    const fromBody = extractMessage((error as Error & { body?: unknown }).body);
    if (fromBody) return fromBody;
    return error.message || "Unknown error";
  }

  if (typeof error === "object") {
    const fromBody = extractMessage(error);
    if (fromBody) return fromBody;
  }

  const fallback = String(error);
  return fallback === "[object Object]" ? "Unknown error" : fallback;
}

function extractMessage(body: unknown): string | null {
  if (typeof body === "string") return body || null;
  if (body == null || typeof body !== "object") return null;

  const obj = body as Record<string, unknown>;
  if (typeof obj.message === "string" && obj.message) return obj.message;
  if (typeof obj.detail === "string" && obj.detail) return obj.detail;
  if (typeof obj.error === "string" && obj.error) return obj.error;
  if (typeof obj.error === "object" && obj.error) {
    const inner = obj.error as Record<string, unknown>;
    if (typeof inner.message === "string" && inner.message) return inner.message;
    if (typeof inner.detail === "string" && inner.detail) return inner.detail;
  }
  return null;
}
