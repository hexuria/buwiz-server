#!/usr/bin/env node
/**
 * End-to-end UI walkthrough: register, login, dashboard, click pages/buttons.
 *
 * Usage (Spin already on :3008):
 *   BASE_URL=http://localhost:3008 node scripts/e2e_walkthrough.mjs
 */
import { createRequire } from "node:module";
import { mkdirSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";

const require = createRequire("/workspace/package.json");
const { chromium } = require("playwright");

const baseUrl = (
  process.env.BASE_URL ||
  process.env.AUTH_PUBLIC_BASE_URL ||
  "http://localhost:3008"
).replace(/\/$/, "");

const artifactDir =
  process.env.E2E_ARTIFACT_DIR || "/opt/cursor/artifacts/e2e";
const email =
  process.env.E2E_EMAIL ||
  `e2e-${Date.now()}-${Math.floor(Math.random() * 1e6)}@example.test`;
const password = process.env.E2E_PASSWORD || "browser-correct-123";
const workspaceName = `E2E Workspace ${Date.now().toString().slice(-6)}`;

mkdirSync(artifactDir, { recursive: true });

const findings = [];
const apiErrors = [];

function addFinding(severity, area, detail, extra = {}) {
  findings.push({ severity, area, detail, ...extra });
  const tag = severity.toUpperCase();
  console.log(`[${tag}] ${area}: ${detail}`);
}

function url(path) {
  return new URL(path, baseUrl).toString();
}

function encodeBody(body) {
  return Buffer.from(body, "utf8").toString("base64");
}

async function fetchUi(path, body) {
  const encoded = encodeBody(body);
  return fetch(url(path), {
    method: "POST",
    headers: {
      "content-type": "application/x-www-form-urlencoded",
      "x-buwiz-request-body": encoded,
      "x-buwiz-request-body-enc": "b64",
      origin: baseUrl,
    },
  });
}

async function activateUser(targetEmail) {
  const escaped = targetEmail.replace(/'/g, "''");
  spawnSync(
    "psql",
    [
      "-h",
      "127.0.0.1",
      "-p",
      "54329",
      "-U",
      "wasi_auth",
      "-d",
      "wasi_auth",
      "-c",
      `UPDATE auth_users SET status='active' WHERE primary_email='${escaped}';`,
    ],
    {
      env: { ...process.env, PGPASSWORD: "wasi_auth_dev" },
      stdio: "ignore",
    },
  );
}

async function shot(page, name) {
  const file = `${artifactDir}/${name}.png`;
  await page.screenshot({ path: file, fullPage: true });
  return file;
}

async function pageState(page) {
  return page.evaluate(() => {
    const buttons = [...document.querySelectorAll("button, a[href], [role=button]")]
      .map((el) => ({
        tag: el.tagName.toLowerCase(),
        text: (el.innerText || el.getAttribute("aria-label") || "").trim().slice(0, 80),
        href: el.getAttribute("href") || "",
        disabled: Boolean(el.disabled),
      }))
      .filter((el) => el.text || el.href);
    return {
      title: document.title,
      h1: document.querySelector("h1")?.textContent?.trim() || "",
      h2: document.querySelector("h2")?.textContent?.trim() || "",
      path: location.pathname + location.search,
      body: (document.body?.innerText || "").slice(0, 4000),
      buttons,
    };
  });
}

function collectBodyProblems(body, path) {
  const lower = body.toLowerCase();
  const phrases = [
    "the server failed to complete this request",
    "missing delimiter",
    "auth storage is unavailable",
    "request origin rejected",
    "error deserializing server function",
    "servererror|",
    "internal error",
  ];
  for (const phrase of phrases) {
    if (lower.includes(phrase)) {
      addFinding("broken", path, `page text contains "${phrase}"`);
    }
  }
}

async function attachNetwork(page) {
  page.on("response", async (res) => {
    const u = res.url();
    if (!u.includes("/api/")) return;
    if (res.status() < 400) return;
    let body = "";
    try {
      body = (await res.text()).slice(0, 240);
    } catch {
      /* ignore */
    }
    apiErrors.push({
      url: u.replace(baseUrl, ""),
      status: res.status(),
      body,
    });
    if (res.status() >= 500) {
      addFinding("broken", u.replace(baseUrl, ""), `HTTP ${res.status()} ${body}`);
    }
  });
  page.on("pageerror", (error) => {
    addFinding("broken", "pageerror", String(error).slice(0, 300));
  });
}

async function visit(page, path, name) {
  await page.goto(url(path), { waitUntil: "domcontentloaded", timeout: 30_000 });
  await page.waitForLoadState("networkidle", { timeout: 8_000 }).catch(() => {});
  await page.waitForTimeout(500);
  const state = await pageState(page);
  await shot(page, name);
  collectBodyProblems(state.body, path);
  return state;
}

async function clickNamed(page, name, { timeout = 4_000 } = {}) {
  const locator = page.getByRole("button", { name }).first();
  if (!(await locator.count())) {
    const link = page.getByRole("link", { name }).first();
    if (await link.count()) {
      await link.click({ timeout });
      return true;
    }
    return false;
  }
  if (await locator.isDisabled().catch(() => false)) return false;
  await locator.click({ timeout });
  return true;
}

async function fillLabel(page, label, value) {
  const field = page.getByLabel(label, { exact: false }).first();
  if (await field.count()) {
    await field.fill(value);
    return true;
  }
  return false;
}

async function main() {
  console.log(`E2E base: ${baseUrl}`);
  console.log(`Account: ${email}`);

  const probe = await fetch(url("/"));
  if (!probe.ok) {
    throw new Error(`app not reachable: GET / -> ${probe.status}`);
  }

  const browser = await chromium.launch({
    channel: "chrome",
    headless: true,
    args: ["--no-sandbox", "--disable-dev-shm-usage"],
  });
  const context = await browser.newContext({
    baseURL: baseUrl,
    viewport: { width: 1280, height: 900 },
  });
  const page = await context.newPage();
  await attachNetwork(page);

  // --- Guest pages ---
  const guestPaths = [
    ["/", "guest-home"],
    ["/login", "guest-login"],
    ["/register", "guest-register"],
    ["/forgot-password", "guest-forgot"],
    ["/reset-password", "guest-reset"],
    ["/verify-email", "guest-verify"],
    ["/verify-email/pending", "guest-verify-pending"],
    ["/verify-email/resend", "guest-verify-resend"],
    ["/auth/required", "guest-auth-required"],
    ["/auth/forbidden", "guest-auth-forbidden"],
    ["/auth/session-expired", "guest-session-expired"],
    ["/auth/passkey-unsupported", "guest-passkey-unsupported"],
    ["/auth/callback/google", "guest-oauth-callback"],
    ["/u/nobody", "guest-public-profile"],
    ["/this-route-does-not-exist", "guest-404"],
    ["/dashboard", "guest-dashboard-redirect"],
    ["/tax-profiles", "guest-tax-redirect"],
  ];
  for (const [path, name] of guestPaths) {
    const state = await visit(page, path, name);
    console.log(`GUEST ${path} → ${state.path} h1="${state.h1}"`);
  }

  // Home CTAs
  await visit(page, "/", "home-before-cta");
  if (await clickNamed(page, "Create account")) {
    await page.waitForTimeout(400);
    await shot(page, "home-create-account");
  }
  await visit(page, "/", "home-before-signin");
  if (await clickNamed(page, "Sign in")) {
    await page.waitForTimeout(400);
    await shot(page, "home-sign-in");
  }

  // --- Register via UI ---
  await visit(page, "/register", "register-form");
  await fillLabel(page, "Email", email);
  await fillLabel(page, "Password", password);
  await clickNamed(page, "Create workspace");
  await page.waitForTimeout(2500);
  const afterRegister = await pageState(page);
  await shot(page, "register-after-submit");
  console.log(
    `REGISTER → ${afterRegister.path} h1="${afterRegister.h1}" body=${afterRegister.body.slice(0, 180)}`,
  );
  if (!/account created|check your inbox|welcome back|create your workspace/i.test(afterRegister.body)) {
    addFinding(
      "broken",
      "/register",
      `register submit did not show success: ${afterRegister.body.slice(0, 240)}`,
    );
  } else if (/account created/i.test(afterRegister.body)) {
    addFinding("ok", "/register", "account created banner shown");
  }

  await activateUser(email);

  // --- Login via UI ---
  await visit(page, "/login", "login-form");
  await fillLabel(page, "Email", email);
  if (await clickNamed(page, "Continue")) {
    await page.waitForTimeout(400);
  }
  await fillLabel(page, "Password", password);
  await clickNamed(page, "Sign in");
  await page.waitForTimeout(3000);
  const afterLogin = await pageState(page);
  await shot(page, "login-after-submit");
  console.log(`LOGIN → ${afterLogin.path} h1="${afterLogin.h1}"`);
  if (afterLogin.path.startsWith("/auth/required") || afterLogin.path === "/login") {
    addFinding("broken", "/login", `still unauthenticated after login: ${afterLogin.path}`);
  } else {
    addFinding("ok", "/login", `signed in, landed on ${afterLogin.path}`);
  }

  // Onboarding if needed
  if (afterLogin.path.includes("/onboarding") || afterLogin.h1.includes("Create your workspace")) {
    await fillLabel(page, "Workspace name", workspaceName);
    await page.waitForTimeout(200);
    if (await clickNamed(page, "Create workspace")) {
      await page.waitForTimeout(2500);
    }
    const afterOnboard = await pageState(page);
    await shot(page, "onboarding-after-create");
    console.log(`ONBOARD → ${afterOnboard.path} h1="${afterOnboard.h1}"`);
    if (afterOnboard.path.includes("/onboarding")) {
      addFinding(
        "broken",
        "/onboarding/workspace",
        `still on onboarding: ${afterOnboard.body.slice(0, 240)}`,
      );
    }
  }

  // Discover slug from URL or organizations page
  let slug = "workspace";
  const orgState = await visit(page, "/organizations", "orgs-list");
  const slugMatch = orgState.body.match(/\/org\/([a-z0-9-]+)/);
  if (slugMatch) slug = slugMatch[1];
  if (page.url().includes("/org/")) {
    slug = new URL(page.url()).pathname.split("/")[2] || slug;
  }

  const authedPaths = [
    ["/dashboard", "dash-overview"],
    ["/organizations", "dash-orgs"],
    ["/tax-profiles", "dash-tax"],
    ["/account/profile", "acct-profile"],
    ["/account/password", "acct-password"],
    ["/account/providers", "acct-providers"],
    ["/account/passkeys", "acct-passkeys"],
    ["/account/mfa", "acct-mfa"],
    ["/account/sessions", "acct-sessions"],
    ["/account/vault", "acct-vault"],
    [`/org/${slug}/vault`, "org-vault"],
    [`/org/${slug}/settings`, "org-settings"],
    [`/org/${slug}/settings/general`, "org-general"],
    [`/org/${slug}/settings/members`, "org-members"],
    [`/org/${slug}/settings/invitations`, "org-invitations"],
    [`/org/${slug}/settings/roles`, "org-roles"],
    [`/org/${slug}/settings/audit`, "org-audit"],
    [`/org/${slug}/settings/danger`, "org-danger"],
    ["/organizations/settings", "legacy-settings"],
    ["/organizations/members", "legacy-members"],
    ["/organizations/invitations", "legacy-invitations"],
    ["/organizations/roles", "legacy-roles"],
    ["/organizations/permissions", "legacy-permissions"],
    ["/organizations/audit", "legacy-audit"],
    ["/admin/users", "admin-users"],
    ["/admin/health", "admin-health"],
    ["/admin/policies", "admin-policies"],
    ["/admin/auth/signing-keys", "admin-keys"],
    ["/admin/auth/providers", "admin-providers"],
    ["/admin/auth/redirects", "admin-redirects"],
    ["/admin/authorization/policy", "admin-authz"],
    ["/onboarding/workspace", "onboarding-again"],
    ["/invitations/accept", "invite-accept"],
    ["/u/nobody", "public-profile-authed"],
  ];

  for (const [path, name] of authedPaths) {
    const state = await visit(page, path, name);
    console.log(`AUTH ${path} → ${state.path} h1="${state.h1}" h2="${state.h2}"`);
    if (state.path.startsWith("/auth/required")) {
      addFinding("broken", path, "redirected to /auth/required while logged in");
    }
  }

  // Click through primary nav
  for (const label of ["Overview", "Organizations", "Tax profiles"]) {
    if (await clickNamed(page, label)) {
      await page.waitForTimeout(800);
      const state = await pageState(page);
      await shot(page, `nav-${label.toLowerCase().replace(/\s+/g, "-")}`);
      collectBodyProblems(state.body, `nav:${label}`);
    }
  }

  // Dashboard buttons
  await visit(page, "/dashboard", "dash-buttons");
  for (const label of [
    "Load demos",
    "Load demo connectors",
    "Connect REST",
    "Connect Postgres",
    "Connect gRPC",
    "Catalog",
    "Resource",
    "Query",
  ]) {
    if (await clickNamed(page, label)) {
      await page.waitForTimeout(900);
      const state = await pageState(page);
      await shot(page, `dash-click-${label.toLowerCase().replace(/\s+/g, "-")}`);
      collectBodyProblems(state.body, `dashboard:${label}`);
    }
  }

  // Tax profiles create + claim
  await visit(page, "/tax-profiles", "tax-before-create");
  await fillLabel(page, "TIN (9 digits)", "123456789");
  await fillLabel(page, "Registered name", "E2E Taxpayer");
  await fillLabel(page, "RDO code", "038");
  if (await clickNamed(page, "Create tax profile")) {
    await page.waitForTimeout(2000);
    const state = await pageState(page);
    await shot(page, "tax-after-create");
    collectBodyProblems(state.body, "/tax-profiles:create");
    if (!/tax profile saved|held profiles|····/i.test(state.body)) {
      addFinding(
        "broken",
        "/tax-profiles",
        `create did not confirm success: ${state.body.slice(0, 280)}`,
      );
    }
  }
  await fillLabel(page, "TIN (9 digits)", "123456789");
  if (await clickNamed(page, "Claim as owner")) {
    await page.waitForTimeout(1500);
    await shot(page, "tax-after-claim");
  }
  if (await clickNamed(page, "Refresh")) {
    await page.waitForTimeout(800);
  }

  // Account profile save
  await visit(page, "/account/profile", "profile-edit");
  if (await fillLabel(page, "Display name", "E2E User")) {
    if (await clickNamed(page, "Save")) {
      await page.waitForTimeout(1500);
      await shot(page, "profile-after-save");
    }
  }

  // Settings general rename
  await visit(page, `/org/${slug}/settings/general`, "settings-rename");
  const nameInput = page.locator('input[type="text"]').first();
  if (await nameInput.count()) {
    const current = await nameInput.inputValue();
    await nameInput.fill(`${current} x`);
    if (await clickNamed(page, /save/i)) {
      await page.waitForTimeout(1500);
      await shot(page, "settings-after-rename");
    }
  }

  // Vault
  await visit(page, `/org/${slug}/vault`, "vault-page");
  for (const label of ["Load demo connectors", "Load demos", "Add secret"]) {
    if (await clickNamed(page, label)) {
      await page.waitForTimeout(1200);
      await shot(page, `vault-${label.toLowerCase().replace(/\s+/g, "-")}`);
    }
  }

  // Logout
  if (await clickNamed(page, "Sign out") || await clickNamed(page, "Log out")) {
    await page.waitForTimeout(1000);
    await shot(page, "after-logout");
  }

  await browser.close();

  const report = {
    baseUrl,
    email,
    slug,
    findings,
    apiErrors,
    broken: findings.filter((f) => f.severity === "broken"),
    ok: findings.filter((f) => f.severity === "ok"),
  };
  writeFileSync(`${artifactDir}/report.json`, JSON.stringify(report, null, 2));
  console.log("\n=== E2E SUMMARY ===");
  console.log(`ok: ${report.ok.length}  broken: ${report.broken.length}  api4xx+: ${apiErrors.length}`);
  for (const item of report.broken) {
    console.log(`BROKEN ${item.area}: ${item.detail}`);
  }
  process.exit(report.broken.length ? 1 : 0);
}

main().catch((error) => {
  console.error(error);
  writeFileSync(
    `${artifactDir}/report.json`,
    JSON.stringify({ fatal: String(error?.stack || error) }, null, 2),
  );
  process.exit(1);
});
