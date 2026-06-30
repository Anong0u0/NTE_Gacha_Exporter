import { expect, test } from "@playwright/test";

const mockScenarioKey = "nte.mockScenario";

test.beforeEach(async ({ page }) => {
  await page.addInitScript((key) => {
    window.localStorage.setItem(key, "capture-stalled");
  }, mockScenarioKey);
});

test("capture window stalled opens retry dialog", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "更新資料" }).click();

  const stalledDialog = page.getByRole("dialog", { name: "擷取失敗" });
  await expect(stalledDialog).toBeVisible();
  await expect(stalledDialog).toContainText("auto_page_capture_window_stalled");
  await expect(stalledDialog).toContainText("可能網路延遲");
  await expect(stalledDialog.getByRole("button", { name: /降低速度重試.*500ms/ })).toBeVisible();

  await stalledDialog.getByRole("button", { name: /降低速度重試.*500ms/ }).click();
  await expect(stalledDialog).toBeHidden();
});

test("topbar status details do not duplicate stalled retry action", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "更新資料" }).click();

  const stalledDialog = page.getByRole("dialog", { name: "擷取失敗" });
  await expect(stalledDialog).toBeVisible();
  await stalledDialog.getByRole("button", { name: "關閉" }).first().click();

  await page.locator('[data-agent-id="topbar-status"]').click();
  const statusDialog = page.getByRole("dialog", { name: "擷取狀態" });
  await expect(statusDialog).toBeVisible();
  await expect(statusDialog).not.toContainText("可能網路延遲");
  await expect(statusDialog).not.toContainText("降低速度重試");
});
