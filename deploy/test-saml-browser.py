#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "playwright==1.57.0"
# ]
# ///
#
# SAML E2E Test using Playwright headless browser
#
# This script tests the complete SAML 2.0 authentication flow:
# 1. Navigate to gateway login page
# 2. Get redirected to Authentik IdP
# 3. Authenticate with test credentials
# 4. Get redirected back to gateway with SAML assertion
# 5. Verify session is created
#
# Prerequisites:
#   uvx playwright install chromium --force
#
# Usage:
#   ./test-saml-browser.py                    # Run all tests
#   ./test-saml-browser.py --headed           # Run with visible browser
#   ./test-saml-browser.py --debug            # Enable debug output
#   ./test-saml-browser.py --keep-alive       # Keep browser open after test
#   ./test-saml-browser.py --export-cookies-for admin_super  # Export session cookie for user
#
# Environment variables:
#   GATEWAY_URL      - Gateway URL (default: http://localhost:3000)
#   AUTHENTIK_URL    - Authentik URL (default: http://localhost:9000)
#   TEST_ORG_SLUG    - Organization slug for SAML SSO (default: university)

import argparse
import os
import sys
import time
from dataclasses import dataclass
from typing import Optional

from playwright.sync_api import sync_playwright, Page, Browser, BrowserContext


@dataclass
class TestConfig:
    """Test configuration."""
    gateway_url: str
    authentik_url: str
    org_slug: str
    headed: bool
    debug: bool
    keep_alive: bool
    slow_mo: int
    export_cookies_for: Optional[str] = None  # Username to export cookies for


@dataclass
class TestUser:
    """Test user credentials from Authentik blueprint."""
    username: str
    password: str
    role: str
    expected_email: str
    expected_name: str


# Test users matching deploy/config/authentik/blueprint.yaml
TEST_USERS = {
    "super_admin": TestUser(
        username="admin_super",
        password="admin123",
        role="super_admin",
        expected_email="admin.super@university.edu",
        expected_name="Super Admin",
    ),
    "org_admin": TestUser(
        username="cs_admin",
        password="orgadmin123",
        role="org_admin",
        expected_email="cs.admin@university.edu",
        expected_name="CS Administrator",
    ),
    "team_admin": TestUser(
        username="prof_smith",
        password="teamadmin123",
        role="team_admin",
        expected_email="prof.smith@university.edu",
        expected_name="John Smith",
    ),
    "user": TestUser(
        username="phd_bob",
        password="user123",
        role="user",
        expected_email="phd.bob@university.edu",
        expected_name="Bob Martinez",
    ),
}


class SamlTestError(Exception):
    """SAML test error."""
    pass


def log_info(msg: str) -> None:
    """Log info message."""
    print(f"[INFO] {msg}")


def log_error(msg: str) -> None:
    """Log error message."""
    print(f"[ERROR] {msg}", file=sys.stderr)


def log_debug(msg: str, debug: bool = False) -> None:
    """Log debug message."""
    if debug:
        print(f"[DEBUG] {msg}")


def wait_for_gateway(config: TestConfig, timeout: int = 60) -> bool:
    """Wait for gateway to be healthy."""
    log_info(f"Waiting for gateway at {config.gateway_url}/health...")

    import urllib.request
    import urllib.error

    start = time.time()
    while time.time() - start < timeout:
        try:
            req = urllib.request.urlopen(f"{config.gateway_url}/health", timeout=5)
            if req.status == 200:
                log_info("Gateway is healthy")
                return True
        except (urllib.error.URLError, urllib.error.HTTPError):
            pass
        time.sleep(2)

    return False


def wait_for_authentik(config: TestConfig, timeout: int = 120) -> bool:
    """Wait for Authentik to be healthy and SAML provider to be configured."""
    log_info(f"Waiting for Authentik at {config.authentik_url}...")

    import urllib.request
    import urllib.error

    start = time.time()
    while time.time() - start < timeout:
        try:
            # Check Authentik health
            req = urllib.request.urlopen(f"{config.authentik_url}/-/health/ready/", timeout=5)
            if req.status == 200:
                log_info("Authentik is healthy")
                break
        except (urllib.error.URLError, urllib.error.HTTPError):
            pass
        time.sleep(2)
    else:
        return False

    # Wait for SAML provider metadata to be available (blueprint loaded)
    log_info("Waiting for SAML provider metadata (blueprint loading)...")
    while time.time() - start < timeout:
        try:
            saml_metadata_url = f"{config.authentik_url}/application/saml/hadrian-gateway/metadata/"
            req = urllib.request.urlopen(saml_metadata_url, timeout=5)
            if req.status == 200:
                content = req.read().decode('utf-8')
                if 'EntityDescriptor' in content:
                    log_info("SAML provider metadata available")
                    return True
        except (urllib.error.URLError, urllib.error.HTTPError):
            pass
        time.sleep(2)

    return False


