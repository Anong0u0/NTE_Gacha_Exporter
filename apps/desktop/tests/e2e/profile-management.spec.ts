import { expect, test, type Locator, type Page } from "@playwright/test";
import { profileAgentId } from "../../src/app/profileNames";

test.beforeEach(async ({ page }) => {
  await page.setViewportSize({ width: 1130, height: 810 });
  await page.goto("/");
  await expect(page.locator('[data-agent-id="profile-list"]')).toBeVisible();
});

test("profile list grows from one through five rows, then scrolls the sixth into view", async ({ page }) => {
  const list = page.locator('[data-agent-id="profile-list"]');
  await expect(page.locator(".sidebar-profile-row")).toHaveCount(1);

  const single = await listGeometry(list);
  expect(single.clientWidth).toBe(206);
  expect(single.scrollWidth).toBe(206);
  expect(single.clientHeight).toBe(44);
  expect(single.scrollHeight).toBe(44);
  expect(single.height).toBeCloseTo(46, 0);
  expect(single.rows).toEqual([{ height: 44, width: 206, rightGap: 0, inside: true }]);

  for (const name of ["alpha", "bravo", "charlie", "delta"]) await createProfile(page, name);
  await expect(page.locator(".sidebar-profile-row")).toHaveCount(5);

  const five = await listGeometry(list);
  expect(five.clientWidth).toBe(206);
  expect(five.scrollWidth).toBe(206);
  expect(five.clientHeight).toBe(220);
  expect(five.scrollHeight).toBe(220);
  expect(five.height).toBeCloseTo(222, 0);
  expect(five.rows).toHaveLength(5);
  expect(five.rows.every((row) => row.height === 44 && row.width === 206 && row.rightGap === 0 && row.inside)).toBe(true);

  await createProfile(page, "zzzzzz");
  await expect(page.locator(".sidebar-profile-row")).toHaveCount(6);
  const activeRow = page.locator('[data-agent-id="profile-row-zzzzzz"]');
  await expect(activeRow).toHaveClass(/active/);

  await expect.poll(async () => (await listGeometry(list)).scrollTop).toBeGreaterThan(0);
  const six = await listGeometry(list);
  expect(six.clientHeight).toBe(220);
  expect(six.scrollHeight).toBe(264);
  expect(six.height).toBeCloseTo(222, 0);
  expect(await elementInside(activeRow, list)).toBe(true);
  expect(await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth)).toBe(0);
  await expect(page.locator(".five-wall-shell")).not.toHaveClass(/is-collapsed/);
});

test("profile rows expose distinct inactive, hover, active, and focus states", async ({ page }) => {
  await createProfile(page, "alpha");
  const inactiveRow = page.locator('[data-agent-id="profile-row-default"]');
  const activeRow = page.locator('[data-agent-id="profile-row-alpha"]');
  const inactiveSelect = page.locator('[data-agent-id="profile-select-default"]');
  const inactiveMenu = page.locator('[data-agent-id="profile-menu-default"]');

  await page.mouse.move(600, 700);
  await expect(page.locator('[data-agent-id="profile-list"]')).toHaveCSS("background-color", "rgb(16, 28, 24)");
  await expect(inactiveRow).toHaveCSS("background-color", "rgb(27, 42, 37)");
  await expect(inactiveRow).toHaveCSS("border-bottom-color", "rgb(52, 74, 66)");
  await expect(activeRow).toHaveCSS("background-color", "rgb(48, 72, 62)");
  await expect(activeRow.locator(".profile-select svg")).toHaveCSS("color", "rgb(239, 196, 90)");

  await inactiveSelect.hover();
  await expect(inactiveRow).toHaveCSS("background-color", "rgb(38, 58, 50)");

  await page.mouse.move(600, 700);
  await expect(page.locator('[data-agent-id="profile-create-open"]')).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(inactiveSelect).toBeFocused();
  await expect(inactiveSelect).toHaveCSS("outline-color", "rgb(111, 159, 146)");
  await expect(inactiveSelect).toHaveCSS("outline-width", "2px");
  await expect(inactiveSelect).toHaveCSS("outline-offset", "-2px");

  await page.keyboard.press("Tab");
  await expect(inactiveMenu).toBeFocused();
  await expect(inactiveMenu).toHaveCSS("outline-color", "rgb(111, 159, 146)");
  await expect(inactiveMenu).toHaveCSS("border-left-color", "rgb(65, 91, 80)");
});

