/**
 * Playwright-based SAML browser fixtures for E2E tests.
 *
 * Performs SAML 2.0 authentication flows through browser automation:
 * 1. Navigate to gateway's SAML login endpoint
 * 2. Get redirected to Authentik IdP
 * 3. Authenticate with test credentials
 * 4. Get redirected back to gateway with SAML assertion
 * 5. Extract session cookie for subsequent API calls
 *
 * Prerequisites:
 *   npx playwright install chromium
 *
 * Usage:
 *   const session = await getSamlSession(config, TEST_SAML_USERS.superAdmin);
 *   // Use session.cookie for authenticated requests
 *   await session.cleanup();
 */

import { chromium, type Browser, type BrowserContext, type Page } from "@playwright/test";
import type { SamlTestUser } from "./authentik";

export interface SamlBrowserConfig {
  /** Gateway base URL (e.g., http://localhost:3000) */
  gatewayUrl: string;
  /** Authentik base URL (e.g., http://localhost:9000) */
  authentikUrl: string;
  /** Organization slug for SAML SSO (default: university) */
  orgSlug?: string;
  /** Run browser in headed mode for debugging (default: false) */
  headed?: boolean;
  /** Slow down actions by N milliseconds (default: 0) */
  slowMo?: number;
  /** Enable debug logging (default: false) */
  debug?: boolean;
}

/**
 * Result of a successful SAML authentication.
 */
export interface SamlSession {
  /** Session cookie value (__gw_session) */
  cookie: string;
  /** User info from /auth/me endpoint */
  userInfo: SamlUserInfo;
  /** Cleanup function to close browser context */
  cleanup: () => Promise<void>;
}

/**
 * User info returned from /auth/me after SAML authentication.
 */
export interface SamlUserInfo {
  email: string;
  name: string;
  external_id?: string;
  roles?: string[];
  idp_groups?: string[];
  org_ids?: string[];
  team_ids?: string[];
}

const DEFAULT_ORG_SLUG = "university";
const SESSION_COOKIE_NAME = "__gw_session";

/**
 * Perform SAML login and get session cookie.
 *
 * This automates the complete SAML 2.0 SP-initiated flow:
 * 1. Navigate to /auth/saml/login?org={orgSlug}
 * 2. Follow redirect to Authentik IdP
 * 3. Fill username (step 1) and password (step 2)
 * 4. Follow redirect back to gateway ACS endpoint
 * 5. Extract session cookie
 *
 * @param config SAML browser configuration
 * @param user Test user credentials
 * @returns SAML session with cookie and user info
 *
 * @example
 * ```ts
 * import { getSamlSession, type SamlBrowserConfig } from "./saml-browser";
 * import { TEST_SAML_USERS } from "./authentik";
 *
 * const config: SamlBrowserConfig = {
 *   gatewayUrl: "http://localhost:3000",
 *   authentikUrl: "http://localhost:9000",
 * };
 *
 * const session = await getSamlSession(config, TEST_SAML_USERS.superAdmin);
 * try {
 *   // Make authenticated requests using session.cookie
 *   const response = await fetch(`${config.gatewayUrl}/admin/v1/organizations`, {
 *     headers: { Cookie: `${SESSION_COOKIE_NAME}=${session.cookie}` },
 *   });
 * } finally {
 *   await session.cleanup();
 * }
 * ```
 */
export async function getSamlSession(
  config: SamlBrowserConfig,
  user: SamlTestUser
): Promise<SamlSession> {
  const debug = config.debug ?? false;
  const orgSlug = config.orgSlug ?? DEFAULT_ORG_SLUG;

  const browser = await chromium.launch({
    headless: !config.headed,
    slowMo: config.slowMo ?? 0,
  });

  const context = await browser.newContext();
  const page = await context.newPage();

  try {
    // Perform the SAML login flow
    const userInfo = await performSamlLogin(page, config, user, debug);

    // Extract session cookie
    const cookies = await context.cookies();
    const sessionCookie = cookies.find((c) => c.name === SESSION_COOKIE_NAME);

    if (!sessionCookie) {
      throw new SamlAuthError(
        `Session cookie '${SESSION_COOKIE_NAME}' not found after SAML login. ` +
          `Available cookies: ${cookies.map((c) => c.name).join(", ")}`
      );
    }

    return {
      cookie: sessionCookie.value,
      userInfo,
      cleanup: async () => {
        await context.close();
        await browser.close();
      },
    };
  } catch (error) {
    // Cleanup on error
    await context.close();
    await browser.close();
    throw error;
  }
}