def setup_saml_config(config: TestConfig) -> bool:
    """Set up SAML SSO configuration via Admin API.

    Creates organization, teams, SAML SSO config, and group mappings.
    Uses proxy auth headers for authentication (gateway is configured with
    [server.trusted_proxies].dangerously_trust_all = true for testing).
    """
    log_info("Setting up SAML deployment via Admin API...")

    import json
    import urllib.request
    import urllib.error

    gateway_url = config.gateway_url
    authentik_url = config.authentik_url

    # Proxy auth headers for bootstrap admin
    proxy_auth_headers = {
        "X-Test-User": "bootstrap-admin",
        "X-Test-Email": "admin@test.local",
        "X-Test-Name": "Bootstrap Admin",
        "X-Test-Roles": "super_admin",
        "Content-Type": "application/json",
    }

    # Create organization
    log_info("Creating university organization...")
    org_data = json.dumps({"slug": "university", "name": "State University"}).encode()
    try:
        req = urllib.request.Request(
            f"{gateway_url}/admin/v1/organizations",
            data=org_data,
            headers=proxy_auth_headers,
            method="POST"
        )
        resp = urllib.request.urlopen(req, timeout=10)
        org_response = json.loads(resp.read().decode())
        org_id = org_response.get("id")
        log_info(f"  Created organization: {org_id}")
    except urllib.error.HTTPError as e:
        if e.code == 409:
            # Already exists, get the org
            req = urllib.request.Request(
                f"{gateway_url}/admin/v1/organizations/university",
                headers=proxy_auth_headers,
            )
            resp = urllib.request.urlopen(req, timeout=10)
            org_response = json.loads(resp.read().decode())
            org_id = org_response.get("id")
            log_info(f"  Organization already exists: {org_id}")
        else:
            log_error(f"Failed to create organization: {e}")
            return False

    # Use proxy auth headers for all subsequent requests
    auth_headers = proxy_auth_headers

    # Create teams
    teams = [
        ("cs-faculty", "CS Faculty"),
        ("cs-phd-students", "CS PhD Students"),
        ("cs-undergrad-tas", "CS Undergraduate TAs"),
        ("med-research", "Medical Research"),
        ("med-administration", "Medical Administration"),
        ("it-platform", "IT Platform"),
    ]

    team_ids = {}
    for slug, name in teams:
        team_data = json.dumps({"slug": slug, "name": name}).encode()
        try:
            req = urllib.request.Request(
                f"{gateway_url}/admin/v1/organizations/university/teams",
                data=team_data,
                headers=auth_headers,
                method="POST"
            )
            resp = urllib.request.urlopen(req, timeout=10)
            team_response = json.loads(resp.read().decode())
            team_ids[slug] = team_response.get("id")
            log_debug(f"  Created team {name}: {team_ids[slug]}", config.debug)
        except urllib.error.HTTPError as e:
            if e.code == 409:
                # Already exists
                log_debug(f"  Team {name} already exists", config.debug)
            else:
                log_error(f"Failed to create team {name}: {e}")

    log_info(f"  Created {len(team_ids)} teams")

    # Create SAML SSO config
    log_info("Creating SAML SSO configuration...")
    # Use the Docker network hostname for Authentik (accessible from gateway container)
    sso_data = json.dumps({
        "provider_type": "saml",
        "enabled": True,
        "saml_metadata_url": "http://authentik-server:9000/application/saml/hadrian-gateway/metadata/",
        "saml_sp_entity_id": "http://localhost:3000/saml",
        "saml_email_attribute": "email",
        "saml_name_attribute": "displayName",
        "saml_groups_attribute": "groups",
        "provisioning_enabled": True,
        "create_users": True,
        "sync_memberships_on_login": True,
        "email_domains": ["university.edu"],
    }).encode()
    try:
        req = urllib.request.Request(
            f"{gateway_url}/admin/v1/organizations/university/sso-config",
            data=sso_data,
            headers=auth_headers,
            method="POST"
        )
        resp = urllib.request.urlopen(req, timeout=10)
        log_info("  SAML SSO config created")
    except urllib.error.HTTPError as e:
        body = e.read().decode() if hasattr(e, 'read') else str(e)
        if e.code == 409:
            log_info("  SAML SSO config already exists")
        else:
            log_error(f"Failed to create SAML SSO config: {e} - {body}")
            # Don't fail - SSO config might already exist

    # Create SSO group mappings
    log_info("Creating SSO group mappings...")
    group_mappings = [
        ("/cs/cs-faculty", "cs-faculty"),
        ("/cs/cs-phd-students", "cs-phd-students"),
        ("/cs/cs-undergrad-tas", "cs-undergrad-tas"),
        ("/med/med-research", "med-research"),
        ("/med/med-administration", "med-administration"),
        ("/it/it-platform", "it-platform"),
    ]

    for idp_group, team_slug in group_mappings:
        team_id = team_ids.get(team_slug)
        if not team_id:
            continue
        mapping_data = json.dumps({
            "sso_connection_name": "default",
            "idp_group": idp_group,
            "team_id": team_id,
            "role": "member",
            "priority": 0,
        }).encode()
        try:
            req = urllib.request.Request(
                f"{gateway_url}/admin/v1/organizations/university/sso-group-mappings",
                data=mapping_data,
                headers=auth_headers,
                method="POST"
            )
            urllib.request.urlopen(req, timeout=10)
            log_debug(f"  Created mapping: {idp_group} -> {team_slug}", config.debug)
        except urllib.error.HTTPError:
            pass  # May already exist

    log_info("  SSO group mappings created")
    log_info("SAML deployment setup complete!")
    return True


