import { createClient, createConfig, type Client } from "../client/client";
import { waitForHealthy } from "./wait-for";
import { coverageTracker } from "../utils/coverage-tracker";

export interface GatewayContext {
  url: string;
  client: Client;
  adminClient: (token: string) => Client;
  apiKeyClient: (apiKey: string) => Client;
}

/**
 * Install coverage tracking interceptors on a client.
 * Captures URL patterns, parameters, and response status codes.
 */
export function installCoverageInterceptors(client: Client): void {
  // We need to capture request details and correlate with response status codes
  // Using a WeakMap to store request metadata keyed by Request object
  const requestMeta = new WeakMap<
    Request,
    {
      method: string;
      url: string;
      path?: Record<string, unknown>;
      query?: Record<string, unknown>;
      body?: unknown;
    }
  >();

  // Request interceptor: capture URL pattern and parameters
  client.interceptors.request.use((request, options) => {
    requestMeta.set(request, {
      method: request.method,
      url: options.url,
      path: options.path as Record<string, unknown> | undefined,
      query: options.query as Record<string, unknown> | undefined,
      body: options.body,
    });
    return request;
  });

  // Response interceptor: record the call with status code
  client.interceptors.response.use((response, request, _options) => {
    const meta = requestMeta.get(request);
    if (meta) {
      coverageTracker.recordFromInterceptor(
        meta.method,
        meta.url,
        meta.path,
        meta.query,
        meta.body,
        response.status
      );
      // Clean up to avoid memory leaks
      requestMeta.delete(request);
    }
    return response;
  });
}

/**
 * Create a client with coverage tracking enabled.
 * Export this function for use in tests that create their own clients.
 */
export function createTrackedClient(
  config: ReturnType<typeof createConfig>
): Client {
  const client = createClient(config);
  installCoverageInterceptors(client);
  return client;
}

export async function setupGateway(baseUrl: string): Promise<GatewayContext> {
  await waitForHealthy(`${baseUrl}/health`, {
    maxRetries: 60,
    retryInterval: 2000,
  });

  const client = createTrackedClient(
    createConfig({
      baseUrl,
    })
  );

  return {
    url: baseUrl,
    client,
    adminClient: (token: string) =>
      createTrackedClient(
        createConfig({
          baseUrl,
          headers: { Authorization: `Bearer ${token}` },
        })
      ),
    apiKeyClient: (apiKey: string) =>
      createTrackedClient(
        createConfig({
          baseUrl,
          headers: { Authorization: `Bearer ${apiKey}` },
        })
      ),
  };
}
