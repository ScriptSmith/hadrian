/**
 * OpenRouter OAuth PKCE flow.
 *
 * @see https://openrouter.ai/docs/guides/overview/auth/oauth
 */

const VERIFIER_KEY = "hadrian-openrouter-verifier";

/** Generate a cryptographically random code verifier. */
function generateCodeVerifier(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(32));
  return base64url(bytes);
}

/** SHA-256 hash the verifier and base64url-encode it. */
async function generateCodeChallenge(verifier: string): Promise<string> {
  const encoded = new TextEncoder().encode(verifier);
  const digest = await crypto.subtle.digest("SHA-256", encoded);
  return base64url(new Uint8Array(digest));
}

function base64url(bytes: Uint8Array): string {
  let binary = "";
  for (const b of bytes) binary += String.fromCharCode(b);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/** True when running inside an iframe (e.g. embedded in docs). */
export function isInIframe(): boolean {
  try {
    return window.self !== window.top;
  } catch {
    return true; // cross-origin iframe throws, so we're definitely framed
  }
}

/**
 * Start the OpenRouter OAuth PKCE flow.
 * Stores the code verifier in sessionStorage and redirects to OpenRouter.
 *
 * When running inside an iframe, opens the app in a new tab instead —
 * OpenRouter refuses to load in iframes.
 */
export async function startOpenRouterOAuth() {
  if (isInIframe()) {
    window.open(window.location.origin + window.location.pathname, "_blank", "noopener,noreferrer");
    return;
  }

  const verifier = generateCodeVerifier();
  const challenge = await generateCodeChallenge(verifier);
  sessionStorage.setItem(VERIFIER_KEY, verifier);

  const callbackUrl = window.location.origin + window.location.pathname;
  const params = new URLSearchParams({
    callback_url: callbackUrl,
    code_challenge: challenge,
    code_challenge_method: "S256",
  });

  window.location.href = `https://openrouter.ai/auth?${params}`;
}

/**
 * Check if we're returning from an OpenRouter OAuth callback.
 * Returns the authorization code if present, otherwise null.
 */
export function getOpenRouterCallbackCode(): string | null {
  const params = new URLSearchParams(window.location.search);
  return params.get("code");
}

/** Remove the code param from the URL without a page reload. */
export function clearCallbackCode() {
  const url = new URL(window.location.href);
  url.searchParams.delete("code");
  window.history.replaceState({}, "", url.toString());
}

/**
 * Exchange the authorization code for an OpenRouter API key.
 * Uses the stored code verifier from sessionStorage.
 */
export async function exchangeCodeForKey(code: string): Promise<string> {
  const verifier = sessionStorage.getItem(VERIFIER_KEY);
  if (!verifier) {
    throw new Error("Missing code verifier — OAuth flow may have been interrupted");
  }

  const res = await fetch("https://openrouter.ai/api/v1/auth/keys", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      code,
      code_verifier: verifier,
      code_challenge_method: "S256",
    }),
  });

  if (!res.ok) {
    const body = await res.text();
    throw new Error(`OpenRouter key exchange failed (${res.status}): ${body}`);
  }

  const { key } = await res.json();
  sessionStorage.removeItem(VERIFIER_KEY);
  return key;
}