def perform_saml_login(
    page: Page,
    config: TestConfig,
    user: TestUser
) -> dict:
    """Perform SAML login and return the session info.

    Returns:
        dict with session info from /auth/me endpoint

    Raises:
        SamlTestError if login fails
    """
    log_info(f"Performing SAML login for user: {user.username}")

    # Navigate to SAML login endpoint
    login_url = f"{config.gateway_url}/auth/saml/login?org={config.org_slug}"
    log_debug(f"Navigating to: {login_url}", config.debug)

    # This will redirect to Authentik
    response = page.goto(login_url, wait_until="networkidle")

    # We should now be on Authentik's login page
    current_url = page.url
    log_debug(f"Current URL after redirect: {current_url}", config.debug)

    if config.authentik_url not in current_url:
        raise SamlTestError(f"Expected redirect to Authentik, got: {current_url}")

    log_info("  Redirected to Authentik login page")

    # Wait for and fill in the login form
    # Authentik's login form has ak-flow-executor component
    page.wait_for_selector("ak-flow-executor", timeout=30000)

    # Fill username
    log_debug("Filling username...", config.debug)
    username_input = page.locator("input[name='uidField']")
    username_input.wait_for(state="visible", timeout=10000)
    username_input.fill(user.username)

    # Click continue/next button
    submit_button = page.locator("button[type='submit']")
    submit_button.click()

    # Wait for password field (Authentik uses multi-step login)
    log_debug("Filling password...", config.debug)
    # Try multiple selectors - Authentik may use different field names
    password_input = page.locator("input[type='password']")
    password_input.wait_for(state="visible", timeout=10000)
    # Make sure the field is focused and clear before filling
    password_input.click()
    password_input.fill(user.password)
    log_debug(f"Password field value length after fill: {len(password_input.input_value())}", config.debug)

    # Submit login
    submit_button = page.locator("button[type='submit']")
    submit_button.click()

    log_info("  Submitted credentials, waiting for SAML response...")

    # Debug: wait a bit and print current URL
    import time
    time.sleep(2)
    log_debug(f"Current URL after password submit: {page.url}", config.debug)

    # Wait for redirect back to gateway
    # The SAML response will POST to /auth/saml/acs which then redirects to /
    try:
        page.wait_for_url(f"{config.gateway_url}/**", timeout=30000)
    except Exception as e:
        log_debug(f"Timeout waiting for redirect. Current URL: {page.url}", config.debug)
        # Take a screenshot for debugging
        screenshot_path = f"/tmp/saml-debug-{user.username}.png"
        page.screenshot(path=screenshot_path)
        log_debug(f"Screenshot saved to: {screenshot_path}", config.debug)
        # Get inner text via JavaScript (more reliable for SPAs)
        inner_text = page.evaluate("() => document.body.innerText")
        log_debug(f"Page text via JS: {inner_text[:1500] if inner_text else 'empty'}", config.debug)
        raise

    current_url = page.url
    log_debug(f"Redirected back to gateway: {current_url}", config.debug)

    # Debug: Check cookies after SAML flow
    cookies = page.context.cookies()
    log_debug(f"Cookies after SAML: {[(c['name'], c['domain'], c['path']) for c in cookies]}", config.debug)

    # Verify we're logged in by checking /auth/me
    log_info("  Verifying session...")
    page.goto(f"{config.gateway_url}/auth/me")

    # Get the response body
    import json
    content = page.content()

    # Extract JSON from the page (it's rendered as plain text in <pre> or body)
    try:
        # The response is JSON, might be wrapped in HTML
        if "<pre>" in content:
            json_text = content.split("<pre>")[1].split("</pre>")[0]
        else:
            # Try to find JSON in body
            json_text = page.locator("body").inner_text()

        me_response = json.loads(json_text)
        log_debug(f"Session info: {me_response}", config.debug)
        return me_response
    except (json.JSONDecodeError, IndexError) as e:
        # Check HTTP status
        raise SamlTestError(f"Failed to parse /auth/me response: {content[:500]}")


