/**
 * JWT decoding utilities for E2E tests.
 *
 * These utilities decode JWT payloads for claim verification in tests.
 * Note: This does NOT verify signatures - use only for testing purposes.
 */

/**
 * Standard JWT claims that may appear in tokens.
 */
export interface JwtClaims {
  /** Issuer - identifies the principal that issued the JWT */
  iss?: string;
  /** Subject - identifies the principal that is the subject of the JWT */
  sub?: string;
  /** Audience - identifies the recipients for which the JWT is intended */
  aud?: string | string[];
  /** Expiration Time - time after which the JWT must not be accepted */
  exp?: number;
  /** Not Before - time before which the JWT must not be accepted */
  nbf?: number;
  /** Issued At - time at which the JWT was issued */
  iat?: number;
  /** JWT ID - unique identifier for the JWT */
  jti?: string;

  // Common OIDC claims
  /** Preferred username (from OIDC) */
  preferred_username?: string;
  /** Email address */
  email?: string;
  /** Email verified flag */
  email_verified?: boolean;
  /** Full name */
  name?: string;
  /** Given name */
  given_name?: string;
  /** Family name */
  family_name?: string;

  // Keycloak-specific claims (from realm-export.json mappers)
  /** Realm roles (configured via oidc-usermodel-realm-role-mapper) */
  roles?: string[];
  /** Group memberships (configured via oidc-group-membership-mapper) */
  groups?: string[];

  /** Allow additional custom claims */
  [key: string]: unknown;
}

/**
 * Decode a JWT token and extract its payload.
 * Note: This does NOT verify the signature - use only for testing purposes.
 *
 * @param token The JWT token string
 * @returns Decoded payload as a typed object
 * @throws Error if token format is invalid
 *
 * @example
 * ```ts
 * const payload = decodeJwtPayload(tokens.access_token);
 * console.log(payload.preferred_username); // "admin_super"
 * console.log(payload.roles); // ["super_admin", "user"]
 * ```
 */
export function decodeJwtPayload(token: string): JwtClaims {
  const parts = token.split(".");
  if (parts.length !== 3) {
    throw new Error(
      `Invalid JWT format: expected 3 parts (header.payload.signature), got ${parts.length}`
    );
  }

  const payload = parts[1];
  const decoded = base64UrlDecode(payload);

  try {
    return JSON.parse(decoded);
  } catch {
    throw new Error(`Failed to parse JWT payload as JSON: ${decoded}`);
  }
}

/**
 * Check if a JWT payload contains a specific claim.
 *
 * @param payload JWT payload (decoded)
 * @param claim Claim name to check
 * @returns true if the claim exists (even if value is falsy)
 *
 * @example
 * ```ts
 * if (jwtHasClaim(payload, "roles")) {
 *   console.log("Token has roles claim");
 * }
 * ```
 */
export function jwtHasClaim(payload: JwtClaims, claim: string): boolean {
  return Object.prototype.hasOwnProperty.call(payload, claim);
}

/**
 * Get a claim value from a JWT payload.
 *
 * @param payload JWT payload (decoded)
 * @param claim Claim name to get
 * @returns Claim value or undefined if not present
 *
 * @example
 * ```ts
 * const username = jwtGetClaim(payload, "preferred_username");
 * const roles = jwtGetClaim(payload, "roles") as string[];
 * ```
 */
export function jwtGetClaim<T = unknown>(
  payload: JwtClaims,
  claim: string
): T | undefined {
  return payload[claim] as T | undefined;
}

/**
 * Check if a JWT payload contains a specific role.
 *
 * @param payload JWT payload (decoded)
 * @param role Role name to check for
 * @returns true if the roles claim contains the specified role
 *
 * @example
 * ```ts
 * if (jwtHasRole(payload, "super_admin")) {
 *   console.log("User is a super admin");
 * }
 * ```
 */
export function jwtHasRole(payload: JwtClaims, role: string): boolean {
  const roles = payload.roles;
  if (!Array.isArray(roles)) {
    return false;
  }
  return roles.includes(role);
}

/**
 * Check if a JWT payload contains a specific group membership.
 *
 * @param payload JWT payload (decoded)
 * @param group Group path to check for (e.g., "/cs/faculty")
 * @returns true if the groups claim contains the specified group
 *
 * @example
 * ```ts
 * if (jwtHasGroup(payload, "/cs/faculty")) {
 *   console.log("User is in CS faculty group");
 * }
 * ```
 */
export function jwtHasGroup(payload: JwtClaims, group: string): boolean {
  const groups = payload.groups;
  if (!Array.isArray(groups)) {
    return false;
  }
  return groups.includes(group);
}

/**
 * Check if a JWT token is expired.
 *
 * @param payload JWT payload (decoded)
 * @param clockSkewSeconds Allowed clock skew in seconds (default: 60)
 * @returns true if the token is expired
 *
 * @example
 * ```ts
 * if (isTokenExpired(payload)) {
 *   // Refresh the token
 * }
 * ```
 */
export function isTokenExpired(
  payload: JwtClaims,
  clockSkewSeconds = 60
): boolean {
  const exp = payload.exp;
  if (typeof exp !== "number") {
    return false; // No expiration claim, assume not expired
  }
  const now = Math.floor(Date.now() / 1000);
  return now > exp + clockSkewSeconds;
}

/**
 * Decode a base64url-encoded string.
 * Base64url replaces + with - and / with _, and may omit padding.
 */
function base64UrlDecode(input: string): string {
  // Replace base64url-specific characters with standard base64
  let base64 = input.replace(/-/g, "+").replace(/_/g, "/");

  // Add padding if needed
  const padding = base64.length % 4;
  if (padding > 0) {
    base64 += "=".repeat(4 - padding);
  }

  // Decode from base64
  // In Node.js, we can use Buffer
  const buffer = Buffer.from(base64, "base64");
  return buffer.toString("utf-8");
}
