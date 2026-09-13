#!/usr/bin/env node
/**
 * Browser E2E for yearly tax profiles (TIN → branch → year → forms).
 *
 * Usage:
 *   BASE_URL=http://localhost:3008 node scripts/e2e_tax_profile_yearly.mjs
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
  process.env.E2E_ARTIFACT_DIR || "/opt/cursor/artifacts/tax-profile-yearly";
mkdirSync(artifactDir, { recursive: true });

const email = `tax-year-${Date.now()}@example.test`;
const password = "Dev-auth-password-15";
const tinRoot = "123456789";

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

async function assertVisible(page, selector, label) {
  const loc = page.locator(selector).first();
  await loc.waitFor({ state: "visible", timeout: 20000 });
  if (!(await loc.isVisible())) {
    throw new Error(`${label} not visible (${selector})`);
  }
}

async function registerAndEnterWorkspace(page, shots) {
  await page.goto(url("/register"), { waitUntil: "domcontentloaded" });
  await assertVisible(
    page,
    '[data-testid="auth-credentials-form"]',
    "register form",
  );
  await page.locator('input[name="email"]').fill(email);
  await page.locator('input[name="password"]').fill(password);
  await page
    .locator('form[data-testid="auth-credentials-form"] button[type="submit"]')
    .click();
  await page.waitForSelector(
    '[data-testid="register-success"], [data-testid="dev-verify-panel"]',
    { timeout: 30000 },
  );
  await assertVisible(
    page,
    '[data-testid="dev-verify-now"]',
    "verify-now control",
  );

  let leftRegister = false;
  for (let attempt = 0; attempt < 12 && !leftRegister; attempt += 1) {
    await page.locator('[data-testid="dev-verify-now"]').click();
    try {
      await page.waitForURL(
        (u) =>
          u.pathname.includes("/verify-email") ||
          u.pathname.includes("/onboarding") ||
          u.pathname.includes("/dashboard"),
        { timeout: 4000 },
      );
      leftRegister = true;
    } catch {
      await page.waitForTimeout(500);
    }
  }
  if (!leftRegister) {
    throw new Error("Verify now did not leave the register success panel");
  }

  await page.waitForURL(
    (u) =>
      u.pathname.includes("/onboarding") || u.pathname.includes("/dashboard"),
    { timeout: 25000 },
  );

  if (page.url().includes("/onboarding")) {
    await assertVisible(
      page,
      '[data-testid="onboarding-workspace"], input[placeholder="Acme Inc"]',
      "onboarding form",
    );
    await page
      .locator('input[placeholder="Acme Inc"]')
      .fill(`Tax Year ${Date.now()}`);
    await page.getByRole("button", { name: "Create workspace" }).click();
    await page.waitForURL((u) => u.pathname.includes("/dashboard"), {
      timeout: 20000,
    });
  }

  await assertVisible(page, '[data-testid="workspace-shell"]', "workspace chrome");
  shots.push(await shot(page, "01_workspace"));
}

async function createBranch(page, branch, name, rdo) {
  await page.locator('[data-testid="tax-profile-add-branch"]').click();
  await assertVisible(
    page,
    '[data-testid="tax-profile-composer"]',
    "branch composer",
  );
  await page.locator('input[name="tin_root"]').fill(tinRoot);
  await page.locator('input[name="branch_code"]').fill(branch);
  await page.locator('input[name="registered_name"]').fill(name);
  await page.locator('input[name="rdo_code"]').fill(rdo);
  await page
    .locator('[data-testid="tax-profile-composer"] button[type="submit"]')
    .click();
  await page
    .locator(`[data-testid="tax-profile-branch-${branch}"]`)
    .waitFor({ state: "visible", timeout: 20000 });
}

async function saveYearForms(page, codes) {
  await page.locator('[data-testid="tax-profile-tab-forms"]').click();
  await assertVisible(page, '[data-testid="tax-profile-forms"]', "forms tab");
  await page.locator('[data-testid="tax-profile-forms-search"]').fill("");
  for (const code of codes) {
    await page.locator('[data-testid="tax-profile-forms-search"]').fill(code);
    const row = page.locator(`[data-testid="tax-profile-form-${code}"]`);
    await row.waitFor({ state: "visible", timeout: 10000 });
    const box = row.locator('input[type="checkbox"]');
    if (!(await box.isChecked())) {
      await box.check();
    }
  }
  await page.locator('[data-testid="tax-profile-forms-search"]').fill("");
  await page.locator('[data-testid="tax-profile-save"]').click();
  await page
    .locator('[data-testid="tax-profile-notice"]')
    .waitFor({ state: "visible", timeout: 20000 });
}

async function main() {
  const browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1440, height: 960 } });
  page.setDefaultTimeout(30000);
  const shots = [];

  try {
    await registerAndEnterWorkspace(page, shots);

    await page.goto(url("/tax-profiles"), { waitUntil: "domcontentloaded" });
    await assertVisible(
      page,
      '[data-testid="tax-profile-workspace"]',
      "tax profile workspace",
    );
    shots.push(await shot(page, "02_empty_workspace"));

    if (!(await page.locator('input[name="tin_root"]').first().isVisible().catch(() => false))) {
      await page.locator('[data-testid="tax-profile-add-tin"]').click();
    }
    await assertVisible(
      page,
      '[data-testid="tax-profile-composer"]',
      "TIN composer",
    );
    await page.locator('input[name="tin_root"]').fill(tinRoot);
    await page.locator('input[name="branch_code"]').fill("00000");
    await page.locator('input[name="registered_name"]').fill("Head office");
    await page.locator('input[name="rdo_code"]').fill("039");
    await page
      .locator('[data-testid="tax-profile-composer"] button[type="submit"]')
      .click();
    await page
      .locator('[data-testid="tax-profile-branch-00000"]')
      .waitFor({ state: "visible", timeout: 20000 });
    await assertVisible(page, '[data-testid="tax-profile-editor"]', "year editor");
    shots.push(await shot(page, "03_head_office_profile"));

    await page.locator('input').filter({ hasText: "" }).first();
    await page.getByText("VAT registered this year").click();
    await page.locator('[data-testid="tax-profile-save"]').click();
    await page
      .locator('[data-testid="tax-profile-notice"]')
      .waitFor({ state: "visible", timeout: 20000 });

    await saveYearForms(page, ["0605", "1701", "2550Q"]);
    shots.push(await shot(page, "04_head_office_forms"));

    await createBranch(page, "00001", "Annex", "041");
    await page.locator('[data-testid="tax-profile-branch-00001"]').click();
    await assertVisible(page, '[data-testid="tax-profile-editor"]', "annex editor");
    shots.push(await shot(page, "05_annex_profile"));
    await saveYearForms(page, ["0605", "1701"]);
    shots.push(await shot(page, "06_annex_fewer_forms"));

    const addTin = page.locator('[data-testid="tax-profile-add-tin"]');
    if (await addTin.isEnabled()) {
      throw new Error("personal account should not add a second TIN");
    }

    await page.locator('[data-testid="tax-profile-year"]').selectOption("2027");
    await page.locator('[data-testid="tax-profile-clone-year"]').click();
    await page
      .locator('[data-testid="tax-profile-notice"]')
      .waitFor({ state: "visible", timeout: 20000 });
    shots.push(await shot(page, "07_cloned_year"));

    await page.locator('[data-testid="tax-profile-branch-00000"]').click();
    await page.locator('[data-testid="tax-profile-tab-forms"]').click();
    await page.waitForTimeout(500);
    shots.push(await shot(page, "08_head_office_still_three_forms"));

    console.log(JSON.stringify({ ok: true, email, shots }, null, 2));
  } catch (error) {
    await shot(page, "99_failure").catch(() => {});
    console.error(error);
    process.exitCode = 1;
  } finally {
    await browser.close();
  }
}

await main();
