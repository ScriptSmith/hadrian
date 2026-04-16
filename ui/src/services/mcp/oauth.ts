/**
 * MCP OAuth Authorization Code + PKCE Flow
 *
 * Implements OAuth 2.1 PKCE authorization for MCP servers per the MCP spec.
 * Handles metadata discovery, dynamic client registration, token exchange,
 * refresh, and popup-based authorization.
 *
 * @see https://modelcontextprotocol.io/specification/draft/basic/authorization
 */

import type { OAuthTokens, MCPOAuthConfig, MCPAuthType } from "./types";

// =============================================================================
// PKCE Helpers
// =============================================================================

function base64url(bytes: Uint8Array): string {
  let binary = "";
  for (const b of bytes) binary += String.fromCharCode(b);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function generateCodeVerifier(): string {
  return base64url(crypto.getRandomValues(new Uint8Array(32)));
}

async function generateCodeChallenge(verifier: string): Promise<string> {
  const encoded = new TextEncoder().encode(verifier);
  const digest = await crypto.subtle.digest("SHA-256", encoded);
  return base64url(new Uint8Array(digest));
}

function generateState(): string {
  return "mcp-" + base64url(crypto.getRandomValues(new Uint8Array(16)));
}

// =============================================================================
// Discovery Types
// =============================================================================

interface ProtectedResourceMetadata {
  resource: string;
  resource_name?: string;
  authorization_servers?: string[];
  scopes_supported?: string[];
}

interface AuthServerMetadata {
  issuer: string;
  authorization_endpoint: string;
  token_endpoint: string;
  registration_endpoint?: string;
  scopes_supported?: string[];
  response_types_supported?: string[];
  grant_types_supported?: string[];
  code_challenge_methods_supported?: string[];
}

interface ClientRegistration {
  client_id: string;
  client_secret?: string;
  client_secret_expires_at?: number;
}

// =============================================================================
// Storage
// =============================================================================

/** Stored alongside tokens for refresh without re-discovery */
interface StoredOAuthData {
  tokens: OAuthTokens;
  tokenEndpoint: string;
  clientId: string;
  clientSecret?: string;
  resource: string;
}

/** Transient flow data stored while popup is open */
interface PendingFlow {
  serverUrl: string;
  codeVerifier: string;
  redirectUri: string;
  tokenEndpoint: string;
  clientId: string;
  clientSecret?: string;
  resource: string;
}

const STORAGE_PREFIX = "hadrian-mcp-oauth";

function oauthDataKey(serverUrl: string): string {
  return `${STORAGE_PREFIX}::${serverUrl}`;
}

function clientRegKey(issuer: string): string {
  return `${STORAGE_PREFIX}-client::${issuer}`;
}

function pendingFlowKey(state: string): string {
  return `${STORAGE_PREFIX}-pending::${state}`;
}

function getStoredOAuthData(serverUrl: string): StoredOAuthData | null {
  try {
    const raw = localStorage.getItem(oauthDataKey(serverUrl));
    return raw ? JSON.parse(raw) : null;
  } catch {
    return null;
  }
}

function storeOAuthData(serverUrl: string, data: StoredOAuthData): void {
  localStorage.setItem(oauthDataKey(serverUrl), JSON.stringify(data));
}

/** Get stored tokens for a server URL */
export function getStoredTokens(serverUrl: string): OAuthTokens | null {
  return getStoredOAuthData(serverUrl)?.tokens ?? null;
}

/** Clear all stored OAuth data for a server */
export function clearOAuthData(serverUrl: string): void {
  localStorage.removeItem(oauthDataKey(serverUrl));
}

function getStoredClientReg(issuer: string): ClientRegistration | null {
  try {
    const raw = localStorage.getItem(clientRegKey(issuer));
    if (!raw) return null;
    const reg = JSON.parse(raw) as ClientRegistration;
    // Check expiry
    if (
      reg.client_secret_expires_at &&
      reg.client_secret_expires_at !== 0 &&
      reg.client_secret_expires_at * 1000 < Date.now()
    ) {
      localStorage.removeItem(clientRegKey(issuer));
      return null;
    }
    return reg;
  } catch {
    return null;
  }
}

function storeClientReg(issuer: string, reg: ClientRegistration): void {
  localStorage.setItem(clientRegKey(issuer), JSON.stringify(reg));
}

function storePendingFlow(state: string, flow: PendingFlow): void {
  localStorage.setItem(pendingFlowKey(state), JSON.stringify(flow));
}

function getPendingFlow(state: string): PendingFlow | null {
  try {
    const raw = localStorage.getItem(pendingFlowKey(state));
    return raw ? JSON.parse(raw) : null;
  } catch {
    return null;
  }
}

function clearPendingFlow(state: string): void {
  localStorage.removeItem(pendingFlowKey(state));
}

// =============================================================================
// Token Helpers
// =============================================================================

/** Check if an OAuth token has expired (with 30s buffer) */
export function isTokenExpired(tokens: OAuthTokens): boolean {
  if (tokens.expires_in == null) return false;
  const expiresAt = tokens.obtained_at + tokens.expires_in * 1000;
  return Date.now() > expiresAt - 30_000;
}

/** Check if a server URL has valid (non-expired) OAuth tokens */
export function hasValidTokens(serverUrl: string): boolean {
  const tokens = getStoredTokens(serverUrl);
  return tokens !== null && !isTokenExpired(tokens);
}

// =============================================================================
// Discovery
// =============================================================================

async function fetchJson<T>(url: string): Promise<T | null> {
  try {
    const res = await fetch(url);
    if (!res.ok) return null;
    return (await res.json()) as T;
  } catch {
    return null;
  }
}

/** Validate that a URL shares the same origin as the server (SSRF protection) */
function isSameOrigin(targetUrl: string, serverUrl: string): boolean {
  try {
    return new URL(targetUrl).origin === new URL(serverUrl).origin;
  } catch {
    return false;
  }
}

/**
 * Discover Protected Resource Metadata (RFC 9728).
 * Tries path-aware well-known URL first, then root.
 */
async function discoverProtectedResourceMetadata(
  serverUrl: string
): Promise<ProtectedResourceMetadata | null> {
  const url = new URL(serverUrl);

  // Also try 401 response from the MCP endpoint for WWW-Authenticate header
  let fromWwwAuth: ProtectedResourceMetadata | null = null;
  try {
    const res = await fetch(serverUrl, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: 0, method: "initialize", params: {} }),
    });
    if (res.status === 401) {
      const wwwAuth = res.headers.get("WWW-Authenticate") ?? "";
      const resourceMetaUrl = wwwAuth.match(/resource_metadata="([^"]+)"/)?.[1];
      if (resourceMetaUrl && isSameOrigin(resourceMetaUrl, serverUrl)) {
        fromWwwAuth = await fetchJson<ProtectedResourceMetadata>(resourceMetaUrl);
      }
      // Extract scope hint from WWW-Authenticate as a fallback.
      // Note: this is the scope required for this specific request, not
      // necessarily all scopes the resource supports.
      if (!fromWwwAuth) {
        const scopeHint = wwwAuth.match(/scope="([^"]+)"/)?.[1];
        if (scopeHint) {
          fromWwwAuth = {
            resource: serverUrl,
            scopes_supported: scopeHint.split(" "),
          };
        }
      }
    }
  } catch {
    // Continue to well-known discovery
  }

  if (fromWwwAuth?.authorization_servers?.length) return fromWwwAuth;

  // Well-known discovery
  const paths =
    url.pathname !== "/" && url.pathname !== ""
      ? [
          `${url.origin}/.well-known/oauth-protected-resource${url.pathname}`,
          `${url.origin}/.well-known/oauth-protected-resource`,
        ]
      : [`${url.origin}/.well-known/oauth-protected-resource`];

  for (const path of paths) {
    const meta = await fetchJson<ProtectedResourceMetadata>(path);
    if (meta?.authorization_servers?.length) return meta;
  }

  // If we got partial metadata from WWW-Authenticate (scope but no AS), still useful
  return fromWwwAuth;
}