test("long names, menu keyboard controls, rename, and delete remain contained", async ({ page }) => {
  const longName = `profile_${"x".repeat(32)}`;
  const renamed = `renamed_${"y".repeat(32)}`;
  await createProfile(page, longName);

  const select = page.locator(`[data-agent-id="${profileAgentId("select", longName)}"]`);
  await expect(select).toHaveAttribute("title", longName);
  await expect(select).toHaveAttribute("aria-label", new RegExp(longName));
  expect(await select.locator("strong").evaluate((element) => element.scrollWidth > element.clientWidth)).toBe(true);

  const trigger = page.locator(`[data-agent-id="${profileAgentId("menu", longName)}"]`);
  await trigger.click();
  const menu = page.locator("#profile-actions-menu");
  await expect(menu).toBeVisible();
  await expect(page.locator(`[data-agent-id="${profileAgentId("rename", longName)}"]`)).toBeFocused();
  expect(await elementInsideViewport(menu)).toBe(true);

  await page.keyboard.press("ArrowDown");
  await expect(page.locator(`[data-agent-id="${profileAgentId("delete", longName)}"]`)).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(menu).toBeHidden();
  await expect(trigger).toBeFocused();

  await trigger.click();
  await page.locator(`[data-agent-id="${profileAgentId("rename", longName)}"]`).click();
  const renameInput = page.locator('[data-agent-id="profile-rename-input"]');
  await expect(renameInput).toBeVisible();
  await expect(renameInput).toBeFocused();
  await expect(renameInput).toHaveValue(longName);
  await renameInput.fill(renamed);
  await page.locator('[data-agent-id="profile-rename-save"]').click();
  await expect(page.locator(`[data-agent-id="${profileAgentId("row", renamed)}"]`)).toHaveClass(/active/);
  await expect(page.locator(".profile-dialog")).toBeHidden();

  await page.locator(`[data-agent-id="${profileAgentId("menu", renamed)}"]`).click();
  await page.locator(`[data-agent-id="${profileAgentId("delete", renamed)}"]`).click();
  await expect(page.locator(".profile-dialog-copy")).toContainText(renamed);
  await page.locator('[data-agent-id="profile-dialog-cancel"]').click();
  await expect(page.locator(`[data-agent-id="${profileAgentId("row", renamed)}"]`)).toBeVisible();

  await page.locator(`[data-agent-id="${profileAgentId("menu", renamed)}"]`).click();
  await page.locator(`[data-agent-id="${profileAgentId("delete", renamed)}"]`).click();
  await page.locator(`[data-agent-id="${profileAgentId("delete-confirm", renamed)}"]`).click();
  await expect(page.locator(`[data-agent-id="${profileAgentId("row", renamed)}"]`)).toHaveCount(0);
  await expect(page.locator('[data-agent-id="profile-row-default"]')).toHaveClass(/active/);
});

test("Unicode profile names can be created, renamed, and deleted", async ({ page }) => {
  const original = "玩家 一號✨";
  const renamed = "旅行者 二號🌙";
  await createProfile(page, original);

  const originalRow = page.locator(`[data-agent-id="${profileAgentId("row", original)}"]`);
  await expect(originalRow).toHaveClass(/active/);
  await expect(originalRow.locator("strong")).toHaveText(original);
  expect(await elementInside(originalRow, page.locator('[data-agent-id="profile-list"]'))).toBe(true);

  await page.locator(`[data-agent-id="${profileAgentId("menu", original)}"]`).click();
  await page.locator(`[data-agent-id="${profileAgentId("rename", original)}"]`).click();
  await page.locator('[data-agent-id="profile-rename-input"]').fill(renamed);
  await page.locator('[data-agent-id="profile-rename-save"]').click();

  const renamedRow = page.locator(`[data-agent-id="${profileAgentId("row", renamed)}"]`);
  await expect(renamedRow).toHaveClass(/active/);
  await expect(renamedRow.locator("strong")).toHaveText(renamed);
  await expect(originalRow).toHaveCount(0);

  await page.locator(`[data-agent-id="${profileAgentId("menu", renamed)}"]`).click();
  await page.locator(`[data-agent-id="${profileAgentId("delete", renamed)}"]`).click();
  await page.locator(`[data-agent-id="${profileAgentId("delete-confirm", renamed)}"]`).click();
  await expect(renamedRow).toHaveCount(0);
  await expect(page.locator('[data-agent-id="profile-row-default"]')).toHaveClass(/active/);
});