/**
 * Get session cookies for multiple users.
 * Useful for tests that need to verify role-based access control.
 *
 * @param config SAML browser configuration
 * @param users Map of role names to test users
 * @returns Map of role names to session info
 */
export async function getSamlSessionsForUsers(
  config: SamlBrowserConfig,
  users: Record<string, SamlTestUser>
): Promise<SamlSessionMap> {
  const sessions: Record<string, SamlSession> = {};
  const errors: string[] = [];

  for (const [role, user] of Object.entries(users)) {
    try {
      sessions[role] = await getSamlSession(config, user);
    } catch (error) {
      errors.push(`${role}: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  if (errors.length > 0) {
    // Cleanup any sessions that were created
    for (const session of Object.values(sessions)) {
      await session.cleanup();
    }
    throw new SamlAuthError(`Failed to get sessions for users:\n${errors.join("\n")}`);
  }

  return {
    sessions,
    cleanup: async () => {
      for (const session of Object.values(sessions)) {
        await session.cleanup();
      }
    },
  };
}

export interface SamlSessionMap {
  sessions: Record<string, SamlSession>;
  cleanup: () => Promise<void>;
}

/**
 * Get just the session cookie without keeping browser context open.
 * Use this for one-off authentication when you don't need cleanup management.
 *
 * @param config SAML browser configuration
 * @param user Test user credentials
 * @returns Session cookie value
 */
export async function getSamlSessionCookie(
  config: SamlBrowserConfig,
  user: SamlTestUser
): Promise<string> {
  const session = await getSamlSession(config, user);
  const cookie = session.cookie;
  await session.cleanup();
  return cookie;
}

/**
 * Perform the SAML login flow and return user info.
 * Internal function used by getSamlSession.
 */
async function performSamlLogin(
  page: Page,
  config: SamlBrowserConfig,
  user: SamlTestUser,
  debug: boolean
): Promise<SamlUserInfo> {
  const orgSlug = config.orgSlug ?? DEFAULT_ORG_SLUG;
  const loginUrl = `${config.gatewayUrl}/auth/saml/login?org=${orgSlug}`;

  debugLog(debug, `Navigating to SAML login: ${loginUrl}`);

  // Navigate to SAML login endpoint - this redirects to Authentik
  await page.goto(loginUrl, { waitUntil: "networkidle" });

  // Verify we're on Authentik's login page
  const currentUrl = page.url();
  debugLog(debug, `Current URL after redirect: ${currentUrl}`);

  if (!currentUrl.includes(new URL(config.authentikUrl).host)) {
    throw new SamlAuthError(
      `Expected redirect to Authentik at ${config.authentikUrl}, got: ${currentUrl}`
    );
  }

  // Wait for Authentik's flow executor component
  await page.waitForSelector("ak-flow-executor", { timeout: 30000 });
  debugLog(debug, "Authentik login page loaded");

  // Step 1: Enter username
  debugLog(debug, `Entering username: ${user.username}`);
  const usernameInput = page.locator("input[name='uidField']");
  await usernameInput.waitFor({ state: "visible", timeout: 10000 });
  await usernameInput.fill(user.username);

  // Click continue/submit
  const submitButton = page.locator("button[type='submit']");
  await submitButton.click();

  // Step 2: Enter password (Authentik uses multi-step login)
  debugLog(debug, "Entering password");
  const passwordInput = page.locator("input[type='password']");
  await passwordInput.waitFor({ state: "visible", timeout: 10000 });
  await passwordInput.click();
  await passwordInput.fill(user.password);

  // Submit login
  await submitButton.click();

  debugLog(debug, "Credentials submitted, waiting for redirect back to gateway");

  // Wait for redirect back to gateway
  try {
    await page.waitForURL(`${config.gatewayUrl}/**`, { timeout: 30000 });
  } catch {
    const errorUrl = page.url();
    debugLog(debug, `Timeout waiting for redirect. Current URL: ${errorUrl}`);

    // Take screenshot for debugging
    const screenshotPath = `/tmp/saml-debug-${user.username}-${Date.now()}.png`;
    try {
      await page.screenshot({ path: screenshotPath });
      debugLog(debug, `Screenshot saved to: ${screenshotPath}`);
    } catch {
      debugLog(debug, "Failed to save screenshot");
    }

    // Try to get error message from page
    const pageText = await page.evaluate(() => document.body.innerText);
    throw new SamlAuthError(
      `SAML login did not redirect back to gateway.\n` +
        `Current URL: ${errorUrl}\n` +
        `Screenshot: ${screenshotPath}\n` +
        `Page content: ${pageText.slice(0, 500)}`
    );
  }

  debugLog(debug, `Redirected to: ${page.url()}`);

  // Verify session by fetching /auth/me
  debugLog(debug, "Verifying session via /auth/me");
  await page.goto(`${config.gatewayUrl}/auth/me`);

  const content = await page.content();
  const userInfo = parseAuthMeResponse(content);

  debugLog(debug, `Session verified for user: ${userInfo.email}`);

  return userInfo;
}

/**
 * Parse the /auth/me response from the page content.
 */
function parseAuthMeResponse(content: string): SamlUserInfo {
  try {
    // The response is JSON, might be wrapped in HTML <pre> tags
    let jsonText: string;

    if (content.includes("<pre>")) {
      const match = content.match(/<pre[^>]*>([\s\S]*?)<\/pre>/);
      jsonText = match ? match[1] : content;
    } else if (content.includes("<body>")) {
      const match = content.match(/<body[^>]*>([\s\S]*?)<\/body>/);
      jsonText = match ? match[1] : content;
    } else {
      jsonText = content;
    }

    // Clean up HTML entities and whitespace
    jsonText = jsonText
      .replace(/&quot;/g, '"')
      .replace(/&amp;/g, "&")
      .replace(/&lt;/g, "<")
      .replace(/&gt;/g, ">")
      .trim();

    return JSON.parse(jsonText) as SamlUserInfo;
  } catch (error) {
    throw new SamlAuthError(
      `Failed to parse /auth/me response: ${error instanceof Error ? error.message : String(error)}\n` +
        `Content: ${content.slice(0, 500)}`
    );
  }
}

/**
 * Error thrown when SAML authentication fails.
 */
export class SamlAuthError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "SamlAuthError";
  }
}

/**
 * Log debug messages when debug mode is enabled.
 */
function debugLog(debug: boolean, message: string): void {
  if (debug) {
    console.log(`[SAML] ${message}`);
  }
}

/**
 * Test the SAML login flow for a user and verify expected attributes.
 * Useful for E2E tests that need to verify SAML assertion attributes.
 *
 * @param config SAML browser configuration
 * @param user Test user with expected attributes
 * @returns Validation result with any mismatches
 */
export async function verifySamlLogin(
  config: SamlBrowserConfig,
  user: SamlTestUser
): Promise<SamlVerificationResult> {
  const session = await getSamlSession(config, user);
  const mismatches: string[] = [];

  try {
    // Verify email
    if (session.userInfo.email !== user.expectedEmail) {
      mismatches.push(
        `Email mismatch: expected '${user.expectedEmail}', got '${session.userInfo.email}'`
      );
    }

    // Verify name
    if (session.userInfo.name !== user.expectedName) {
      mismatches.push(
        `Name mismatch: expected '${user.expectedName}', got '${session.userInfo.name}'`
      );
    }

    return {
      success: mismatches.length === 0,
      userInfo: session.userInfo,
      mismatches,
      cookie: session.cookie,
    };
  } finally {
    await session.cleanup();
  }
}

export interface SamlVerificationResult {
  success: boolean;
  userInfo: SamlUserInfo;
  mismatches: string[];
  cookie: string;
}

/**
 * Create a fetch wrapper that includes the SAML session cookie.
 * Useful for making authenticated API requests after SAML login.
 *
 * @param session SAML session
 * @param gatewayUrl Gateway base URL
 * @returns Fetch function with session cookie included
 */
export function createAuthenticatedFetch(
  session: SamlSession,
  gatewayUrl: string
): (path: string, init?: RequestInit) => Promise<Response> {
  return async (path: string, init?: RequestInit): Promise<Response> => {
    const url = path.startsWith("http") ? path : `${gatewayUrl}${path}`;
    const headers = new Headers(init?.headers);
    headers.set("Cookie", `${SESSION_COOKIE_NAME}=${session.cookie}`);

    return fetch(url, {
      ...init,
      headers,
    });
  };
}

/**
 * Test the complete SAML login and logout flow.
 * Verifies both login succeeds and SLO (Single Logout) works correctly.
 *
 * @param config SAML browser configuration
 * @param user Test user credentials
 * @returns Test result with login and logout status
 */
export async function testSamlLoginLogout(
  config: SamlBrowserConfig,
  user: SamlTestUser
): Promise<SamlLoginLogoutResult> {
  const debug = config.debug ?? false;
  const orgSlug = config.orgSlug ?? DEFAULT_ORG_SLUG;

  const browser = await chromium.launch({
    headless: !config.headed,
    slowMo: config.slowMo ?? 0,
  });

  const context = await browser.newContext();
  const page = await context.newPage();

  const result: SamlLoginLogoutResult = {
    loginSuccess: false,
    logoutSuccess: false,
    userInfo: null,
    errors: [],
  };

  try {
    // Perform login
    const userInfo = await performSamlLogin(page, config, user, debug);
    result.loginSuccess = true;
    result.userInfo = userInfo;

    // Verify user attributes
    if (userInfo.email !== user.expectedEmail) {
      result.errors.push(`Email mismatch: expected '${user.expectedEmail}', got '${userInfo.email}'`);
    }
    if (userInfo.name !== user.expectedName) {
      result.errors.push(`Name mismatch: expected '${user.expectedName}', got '${userInfo.name}'`);
    }

    // Test logout via SAML SLO
    debugLog(debug, "Testing SAML Single Logout...");
    await page.goto(`${config.gatewayUrl}/auth/saml/slo`);

    // Verify logged out by checking /auth/me returns 401
    const meResponse = await page.goto(`${config.gatewayUrl}/auth/me`);
    if (meResponse && meResponse.status() === 401) {
      result.logoutSuccess = true;
      debugLog(debug, "Logout successful (401 from /auth/me)");
    } else {
      // Check if redirected to login page (also valid logout behavior)
      const currentUrl = page.url();
      if (currentUrl.includes("/auth/") || currentUrl.includes("/login")) {
        result.logoutSuccess = true;
        debugLog(debug, `Logout successful (redirected to ${currentUrl})`);
      } else {
        result.errors.push(`Logout verification failed: expected 401 or redirect, got ${meResponse?.status()}`);
      }
    }
  } catch (error) {
    result.errors.push(error instanceof Error ? error.message : String(error));
  } finally {
    await context.close();
    await browser.close();
  }

  return result;
}

export interface SamlLoginLogoutResult {
  loginSuccess: boolean;
  logoutSuccess: boolean;
  userInfo: SamlUserInfo | null;
  errors: string[];
}

/**
 * Test that SAML login correctly rejects non-existent organizations.
 * Should return 403 or 404 for unknown org slugs.
 *
 * @param config SAML browser configuration
 * @param orgSlug Organization slug to test (should not exist)
 * @returns True if correctly rejected, false otherwise
 */
export async function testSamlRejectUnknownOrg(
  config: SamlBrowserConfig,
  orgSlug: string = "nonexistent"
): Promise<{ rejected: boolean; statusCode: number | null; error?: string }> {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext();
  const page = await context.newPage();

  try {
    const loginUrl = `${config.gatewayUrl}/auth/saml/login?org=${orgSlug}`;
    const response = await page.goto(loginUrl);

    const statusCode = response?.status() ?? null;
    const rejected = statusCode === 403 || statusCode === 404;

    return { rejected, statusCode };
  } catch (error) {
    return {
      rejected: false,
      statusCode: null,
      error: error instanceof Error ? error.message : String(error),
    };
  } finally {
    await context.close();
    await browser.close();
  }
}