/**
 * Discover Authorization Server Metadata (RFC 8414 / OIDC Discovery).
 * Tries OAuth 2.0 well-known first, then OIDC.
 */
async function discoverAuthServerMetadata(issuerUrl: string): Promise<AuthServerMetadata | null> {
  const url = new URL(issuerUrl);

  const paths =
    url.pathname !== "/" && url.pathname !== ""
      ? [
          `${url.origin}/.well-known/oauth-authorization-server${url.pathname}`,
          `${url.origin}/.well-known/openid-configuration${url.pathname}`,
          `${issuerUrl}/.well-known/openid-configuration`,
        ]
      : [
          `${url.origin}/.well-known/oauth-authorization-server`,
          `${url.origin}/.well-known/openid-configuration`,
        ];

  for (const path of paths) {
    const meta = await fetchJson<AuthServerMetadata>(path);
    if (meta?.authorization_endpoint && meta.token_endpoint) return meta;
  }

  return null;
}

// =============================================================================
// Dynamic Client Registration (RFC 7591)
// =============================================================================

async function dynamicClientRegistration(
  registrationEndpoint: string,
  redirectUri: string
): Promise<ClientRegistration> {
  const res = await fetch(registrationEndpoint, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      redirect_uris: [redirectUri],
      grant_types: ["authorization_code", "refresh_token"],
      response_types: ["code"],
      application_type: "web",
      client_name: "Hadrian Gateway",
      token_endpoint_auth_method: "none",
    }),
  });

  if (!res.ok) {
    const body = await res.text().catch(() => "");
    throw new Error(`Dynamic client registration failed (${res.status}): ${body}`);
  }

  return (await res.json()) as ClientRegistration;
}