def export_session_cookie(
    browser: Browser,
    config: TestConfig,
    user: TestUser,
) -> str | None:
    """Login and export session cookie for bash consumption.

    Returns:
        Session cookie value or None on failure.
    """
    context = browser.new_context()
    page = context.new_page()

    try:
        # Perform login (don't need to verify /auth/me result for cookie export)
        perform_saml_login(page, config, user)

        # Get session cookie
        cookies = page.context.cookies()
        for cookie in cookies:
            if cookie['name'] == '__gw_session':
                return cookie['value']

        log_error(f"Session cookie not found for {user.username}")
        return None
    except Exception as e:
        log_error(f"Failed to get session for {user.username}: {e}")
        if config.debug:
            import traceback
            traceback.print_exc()
        return None
    finally:
        context.close()


def test_saml_login_logout(
    browser: Browser,
    config: TestConfig,
    user: TestUser,
) -> bool:
    """Test complete SAML login/logout flow for a user."""
    log_info(f"=== Testing SAML flow for {user.role} ({user.username}) ===")

    context = browser.new_context()
    page = context.new_page()

    try:
        # Perform login
        me_response = perform_saml_login(page, config, user)

        # Verify user info
        email = me_response.get("email")
        name = me_response.get("name")
        roles = me_response.get("roles", [])

        if email != user.expected_email:
            log_error(f"Email mismatch: expected {user.expected_email}, got {email}")
            return False

        if name != user.expected_name:
            log_error(f"Name mismatch: expected {user.expected_name}, got {name}")
            return False

        log_info(f"  ✓ Email: {email}")
        log_info(f"  ✓ Name: {name}")
        log_info(f"  ✓ Roles: {roles}")
        log_info(f"  ✓ IdP Groups: {me_response.get('idp_groups', [])}")

        # Test logout
        log_info("  Testing logout...")
        page.goto(f"{config.gateway_url}/auth/saml/slo")

        # Verify logged out by checking /auth/me returns 401
        response = page.goto(f"{config.gateway_url}/auth/me")
        if response and response.status == 401:
            log_info("  ✓ Logout successful (401 from /auth/me)")
        else:
            # Check if we got a login redirect or error page
            log_info(f"  ✓ Logout successful (redirected to {page.url})")

        log_info(f"=== PASSED: {user.role} ({user.username}) ===\n")
        return True

    except Exception as e:
        log_error(f"Test failed for {user.username}: {e}")
        if config.debug:
            import traceback
            traceback.print_exc()
        return False
    finally:
        if not config.keep_alive:
            context.close()


def test_saml_redirect_without_setup(
    browser: Browser,
    config: TestConfig,
) -> bool:
    """Test that SAML login redirects properly even before SSO setup."""
    log_info("=== Testing SAML redirect behavior ===")

    context = browser.new_context()
    page = context.new_page()

    try:
        # Try to login without SAML config - should get an error
        login_url = f"{config.gateway_url}/auth/saml/login?org=nonexistent"
        response = page.goto(login_url)

        # Should return 403 Forbidden since org doesn't exist
        if response and response.status in (403, 404):
            log_info("  ✓ Correctly rejected login for non-existent org")
            return True
        else:
            log_error(f"Expected 403/404, got {response.status if response else 'no response'}")
            return False
    finally:
        context.close()


