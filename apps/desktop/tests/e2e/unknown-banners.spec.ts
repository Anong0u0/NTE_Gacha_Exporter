import { expect, test, type Page } from "@playwright/test";

const mockScenarioKey = "nte.mockScenario";
const prefsKey = "nte.recordView.v1:default";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(
    ({ mockScenarioKey, prefsKey }) => {
      window.localStorage.setItem(mockScenarioKey, "unknown-banners");
      window.localStorage.removeItem(prefsKey);
    },
    { mockScenarioKey, prefsKey },
  );
});

test("unmapped banners render in dashboard rail and record filter without visuals", async ({ page }) => {
  await page.goto("/");

  const rail = page.locator('[data-agent-id="dashboard-banner-rail"]');
  await expect(rail.getByRole("button", { name: "未收錄限定" })).toBeVisible();
  await expect(rail.getByRole("button", { name: "未收錄限定" }).locator(".rail-thumb.empty")).toBeVisible();

  await page.locator('[data-agent-id="nav-records"]').click();
  await openBannerFilter(page);

  await expect(page.locator(".multi-select-option").filter({ hasText: "未收錄限定" })).toBeVisible();
  const unknownFork = page.locator(".multi-select-option").filter({ hasText: "KaesiNew" });
  await expect(unknownFork).toBeVisible();
  await expect(unknownFork).not.toContainText("ForkLottery_");
});

test("stale synthetic banner filter selection is cleared after maps update", async ({ page }) => {
  await page.goto("/");
  await page.locator('[data-agent-id="nav-records"]').click();
  await openBannerFilter(page);
  await page.locator(".multi-select-option").filter({ hasText: "未收錄限定" }).click();

  await expect
    .poll(() => page.evaluate((key) => JSON.parse(window.localStorage.getItem(key) ?? "{}").recordBannerIds, prefsKey))
    .toEqual(["CardPool_Character"]);

  await page.evaluate((key) => window.localStorage.setItem(key, "default"), mockScenarioKey);
  await page.reload();
  await page.locator('[data-agent-id="nav-records"]').click();

  await expect
    .poll(() => page.evaluate((key) => JSON.parse(window.localStorage.getItem(key) ?? "{}").recordBannerIds, prefsKey))
    .toEqual([]);

  const bannerFilter = bannerFilterTrigger(page);
  await expect(bannerFilter).toContainText("全部卡池");
});

async function openBannerFilter(page: Page) {
  const trigger = bannerFilterTrigger(page);
  await trigger.click();
  await expect(page.locator(".multi-select-menu")).toBeVisible();
}

function bannerFilterTrigger(page: Page) {
  return page
    .locator(".filter-grid.basic .field")
    .filter({ hasText: "卡池" })
    .locator(".multi-select-trigger");
}