// =============================================================================
// Token Exchange & Refresh
// =============================================================================

async function exchangeCodeForTokens(
  tokenEndpoint: string,
  code: string,
  redirectUri: string,
  clientId: string,
  codeVerifier: string,
  resource: string,
  clientSecret?: string
): Promise<OAuthTokens> {
  const params: Record<string, string> = {
    grant_type: "authorization_code",
    code,
    redirect_uri: redirectUri,
    client_id: clientId,
    code_verifier: codeVerifier,
    resource,
  };
  if (clientSecret) params.client_secret = clientSecret;

  const res = await fetch(tokenEndpoint, {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams(params),
  });

  if (!res.ok) {
    const body = await res.text().catch(() => "");
    throw new Error(`Token exchange failed (${res.status}): ${body}`);
  }

  const data = await res.json();
  return { ...data, obtained_at: Date.now() };
}

async function refreshTokenRequest(
  tokenEndpoint: string,
  refreshToken: string,
  clientId: string,
  resource: string,
  clientSecret?: string
): Promise<OAuthTokens> {
  const params: Record<string, string> = {
    grant_type: "refresh_token",
    refresh_token: refreshToken,
    client_id: clientId,
    resource,
  };
  if (clientSecret) params.client_secret = clientSecret;

  const res = await fetch(tokenEndpoint, {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams(params),
  });

  if (!res.ok) {
    const body = await res.text().catch(() => "");
    throw new Error(`Token refresh failed (${res.status}): ${body}`);
  }

  const data = await res.json();
  return { ...data, obtained_at: Date.now() };
}

// =============================================================================
// OAuth Flow Orchestration
// =============================================================================

