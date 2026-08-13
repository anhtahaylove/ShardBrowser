import { expect, test, type Page } from "@playwright/test";

async function gotoMocked(page: Page, path = "/") {
  await page.goto(path, { waitUntil: "domcontentloaded" });
}

test("startup loading, profile error, and retry are deterministic", async ({ page }) => {
  await gotoMocked(page, "/?e2e=profile-error-once&profilesDelay=400");

  await expect(page.getByRole("status").filter({ hasText: "Loading profiles" })).toBeVisible();
  await expect(page.getByRole("alert").filter({ hasText: "Profiles could not be loaded" })).toBeVisible();
  await page.getByRole("button", { name: "Retry" }).evaluate((button) => {
    (button as HTMLButtonElement).click();
    (button as HTMLButtonElement).click();
  });
  await expect(page.getByText("VN Automation 001 - No Proxy")).toBeVisible();
  await expect(page.locator("html")).toHaveAttribute("data-e2e-max-profile-lists-in-flight", "1");
});

test("runtime startup error can retry into the app", async ({ page }) => {
  await gotoMocked(page, "/?e2e=runtime-error-once");

  await expect(page.getByRole("alert").filter({ hasText: "Setup failed" })).toBeVisible();
  await page.getByRole("button", { name: "Retry setup" }).evaluate((button) => {
    (button as HTMLButtonElement).click();
    (button as HTMLButtonElement).click();
  });
  await expect(page.getByRole("heading", { name: "Browsers" })).toBeVisible();
  await expect.poll(() => page.locator("html").getAttribute("data-e2e-runtime-checks")).toBe("2");
});

test("runtime install errors remain recoverable", async ({ page }) => {
  await gotoMocked(page, "/?e2e=runtime-install-error-once");

  await expect(page.getByRole("alert").filter({ hasText: "fixture runtime install failed" })).toBeVisible();
  await page.getByRole("button", { name: "Retry setup" }).click();
  await expect(page.getByRole("heading", { name: "Browsers" })).toBeVisible();
});