test("last-profile protection, API errors, native validation, and dialog focus stay correct", async ({ page }) => {
  await page.locator('[data-agent-id="profile-menu-default"]').click();
  const lastDelete = page.locator('[data-agent-id="profile-delete-default"]');
  await expect(lastDelete).toBeDisabled();
  await expect(lastDelete).toHaveAttribute("title", /至少需要保留一個個人檔案/);
  await page.keyboard.press("Escape");

  await page.locator('[data-agent-id="profile-create-open"]').click();
  const createInput = page.locator('[data-agent-id="profile-create-input"]');
  await createInput.fill("alpha");
  await createInput.press("Enter");
  await expect(page.locator('[data-agent-id="profile-row-alpha"]')).toHaveClass(/active/);

  await page.locator('[data-agent-id="profile-menu-default"]').click();
  await page.locator('[data-agent-id="profile-rename-default"]').click();
  const renameInput = page.locator('[data-agent-id="profile-rename-input"]');
  await renameInput.fill("ALPHA");
  await page.locator('[data-agent-id="profile-rename-save"]').click();
  await expect(page.locator(".profile-dialog")).toBeVisible();
  await expect(page.locator(".profile-dialog-error")).toContainText("已存在同名個人檔案");
  await page.keyboard.press("Escape");
  await expect(page.locator(".profile-dialog")).toBeHidden();

  await page.locator('[data-agent-id="profile-create-open"]').click();
  await createInput.fill("bad/name");
  await page.locator('[data-agent-id="profile-create-submit"]').click();
  expect(await createInput.evaluate((input: HTMLInputElement) => input.checkValidity())).toBe(true);
  await expect(page.locator(".profile-dialog-error")).toContainText("無法用於 Windows 資料夾");
  await expect(page.locator(".profile-dialog")).toBeVisible();

  await createInput.fill("valid_name");
  const submit = page.locator('[data-agent-id="profile-create-submit"]');
  await submit.focus();
  await page.keyboard.press("Tab");
  await expect(page.locator(".profile-dialog .update-dialog-head button")).toBeFocused();
  await page.locator('[data-agent-id="profile-dialog-cancel"]').click();
});

async function createProfile(page: Page, name: string) {
  await page.locator('[data-agent-id="profile-create-open"]').click();
  await page.locator('[data-agent-id="profile-create-input"]').fill(name);
  await page.locator('[data-agent-id="profile-create-submit"]').click();
  await expect(page.locator(`[data-agent-id="${profileAgentId("row", name.normalize("NFC"))}"]`)).toHaveClass(/active/);
  await expect(page.locator(".profile-dialog")).toBeHidden();
}

async function listGeometry(list: Locator) {
  return list.evaluate((element) => {
    const listRect = element.getBoundingClientRect();
    const rows = [...element.querySelectorAll<HTMLElement>(".sidebar-profile-row")].map((row) => {
      const rect = row.getBoundingClientRect();
      return {
        height: Math.round(rect.height),
        width: Math.round(rect.width),
        rightGap: Math.round((listRect.right - element.clientLeft - rect.right) * 100) / 100,
        inside: rect.top >= listRect.top + 0.5 && rect.bottom <= listRect.bottom - 0.5,
      };
    });
    return {
      height: listRect.height,
      clientWidth: element.clientWidth,
      scrollWidth: element.scrollWidth,
      clientHeight: element.clientHeight,
      scrollHeight: element.scrollHeight,
      scrollTop: element.scrollTop,
      rows,
    };
  });
}

async function elementInside(element: Locator, container: Locator) {
  const [elementBox, containerBox] = await Promise.all([element.boundingBox(), container.boundingBox()]);
  if (!elementBox || !containerBox) return false;
  return elementBox.y >= containerBox.y + 0.5
    && elementBox.y + elementBox.height <= containerBox.y + containerBox.height - 0.5;
}

async function elementInsideViewport(element: Locator) {
  return element.evaluate((node) => {
    const rect = node.getBoundingClientRect();
    return rect.top >= 0 && rect.left >= 0 && rect.right <= window.innerWidth && rect.bottom <= window.innerHeight;
  });
}