/**
 * Start the OAuth Authorization Code + PKCE flow for an MCP server.
 *
 * Opens a popup for user authorization and returns the obtained tokens.
 * The popup redirects back to the app origin, which forwards the callback
 * to the opener via postMessage.
 */
export async function startOAuthFlow(
  serverUrl: string,
  oauthConfig?: MCPOAuthConfig
): Promise<OAuthTokens> {
  // 1. Discover metadata
  const prm = await discoverProtectedResourceMetadata(serverUrl);
  if (!prm?.authorization_servers?.length) {
    throw new Error(
      "Could not discover OAuth metadata. The server may not support OAuth authorization."
    );
  }

  const resource = prm.resource || serverUrl;
  const issuerUrl = prm.authorization_servers[0];

  const asm = await discoverAuthServerMetadata(issuerUrl);
  if (!asm) {
    throw new Error(`Could not discover authorization server metadata at ${issuerUrl}`);
  }

  // Validate PKCE S256 support
  if (
    asm.code_challenge_methods_supported &&
    !asm.code_challenge_methods_supported.includes("S256")
  ) {
    throw new Error("Authorization server does not support S256 PKCE code challenge method.");
  }

  // 2. Resolve client_id
  const redirectUri = `${window.location.origin}/`;
  let clientId = oauthConfig?.clientId;
  let clientSecret = oauthConfig?.clientSecret;

  if (!clientId) {
    const storedReg = getStoredClientReg(asm.issuer);
    if (storedReg) {
      clientId = storedReg.client_id;
      clientSecret = clientSecret ?? storedReg.client_secret;
    }
  }

  if (!clientId && asm.registration_endpoint) {
    const reg = await dynamicClientRegistration(asm.registration_endpoint, redirectUri);
    storeClientReg(asm.issuer, reg);
    clientId = reg.client_id;
    clientSecret = clientSecret ?? reg.client_secret;
  }

  if (!clientId) {
    throw new Error(
      "No client ID available. The server does not support dynamic registration. " +
        "Please provide a Client ID in the OAuth settings."
    );
  }

  // 3. Generate PKCE + state
  const codeVerifier = generateCodeVerifier();
  const codeChallenge = await generateCodeChallenge(codeVerifier);
  const state = generateState();

  // 4. Store pending flow for callback handler
  storePendingFlow(state, {
    serverUrl,
    codeVerifier,
    redirectUri,
    tokenEndpoint: asm.token_endpoint,
    clientId,
    clientSecret,
    resource,
  });

  // 5. Build authorization URL
  const authParams = new URLSearchParams({
    response_type: "code",
    client_id: clientId,
    redirect_uri: redirectUri,
    code_challenge: codeChallenge,
    code_challenge_method: "S256",
    state,
    resource,
  });

  // Determine scopes
  const scopes =
    oauthConfig?.scopes || prm.scopes_supported?.join(" ") || asm.scopes_supported?.join(" ");
  if (scopes) authParams.set("scope", scopes);

  const authUrl = `${asm.authorization_endpoint}?${authParams}`;

  // 6. Open popup and wait for callback
  return new Promise<OAuthTokens>((resolve, reject) => {
    let cleanedUp = false;

    const cleanup = () => {
      if (cleanedUp) return;
      cleanedUp = true;
      window.removeEventListener("message", onMessage);
      clearInterval(pollTimer);
      clearTimeout(timeoutTimer);
    };

    const onMessage = async (event: MessageEvent) => {
      if (event.origin !== window.location.origin) return;
      if (event.data?.type !== "mcp-oauth-callback") return;
      if (event.data.state !== state) return; // not our flow

      cleanup();

      const { code, error, errorDescription } = event.data;
      if (error) {
        clearPendingFlow(state);
        reject(new Error(errorDescription || error));
        return;
      }

      try {
        const tokens = await exchangeCodeForTokens(
          asm.token_endpoint,
          code,
          redirectUri,
          clientId!,
          codeVerifier,
          resource,
          clientSecret
        );
        storeOAuthData(serverUrl, {
          tokens,
          tokenEndpoint: asm.token_endpoint,
          clientId: clientId!,
          clientSecret,
          resource,
        });
        clearPendingFlow(state);
        resolve(tokens);
      } catch (err) {
        clearPendingFlow(state);
        reject(err);
      }
    };

    window.addEventListener("message", onMessage);

    const popup = window.open(authUrl, "mcp-oauth", "width=600,height=700,popup=yes");
    if (!popup) {
      cleanup();
      clearPendingFlow(state);
      reject(new Error("Failed to open popup. Please allow popups for this site."));
      return;
    }

    // Detect popup closed without completing
    const pollTimer = setInterval(() => {
      if (popup.closed) {
        cleanup();
        clearPendingFlow(state);
        reject(new Error("Authorization was cancelled"));
      }
    }, 500);

    // Timeout after 5 minutes
    const timeoutTimer = setTimeout(
      () => {
        cleanup();
        clearPendingFlow(state);
        try {
          popup.close();
        } catch {
          // ignore
        }
        reject(new Error("Authorization timed out"));
      },
      5 * 60 * 1000
    );
  });
}