test("browsers search and responsive actions are keyboard reachable", async ({ page }) => {
  await gotoMocked(page);

  await expect(page.getByRole("heading", { name: "Browsers" })).toBeVisible();
  await page.keyboard.press("ControlOrMeta+K");
  await expect(page.getByRole("searchbox", { name: "Search Browsers" })).toBeFocused();
  await page.getByRole("searchbox", { name: "Search Browsers" }).fill("Beta");
  await expect(page.getByText("Beta Research")).toBeVisible();
  await expect(page.getByText("VN Automation 001 - No Proxy")).toBeHidden();
  await page.getByRole("searchbox", { name: "Search Browsers" }).fill("missing");
  await expect(page.getByRole("heading", { name: "No matching profiles" })).toBeVisible();
  await page.getByRole("button", { name: "Clear search" }).click();
  const profileName = "VN Automation 001 - No Proxy";
  await expect(page.getByRole("button", { name: `Start profile ${profileName}` })).toBeVisible();
  await expect(page.getByRole("button", { name: `Edit profile ${profileName}` })).toBeVisible();
  await expect(page.getByRole("button", { name: `More actions for profile ${profileName}` })).toBeVisible();
  await expect(page.getByRole("button", { name: `Unpin profile ${profileName}` })).toBeHidden();
  await expect(page.getByRole("checkbox", { name: "Select all profiles on this page" })).toBeVisible();
  await expect(page.getByRole("checkbox", { name: `Select profile ${profileName}` })).toBeVisible();
  await expect(page.locator(`.cell-name[title="${profileName}"]`)).toBeVisible();

  await page.getByRole("button", { name: "More actions for profile Beta Research" }).click();
  await expect(page.getByRole("button", { name: "Clone" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Copy CDP HTTP URL" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Copy DevTools inspect URL" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Delete" })).toBeVisible();
  await page.keyboard.press("Escape");
});

test("desktop-wide actions, theme, and launch errors remain visible", async ({ page }) => {
  await page.setViewportSize({ width: 1720, height: 1000 });
  await gotoMocked(page);

  const profileName = "VN Automation 001 - No Proxy";
  await expect(page.getByRole("button", { name: `Unpin profile ${profileName}` })).toBeVisible();
  await expect(page.getByRole("button", { name: "Copy CDP HTTP URL for Beta Research" })).toBeVisible();
  const profileNameText = page.locator(".cell-name .name-main", { hasText: profileName }).first();
  await expect(profileNameText).toBeVisible();
  await expect.poll(() => profileNameText.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
  await page.getByRole("button", { name: "Switch to light theme" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await page.getByRole("button", { name: `Start profile ${profileName}` }).click();
  await expect(page.locator(".launch-error-inline")).toContainText("fixture browser launch failed");
  await expect(page.locator(".toast-err")).toHaveAttribute("role", "alert");
});

test("legacy custom-font metadata survives an ordinary profile edit but stays hidden", async ({ page }) => {
  await gotoMocked(page);

  const profileName = "VN Automation 001 - No Proxy";
  await page.getByRole("button", { name: `Edit profile ${profileName}` }).click();
  await expect(page.getByText("Custom fonts", { exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "Save changes" }).click();

  await expect.poll(async () => {
    const encoded = await page.locator("html").getAttribute("data-e2e-saved-custom-fonts");
    return encoded ? JSON.parse(encoded) : null;
  }).toEqual({
    mode: "append",
    dirs: ["C:\\Fixture\\Fonts"],
    names: ["Fixture Sans"],
    random_count: 1,
  });
});

test("invalid profile names are rejected before profile persistence", async ({ page }) => {
  await gotoMocked(page);

  await page.getByRole("button", { name: "+ New profile" }).click();
  await page.getByLabel("Profile name").fill("bad/name");
  await page.getByRole("button", { name: "Create profile" }).click();

  await expect(page.getByRole("alert").filter({ hasText: "Invalid profile name" })).toBeVisible();
  await expect(page.locator("html")).toHaveAttribute("data-e2e-profile-name-validations", "1");
  await expect(page.locator("html")).not.toHaveAttribute("data-e2e-profile-saves", /.+/);
});

test("empty profiles are distinct from search results", async ({ page }) => {
  await gotoMocked(page, "/?e2e=profiles-empty");

  await expect(page.getByRole("heading", { name: "No profiles yet" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "No matching profiles" })).toHaveCount(0);
});

test("empty folders have a distinct state", async ({ page }) => {
  await gotoMocked(page, "/?e2e=folder-empty");

  await page.getByRole("button", { name: /Empty QA 0/ }).click();
  await expect(page.getByRole("heading", { name: "Folder is empty" })).toBeVisible();
});

test("proxies empty state is backend-free and sanitized", async ({ page }) => {
  await gotoMocked(page, "/?e2e=proxies-empty");

  await page.getByRole("button", { name: "Proxies" }).click();
  await expect(page.getByRole("heading", { name: "Proxies", exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "No proxies yet" })).toBeVisible();
});

test("proxy errors retry single-flight and search can be cleared", async ({ page }) => {
  await gotoMocked(page, "/?e2e=proxy-error-once&proxiesDelay=300");
  await expect(page.getByText("VN Automation 001 - No Proxy")).toBeVisible();

  await page.getByRole("button", { name: "Proxies" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Loading proxies" })).toBeVisible();
  await expect(page.getByRole("alert").filter({ hasText: "Proxies could not be loaded" })).toBeVisible();
  await page.getByRole("button", { name: "Retry" }).evaluate((button) => {
    (button as HTMLButtonElement).click();
    (button as HTMLButtonElement).click();
  });
  await expect(page.getByText("US Sanitize 1")).toBeVisible();
  await expect(page.locator("html")).toHaveAttribute("data-e2e-max-proxy-lists-in-flight", "1");

  await page.keyboard.press("ControlOrMeta+K");
  const search = page.getByRole("searchbox", { name: "Search Proxies" });
  await search.fill("missing");
  await expect(page.getByRole("heading", { name: "No matching proxies" })).toBeVisible();
  await page.getByRole("button", { name: "Clear search" }).click();
  await expect(page.getByText("US Sanitize 1")).toBeVisible();
});

test("settings dirty state and MCP readiness render from fixture", async ({ page }) => {
  await gotoMocked(page);

  await page.getByRole("button", { name: "Settings" }).click();
  await expect(page.getByText("Startup entry not registered")).toBeVisible();
  await page.getByLabel("Start ShardX Launcher when I sign in").check();
  await expect(page.getByLabel("Start in the system tray")).toBeEnabled();
  await expect(page.getByText("MCP ready", { exact: true })).toBeVisible();
  await expect(page.getByLabel("MCP setup readiness")).toContainText("API reachable");
  await expect(page.getByRole("button", { name: "Check Codex registration" })).toBeVisible();
  await page.getByRole("button", { name: "Check Codex registration" }).click();
  await expect(page.getByRole("button", { name: "Refresh status" })).toBeVisible();
  const advanced = page.getByText("Advanced actions", { exact: true });
  await advanced.click();
  await expect(page.getByRole("button", { name: "Copy Codex repair command" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Refresh status" })).toHaveCount(1);
  await page.getByLabel("Port").fill("40326");
  await expect(page.getByText("Unsaved changes")).toBeVisible();
  await expect(page.getByText("Restart required")).toBeVisible();
  await expect(page.getByRole("region", { name: "Settings save status" })).toHaveCSS("position", "sticky");
  await expect(page.getByRole("button", { name: /Save settings/ })).toBeEnabled();
});

test("startup setting registers the Launcher while MCP stays client-spawned", async ({ page }) => {
  await gotoMocked(page);
  await page.getByRole("button", { name: "Settings" }).click();

  await page.getByLabel("Start ShardX Launcher when I sign in").check();
  await page.getByRole("button", { name: /Save settings/ }).click();

  await expect(page.getByText("Startup entry registered")).toBeVisible();
  await expect(page.getByText(/MCP server remains a lightweight stdio process/)).toBeVisible();
  await expect(page.getByText("All changes saved")).toBeVisible();
});

test("Settings exposes one honest Codex repair action", async ({ page }) => {
  await gotoMocked(page, "/?e2e=codex-needs-repair");

  await page.getByRole("button", { name: "Settings" }).click();
  await page.getByRole("button", { name: "Check Codex registration" }).click();
  await expect(page.getByRole("button", { name: "Copy Codex repair command" })).toBeVisible();
  await page.getByText("Advanced actions", { exact: true }).click();
  await expect(page.getByRole("button", { name: "Copy Codex repair command" })).toHaveCount(1);
  await expect(page.getByText("no config is changed automatically")).toBeVisible();
});

test("pages without search do not expose a fake shortcut", async ({ page }) => {
  await gotoMocked(page);

  for (const [navName, heading] of [["Settings", "Settings"], ["Fingerprints", "Fingerprint Library"], ["ProxyShard", "ProxyShard"]]) {
    await page.getByRole("button", { name: navName }).click();
    await expect(page.getByRole("heading", { name: heading, exact: true })).toBeVisible();
    await expect(page.getByRole("searchbox")).toHaveCount(0);
  }
});

test("macOS uses Cmd+K and Escape clears then blurs search", async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      get: () => "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) AppleWebKit/537.36 Chrome/125 Safari/537.36",
    });
  });
  await gotoMocked(page);

  await expect(page.getByText("⌘K", { exact: true })).toBeVisible();
  await page.keyboard.press("Meta+K");
  const search = page.getByRole("searchbox", { name: "Search Browsers" });
  await expect(search).toBeFocused();
  await search.fill("Beta");
  await search.press("Escape");
  await expect(search).toHaveValue("");
  await search.press("Escape");
  await expect(search).not.toBeFocused();
});

test("CSSelect supports Arrow, Enter, Space, and Escape without saving", async ({ page }) => {
  await gotoMocked(page);

  await page.getByRole("button", { name: "+ New profile" }).click();
  const proxySelect = page.locator("label").filter({ has: page.getByText("Proxy", { exact: true }) }).getByRole("combobox").first();
  await proxySelect.focus();
  await proxySelect.press("ArrowDown");
  await expect(proxySelect).toHaveAttribute("aria-expanded", "true");
  await proxySelect.press("ArrowDown");
  await proxySelect.press("Enter");
  await expect(proxySelect).toContainText("US Sanitize 1");
  await proxySelect.press("Space");
  await expect(proxySelect).toHaveAttribute("aria-expanded", "true");
  await proxySelect.press("Escape");
  await expect(proxySelect).toHaveAttribute("aria-expanded", "false");
  await expect(page.getByText("Custom fonts", { exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "Cancel" }).click();
});

test("updater requires consent, reports progress, and requests restart", async ({ page }) => {
  await gotoMocked(page, "/?updateDownloadStepDelay=500");

  await expect(page.getByText("Update available → v0.1.28")).toBeVisible();
  const trigger = page.getByRole("button", { name: /ShardX Launcher v0.1.27/ });
  await trigger.focus();
  await trigger.click();
  await expect(page.getByRole("dialog", { name: "ShardX Launcher update" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: "ShardX Launcher update" })).toBeHidden();
  await expect(trigger).toBeFocused();

  await trigger.click();
  await page.getByRole("button", { name: "Download update" }).click();
  await expect(page.getByRole("progressbar", { name: "Download Launcher update" })).toBeVisible();
  await expect(page.getByText("Signature verified — ready to install")).toBeVisible();
  await expect(page.locator("html")).toHaveAttribute("data-e2e-update-downloads", "1");
  await page.getByRole("button", { name: "Install and restart" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-e2e-update-installs", "1");
  await expect(page.locator("html")).toHaveAttribute("data-e2e-restart-requests", "1");
});

test("updater surfaces invalid signatures inline", async ({ page }) => {
  await gotoMocked(page, "/?update=invalid-signature");

  await page.getByRole("button", { name: /ShardX Launcher v0.1.27/ }).click();
  await page.getByRole("button", { name: "Download update" }).click();
  await expect(page.getByRole("alert").filter({ hasText: "Update could not be completed" })).toBeVisible();
  await expect(page.getByRole("alert")).toContainText("signature verification failed");
});

test("updater surfaces offline checks inline", async ({ page }) => {
  await gotoMocked(page, "/?update=check-error");

  await expect(page.getByText("update check needs attention")).toBeVisible();
  await page.getByRole("button", { name: /ShardX Launcher v/ }).click();
  await expect(page.getByRole("button", { name: "Retry" })).toBeVisible();
});

test("updater distinguishes checking and up-to-date states", async ({ page }) => {
  await gotoMocked(page, "/?update=none&updateHold=1");

  await expect(page.locator("html")).toHaveAttribute("data-e2e-update-check-pending", "true");
  await expect(page.getByText("checking for updates…")).toBeVisible();
  await page.evaluate(() => (window as Window & { __resolveUpdateCheck?: () => void }).__resolveUpdateCheck?.());
  await expect(page.getByText("up to date", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: /ShardX Launcher v0.1.27/ }).click();
  await expect(page.getByText("Launcher is up to date")).toBeVisible();
  await expect(page.getByRole("button", { name: "Check again" })).toBeVisible();
});
