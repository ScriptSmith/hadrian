/**
 * Tracked Fetch Wrapper
 *
 * A drop-in replacement for fetch that automatically records API calls
 * to the coverage tracker.
 */
import { coverageTracker } from "./coverage-tracker";

/**
 * Wrapper around the native fetch that records API calls for coverage tracking.
 * Use this in place of fetch() when making direct API calls outside the SDK.
 *
 * @example
 * ```typescript
 * import { trackedFetch } from '../utils/tracked-fetch';
 *
 * // Use like regular fetch
 * const response = await trackedFetch(`${baseUrl}/admin/v1/users`, {
 *   method: 'GET',
 *   headers: { Authorization: `Bearer ${token}` },
 * });
 * ```
 */
export async function trackedFetch(
  input: RequestInfo | URL,
  init?: RequestInit
): Promise<Response> {
  const url = getUrlFromInput(input);
  const method = getMethod(input, init);

  let body: unknown;
  if (init?.body && typeof init.body === "string") {
    try {
      body = JSON.parse(init.body);
    } catch {
      // Not JSON, ignore body params
    }
  }

  // Make the actual request
  const response = await fetch(input, init);

  // Record the call with response status
  coverageTracker.recordFromFetch(method, url, body, response.status);

  return response;
}

/**
 * Extract URL from fetch input.
 */
function getUrlFromInput(input: RequestInfo | URL): URL {
  if (input instanceof URL) {
    return input;
  }
  if (input instanceof Request) {
    return new URL(input.url);
  }
  // string
  return new URL(input);
}

/**
 * Get the HTTP method from fetch arguments.
 */
function getMethod(input: RequestInfo | URL, init?: RequestInit): string {
  if (init?.method) {
    return init.method.toUpperCase();
  }
  if (input instanceof Request) {
    return input.method.toUpperCase();
  }
  return "GET";
}

/**
 * Creates a version of fetch that's pre-bound to a base URL.
 * Useful for creating test utilities.
 *
 * @example
 * ```typescript
 * const apiFetch = createTrackedFetch('http://localhost:3000');
 *
 * const response = await apiFetch('/admin/v1/users', {
 *   method: 'GET',
 *   headers: { Authorization: `Bearer ${token}` },
 * });
 * ```
 */
export function createTrackedFetch(
  baseUrl: string
): (path: string, init?: RequestInit) => Promise<Response> {
  return async (path: string, init?: RequestInit) => {
    const url = new URL(path, baseUrl);
    return trackedFetch(url, init);
  };
}