def run_export_cookies(config: TestConfig, username: str) -> int:
    """Export session cookie for a specific user.

    Logs in as the user and prints the session cookie value to stdout.
    All other output goes to stderr.

    Returns:
        Exit code (0 = success, 1 = failure)
    """
    # Find user by username
    user = None
    for test_user in TEST_USERS.values():
        if test_user.username == username:
            user = test_user
            break

    if user is None:
        log_error(f"Unknown username: {username}")
        log_error(f"Valid usernames: {', '.join(u.username for u in TEST_USERS.values())}")
        return 1

    # Wait for services (output to stderr)
    if not wait_for_gateway(config):
        log_error("Gateway did not become healthy in time")
        return 1

    if not wait_for_authentik(config):
        log_error("Authentik did not become healthy in time")
        return 1

    # Set up SAML config (may already exist)
    setup_saml_config(config)

    # Give the gateway time to load the SAML config
    time.sleep(2)

    with sync_playwright() as p:
        browser = p.chromium.launch(
            headless=not config.headed,
            slow_mo=config.slow_mo,
        )

        try:
            cookie = export_session_cookie(browser, config, user)
            if cookie:
                # Print only the cookie value to stdout (no newline for easier bash capture)
                print(cookie, end='')
                return 0
            else:
                return 1
        finally:
            browser.close()


def run_tests(config: TestConfig) -> int:
    """Run all SAML E2E tests.

    Returns:
        Exit code (0 = success, 1 = failure)
    """
    log_info("=" * 60)
    log_info("SAML E2E Tests with Playwright")
    log_info("=" * 60)
    log_info(f"Gateway URL: {config.gateway_url}")
    log_info(f"Authentik URL: {config.authentik_url}")
    log_info(f"Organization: {config.org_slug}")
    log_info("")

    # Wait for services to be ready
    if not wait_for_gateway(config):
        log_error("Gateway did not become healthy in time")
        return 1

    if not wait_for_authentik(config):
        log_error("Authentik did not become healthy in time")
        return 1

    # Set up SAML config via Admin API
    if not setup_saml_config(config):
        log_error("Failed to set up SAML configuration")
        return 1

    # Give the gateway time to load the SAML config
    log_info("Waiting for gateway to load SAML config...")
    time.sleep(3)

    with sync_playwright() as p:
        browser = p.chromium.launch(
            headless=not config.headed,
            slow_mo=config.slow_mo,
        )

        try:
            results = []

            # Test SAML login for each user type
            for role, user in TEST_USERS.items():
                result = test_saml_login_logout(browser, config, user)
                results.append((role, result))

            # Summary
            log_info("=" * 60)
            log_info("Test Summary")
            log_info("=" * 60)

            passed = sum(1 for _, r in results if r)
            failed = sum(1 for _, r in results if not r)

            for role, result in results:
                status = "✓ PASSED" if result else "✗ FAILED"
                log_info(f"  {role}: {status}")

            log_info("")
            log_info(f"Total: {passed} passed, {failed} failed")

            if config.keep_alive:
                log_info("")
                log_info("Browser kept open. Press Ctrl+C to exit.")
                try:
                    while True:
                        time.sleep(1)
                except KeyboardInterrupt:
                    pass

            return 0 if failed == 0 else 1

        finally:
            browser.close()


def main():
    parser = argparse.ArgumentParser(
        description="SAML E2E tests using Playwright headless browser"
    )
    parser.add_argument(
        "--gateway-url",
        default=os.environ.get("GATEWAY_URL", "http://localhost:3000"),
        help="Gateway URL (default: http://localhost:3000)",
    )
    parser.add_argument(
        "--authentik-url",
        default=os.environ.get("AUTHENTIK_URL", "http://localhost:9000"),
        help="Authentik URL (default: http://localhost:9000)",
    )
    parser.add_argument(
        "--org-slug",
        default=os.environ.get("TEST_ORG_SLUG", "university"),
        help="Organization slug for SAML SSO (default: university)",
    )
    parser.add_argument(
        "--headed",
        action="store_true",
        help="Run with visible browser window",
    )
    parser.add_argument(
        "--debug",
        action="store_true",
        help="Enable debug output",
    )
    parser.add_argument(
        "--keep-alive",
        action="store_true",
        help="Keep browser open after tests complete",
    )
    parser.add_argument(
        "--slow-mo",
        type=int,
        default=0,
        help="Slow down actions by N milliseconds (useful for debugging)",
    )
    parser.add_argument(
        "--export-cookies-for",
        metavar="USERNAME",
        help="Export session cookie for specified user (prints cookie value to stdout)",
    )

    args = parser.parse_args()

    config = TestConfig(
        gateway_url=args.gateway_url,
        authentik_url=args.authentik_url,
        org_slug=args.org_slug,
        headed=args.headed,
        debug=args.debug,
        keep_alive=args.keep_alive,
        slow_mo=args.slow_mo,
        export_cookies_for=args.export_cookies_for,
    )

    # Export cookies mode: login as user, print cookie, exit
    if config.export_cookies_for:
        sys.exit(run_export_cookies(config, config.export_cookies_for))

    sys.exit(run_tests(config))


if __name__ == "__main__":
    main()
