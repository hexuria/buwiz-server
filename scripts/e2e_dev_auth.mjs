#!/usr/bin/env node
/**
 * Browser E2E for cloud/dev authentication.
 * Proves register → captured verify → login → dashboard → logout → login.
 *
 * Usage:
 *   BASE_URL=http://localhost:3008 node scripts/e2e_dev_auth.mjs
 */
import { createRequire } from "node:module";
import { mkdirSync } from "node:fs";
import { join } from "node:path";

const require = createRequire("/workspace/package.json");
const { chromium } = require("playwright");

const baseUrl = (
  process.env.BASE_URL ||
  process.env.AUTH_PUBLIC_BASE_URL ||
  "http://localhost:3008"
).replace(/\/$/, "");
const artifactDir =
  process.env.E2E_ARTIFACT_DIR || "/opt/cursor/artifacts/dev-auth";
mkdirSync(artifactDir, { recursive: true });

const email = `dev-auth-${Date.now()}@example.test`;
const password = "Dev-auth-password-15";

function url(path) {
  return new URL(path, `${baseUrl}/`).toString();
}

async function shot(page, name) {
  const file = join(artifactDir, `${name}.png`);
  await page.screenshot({ path: file, fullPage: true });
  console.log(`screenshot ${file}`);
  return file;
}

async function launch() {
  return chromium.launch({
    channel: "chrome",
    args: ["--no-sandbox", "--disable-dev-shm-usage"],
  });
}

async function waitHeading(page, text, timeout = 20000) {
  await page.waitForFunction(
    (expected) => {
      const nodes = [...document.querySelectorAll("h1,h2,[data-testid=home-heading]")];
      return nodes.some((n) => (n.textContent || "").includes(expected));
    },
    text,
    { timeout },
  );
}

async function main() {
  const browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  page.setDefaultTimeout(30000);
  const shots = [];

  try {
    await page.goto(url("/"), { waitUntil: "domcontentloaded" });
    await waitHeading(page, "One verified session");
    shots.push(await shot(page, "01_home"));

    await page.goto(url("/register"), { waitUntil: "domcontentloaded" });
    await page.waitForSelector('[data-testid="auth-credentials-form"]');
    await waitHeading(page, "Create your workspace");
    shots.push(await shot(page, "02_register_form"));

    await page.locator('input[name="email"]').fill(email);
    await page.locator('input[name="password"]').fill(password);
    await page.locator('form[data-testid="auth-credentials-form"] button[type="submit"]').click();
    await page.waitForSelector('[data-testid="register-success"], [data-testid="dev-verify-panel"]', {
      timeout: 30000,
    });
    await page.waitForSelector('[data-testid="dev-verify-now"]', { timeout: 15000 });
    shots.push(await shot(page, "03_register_success_capture"));

    let verified = false;
    for (let attempt = 0; attempt < 12 && !verified; attempt += 1) {
      await page.locator('[data-testid="dev-verify-now"]').click();
      try {
        await page.waitForURL(
          (u) =>
            u.pathname.includes("/verify-email") ||
            u.pathname.includes("/onboarding") ||
            u.pathname.includes("/dashboard"),
          { timeout: 4000 },
        );
        verified = true;
      } catch {
        await page.waitForTimeout(400);
      }
    }
    if (!verified) {
      throw new Error("Verify now did not leave the register success panel");
    }
    await page.waitForTimeout(800);
    await waitHeading(page, "Verify your email").catch(async () => {
      await waitHeading(page, "Create your workspace").catch(async () => {
        await waitHeading(page, "Overview").catch(() => {});
      });
    });
    shots.push(await shot(page, "04_verify_dev_path"));

    await page.waitForURL(
      (u) => u.pathname.includes("/onboarding") || u.pathname.includes("/dashboard"),
      { timeout: 20000 },
    );
    shots.push(await shot(page, "05_after_verify"));

    if (page.url().includes("/onboarding")) {
      await page.locator('input[placeholder="Acme Inc"]').fill("Dev Auth Workspace");
      await page.getByRole("button", { name: "Create workspace" }).click();
      await page.waitForURL((u) => u.pathname.includes("/dashboard"), { timeout: 20000 });
    }

    await page.waitForSelector('[data-testid="workspace-shell"]', { timeout: 20000 });
    shots.push(await shot(page, "06_dashboard_chrome"));

    await page.locator('[data-testid="logout-button"]').first().click();
    await page.waitForURL((u) => u.pathname === "/" || u.pathname.includes("/login"), {
      timeout: 15000,
    });
    await page.goto(url("/login"), { waitUntil: "domcontentloaded" });
    await page.waitForSelector('[data-testid="auth-credentials-form"]');
    // Email step then password step.
    const emailBox = page.locator('input[name="email"]');
    await emailBox.fill(email);
    const continueBtn = page.getByRole("button", { name: /Continue|Sign in/ });
    await continueBtn.first().click();
    await page.locator('input[name="password"]').waitFor({ state: "visible" });
    await page.locator('input[name="password"]').fill(password);
    await page.locator('form[data-testid="auth-credentials-form"] button[type="submit"]').click();
    await page.waitForURL(
      (u) => u.pathname.includes("/onboarding") || u.pathname.includes("/dashboard"),
      { timeout: 20000 },
    );
    await page.waitForSelector('[data-testid="workspace-shell"]', { timeout: 20000 });
    shots.push(await shot(page, "07_login_again"));

    const device = await fetch(url("/auth/device/start"), {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ client_id: "buwiz-desktop" }),
    });
    const deviceBody = await device.json();
    if (!device.ok || !deviceBody.device_code || !deviceBody.user_code) {
      throw new Error(`device/start failed ${device.status}: ${JSON.stringify(deviceBody)}`);
    }
    console.log(
      JSON.stringify({
        ok: true,
        email,
        shots,
        device_start: {
          status: device.status,
          has_device_code: Boolean(deviceBody.device_code),
          has_user_code: Boolean(deviceBody.user_code),
          verification_uri: deviceBody.verification_uri,
        },
      }),
    );
  } catch (error) {
    await shot(page, "99_failure").catch(() => {});
    console.error(error);
    process.exitCode = 1;
  } finally {
    await browser.close();
  }
}

await main();