// =============================================================================
// Token Access (used by MCPClient via getAccessToken callback)
// =============================================================================

/**
 * Get a valid access token for an MCP server.
 * Returns the current token if still valid, or refreshes if expired.
 * Returns null if no tokens are available (caller should initiate OAuth flow).
 */
export async function getValidAccessToken(
  serverUrl: string,
  oauthConfig?: MCPOAuthConfig
): Promise<string | null> {
  const data = getStoredOAuthData(serverUrl);
  if (!data) return null;

  if (!isTokenExpired(data.tokens)) {
    return data.tokens.access_token;
  }

  // Try to refresh
  if (data.tokens.refresh_token) {
    try {
      const clientId = oauthConfig?.clientId || data.clientId;
      const clientSecret = oauthConfig?.clientSecret || data.clientSecret;

      const newTokens = await refreshTokenRequest(
        data.tokenEndpoint,
        data.tokens.refresh_token,
        clientId,
        data.resource,
        clientSecret
      );

      // Preserve refresh token if the new response didn't include one (rotation)
      if (!newTokens.refresh_token) {
        newTokens.refresh_token = data.tokens.refresh_token;
      }

      storeOAuthData(serverUrl, { ...data, tokens: newTokens, clientId, clientSecret });
      return newTokens.access_token;
    } catch (err) {
      console.debug("MCP OAuth token refresh failed:", err);
      clearOAuthData(serverUrl);
      return null;
    }
  }

  // Token expired and no refresh token
  clearOAuthData(serverUrl);
  return null;
}

// =============================================================================
// Popup Callback Handler
// =============================================================================

/**
 * Handle an OAuth callback if the current page is a popup returning from authorization.
 * Call this early in the app's bootstrap (before rendering).
 *
 * Returns true if a callback was handled (the app should not render).
 */
export function handleMCPOAuthCallback(): boolean {
  const params = new URLSearchParams(window.location.search);
  const state = params.get("state");

  // Only handle callbacks with our "mcp-" prefixed state
  if (!state?.startsWith("mcp-")) return false;

  const code = params.get("code");
  const error = params.get("error");
  const errorDescription = params.get("error_description");

  if (!code && !error) return false;

  if (window.opener) {
    // We're in a popup — forward to opener and close
    try {
      window.opener.postMessage(
        { type: "mcp-oauth-callback", code, state, error, errorDescription },
        window.location.origin
      );
    } catch {
      // Opener may be closed or cross-origin
    }
    window.close();
    return true;
  }

  // No opener (edge case: user navigated here directly or popup mechanism failed).
  // Process the callback inline if we have the pending flow data.
  if (code) {
    const flow = getPendingFlow(state);
    if (flow) {
      // Exchange in the background and redirect to root
      exchangeCodeForTokens(
        flow.tokenEndpoint,
        code,
        flow.redirectUri,
        flow.clientId,
        flow.codeVerifier,
        flow.resource,
        flow.clientSecret
      )
        .then((tokens) => {
          storeOAuthData(flow.serverUrl, {
            tokens,
            tokenEndpoint: flow.tokenEndpoint,
            clientId: flow.clientId,
            clientSecret: flow.clientSecret,
            resource: flow.resource,
          });
          clearPendingFlow(state);
        })
        .catch((err) => {
          console.error("MCP OAuth inline callback failed:", err);
          clearPendingFlow(state);
        });
    }
    // Clean URL params
    const cleanUrl = new URL(window.location.href);
    cleanUrl.search = "";
    window.history.replaceState({}, "", cleanUrl.toString());
  }

  return false; // Let the app render normally
}

