import { expect, test } from "@playwright/test";

const mockScenarioKey = "nte.mockScenario";

test.beforeEach(async ({ page }) => {
  await page.addInitScript((key) => {
    window.localStorage.setItem(key, "capture-partial");
  }, mockScenarioKey);
});

test("stop during drain saves partial data and offers a slower retry", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "更新資料" }).click();

  await expect(page.getByRole("button", { name: "停止" })).toBeVisible();
  await page.locator('[data-agent-id="topbar-status"]').click();
  const runningStatus = page.getByRole("dialog", { name: "擷取狀態" });
  await expect(runningStatus).toContainText("waiting for capture drain");
  await runningStatus.getByRole("button", { name: "關閉" }).click();

  await page.getByRole("button", { name: "停止" }).click();

  const partialDialog = page.getByRole("dialog", { name: "部分資料已保存" });
  await expect(partialDialog).toBeVisible();
  await expect(partialDialog.locator('[data-agent-id="capture-partial-message"]')).toContainText("可能不完整");
  await expect(partialDialog.locator('[data-agent-id="capture-partial-import-summary"]')).toContainText("新增 5，略過 150");
  await expect(partialDialog.locator('[data-agent-id="capture-partial-pages"]')).toContainText("limited：已解碼 53/55 頁，缺少 2 頁");
  await expect(partialDialog.locator('[data-agent-id="capture-partial-pages"]')).toContainText("standard：已解碼 59/60 頁，缺少 1 頁");
  await expect(partialDialog.locator('[data-agent-id="capture-partial-pages"]')).not.toContainText("fork：已解碼");
  await expect(partialDialog).not.toContainText("擷取失敗");
  await expect(partialDialog.locator('[data-agent-id="capture-recovery-retry_auto_page_slower"]')).toContainText("500ms");

  await partialDialog.locator('[data-agent-id="capture-recovery-close"]').click();
  await expect(partialDialog).toBeHidden();
  await expect(page.locator('[data-agent-id="topbar-status"]')).toHaveClass(/partial/);
  await expect(page.locator('[data-agent-id="topbar-status"]')).toContainText("部分資料已保存");

  await page.locator('[data-agent-id="topbar-status"]').click();
  const partialStatus = page.getByRole("dialog", { name: "擷取狀態" });
  await expect(partialStatus.locator('[data-agent-id="capture-partial-reason"]')).toContainText("capture stopped before packet drain completed");
  await expect(partialStatus).toContainText("2 卡池完成");
  await expect(partialStatus).toContainText("1 卡池略過");
  await partialStatus.locator('[data-agent-id="capture-partial-retry"]').click();

  await expect(page.getByRole("dialog", { name: "擷取狀態" })).toBeHidden();
  await expect(page.locator('[data-agent-id="topbar-status"]')).not.toHaveClass(/partial/);
  await expect(page.locator('[data-agent-id="topbar-status"]')).toContainText("已合併");
});

test("full update partial keeps existing records and retry mode", async ({ page }) => {
  await page.goto("/");
  await page.locator('[data-agent-id="nav-records"]').click();
  const history = page.locator('[data-agent-id="records-history"]');
  await expect(history).toContainText("Sigrid");

  await page.getByRole("button", { name: "更新選項" }).click();
  await page.getByLabel("完整更新").check();
  await page.getByRole("button", { name: "更新資料" }).click();
  await expect(page.getByRole("button", { name: "停止" })).toBeVisible();
  await page.getByRole("button", { name: "停止" }).click();

  const partialDialog = page.getByRole("dialog", { name: "部分資料已保存" });
  await expect(partialDialog).toContainText("完整更新");
  await partialDialog.locator('[data-agent-id="capture-recovery-close"]').click();

  await expect(history).toContainText("Sigrid");
  await page.locator('[data-agent-id="profile-create-open"]').click();
  await page.locator('[data-agent-id="profile-create-input"]').fill("other");
  await page.locator('[data-agent-id="profile-create-submit"]').click();
  await expect(page.locator('[data-agent-id="profile-row-other"]')).toHaveClass(/active/);

  await page.locator('[data-agent-id="nav-settings"]').click();
  const dataLocale = page.getByLabel("資料語言");
  await dataLocale.selectOption("en");
  await expect(dataLocale).toHaveValue("en");
  await page.getByRole("button", { name: "更新選項" }).click();
  await page.getByLabel("完整更新").uncheck();
  await expect(page.getByLabel("完整更新")).not.toBeChecked();

  await page.locator('[data-agent-id="topbar-status"]').click();
  const statusDialog = page.getByRole("dialog", { name: "擷取狀態" });
  await expect(statusDialog).toContainText("完整更新");
  await statusDialog.locator('[data-agent-id="capture-partial-retry"]').click();

  await expect(page.locator('[data-agent-id="topbar-status"]')).toContainText("已合併");
  await expect(page.locator('[data-agent-id="profile-row-default"]')).toHaveClass(/active/);
  await page.locator('[data-agent-id="topbar-status"]').click();
  const completedStatus = page.getByRole("dialog", { name: "擷取狀態" });
  await expect(completedStatus).toContainText("完整更新");
  await completedStatus.getByRole("button", { name: "關閉" }).click();
  await page.locator('[data-agent-id="nav-settings"]').click();
  await expect(page.getByLabel("資料語言")).toHaveValue("zh-Hant");
  await page.getByRole("button", { name: "更新選項" }).click();
  await expect(page.getByLabel("完整更新")).toBeChecked();
});