// =============================================================================
// Auth Detection
// =============================================================================

/** Result of probing a server URL for authentication requirements */
export interface AuthDetectionResult {
  authType: MCPAuthType;
  message: string;
  /** Server name discovered from resource_name in protected resource metadata */
  serverName?: string;
}

/**
 * Probe an MCP server URL to detect what authentication is required.
 *
 * 1. Check well-known OAuth protected-resource metadata (lightweight)
 * 2. Send an initialize request to the MCP endpoint
 *    - 200 → no auth needed
 *    - 401 + WWW-Authenticate with resource_metadata → OAuth
 *    - 401 otherwise → bearer token
 */
export async function detectServerAuth(serverUrl: string): Promise<AuthDetectionResult> {
  // 1. Well-known OAuth discovery (doesn't touch the MCP endpoint)
  try {
    const url = new URL(serverUrl);
    const paths =
      url.pathname !== "/" && url.pathname !== ""
        ? [
            `${url.origin}/.well-known/oauth-protected-resource${url.pathname}`,
            `${url.origin}/.well-known/oauth-protected-resource`,
          ]
        : [`${url.origin}/.well-known/oauth-protected-resource`];

    for (const path of paths) {
      try {
        const res = await fetch(path);
        if (res.ok) {
          const meta = (await res.json()) as ProtectedResourceMetadata;
          if (meta?.authorization_servers?.length) {
            return {
              authType: "oauth",
              message: "OAuth authentication detected",
              serverName: meta.resource_name,
            };
          }
        }
      } catch {
        continue;
      }
    }
  } catch {
    // Invalid URL — fall through
  }

  // 2. Probe the MCP endpoint with an initialize request
  try {
    const res = await fetch(serverUrl, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json,text/event-stream",
      },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 0,
        method: "initialize",
        params: {
          protocolVersion: "2024-11-05",
          capabilities: {},
          clientInfo: { name: "hadrian-detect", version: "1.0.0" },
        },
      }),
    });

    if (res.ok) {
      return { authType: "none", message: "No authentication required" };
    }

    if (res.status === 401) {
      const wwwAuth = res.headers.get("WWW-Authenticate") ?? "";
      const resourceMetaUrl = wwwAuth.match(/resource_metadata="([^"]+)"/)?.[1];

      if (resourceMetaUrl && isSameOrigin(resourceMetaUrl, serverUrl)) {
        // Verify the resource metadata actually has OAuth servers
        try {
          const metaRes = await fetch(resourceMetaUrl);
          if (metaRes.ok) {
            const meta = (await metaRes.json()) as ProtectedResourceMetadata;
            if (meta?.authorization_servers?.length) {
              return {
                authType: "oauth",
                message: "OAuth authentication required",
                serverName: meta.resource_name,
              };
            }
          }
        } catch {
          // Fall through to bearer
        }
      }

      return { authType: "bearer", message: "Bearer token required" };
    }
  } catch {
    // Network error — can't determine
  }

  return { authType: "none", message: "" };
}
