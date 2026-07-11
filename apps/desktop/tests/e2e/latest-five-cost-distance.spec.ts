import { expect, test, type Page } from "@playwright/test";

const mockScenarioKey = "nte.mockScenario";
const defaultProfile = "default";
const altProfile = "cost_alt";
const defaultPrefsKey = `nte.recordView.v1:${defaultProfile}`;
const altPrefsKey = `nte.recordView.v1:${altProfile}`;
const sameTenAllRecordIds = [
  "mock-fork-cost-160",
  "mock-fork-cost-159",
  "mock-fork-cost-158",
  "mock-fork-cost-157",
  "mock-fork-cost-156",
];
const sameTenFocusedRecordIds = ["mock-fork-cost-160", "mock-fork-cost-156"];
const sameTenAllGroupKey = sameTenAllRecordIds.join(":");

test("1130px keeps same-ten hits full-size and filters x5 to x2", async ({ page }) => {
  await useMockScenario(page, "latest-five-cost-distance", [defaultPrefsKey, altPrefsKey]);
  await page.setViewportSize({ width: 1130, height: 900 });
  await page.goto("/");

  const wall = latestFiveWall(page);
  await expect(page.locator('.pool-strip [data-pool-kind="monopoly_limited"]')).toBeVisible();
  await expect(wall).toBeVisible();
  await expect(wall.locator(".latest-distance-toggle")).toHaveCount(0);

  await selectForkPool(page);
  await expect(page.locator('.pool-strip [data-pool-kind="fork_lottery"]')).toHaveAttribute("data-current-pity", "12");
  await expect(page.locator('.pool-strip [data-pool-kind="fork_lottery"]')).toContainText("300 抽");
  await expect(page.locator(".summary-metric-card--total-pulls")).toContainText("300");
  await expect(page.locator(".summary-metric-card--five-pity")).toContainText("12/60");
  const distanceToggle = wall.locator(".latest-distance-toggle");
  const wallToggle = wall.locator(".latest-item-toggle");

  await expect(distanceToggle).toContainText("實際抽數");
  await expect(distanceToggle).toHaveAttribute("aria-pressed", "false");
  await expect(wall.locator(".latest-five-actions > button").nth(0)).toHaveClass(/latest-distance-toggle/);
  await expect(wall.locator(".latest-five-actions > button").nth(1)).toHaveClass(/latest-item-toggle/);
  await expectWallGroups(page, [
    group("80", "mock-fork-cost-288"),
    group("16", "mock-fork-cost-208"),
    group("32", "mock-fork-cost-192"),
    group("4", "mock-fork-cost-160"),
    group("76", "mock-fork-cost-156"),
    group("80", "mock-fork-cost-80"),
  ], "actual");

  await distanceToggle.click();
  await expect(distanceToggle).toContainText("成本抽數");
  await expect(distanceToggle).toHaveAttribute("aria-pressed", "true");
  await expectWallGroups(page, [
    group("80", "mock-fork-cost-288"),
    group("10", "mock-fork-cost-208"),
    group("40", "mock-fork-cost-192"),
    group("80", ...sameTenFocusedRecordIds),
    group("80", "mock-fork-cost-80"),
  ], "cost");
  await expectWallDistanceSum(page, 290);
  await expectGroupCountBadge(page, sameTenFocusedRecordIds.join(":"), "x2");
  await expectUniformTileGeometry(page);
  await expectTenPixelGridGaps(page);
  await expect
    .poll(() => page.evaluate((key) => JSON.parse(window.localStorage.getItem(key) ?? "{}").latestFiveStarDistanceModes?.fork_lottery, defaultPrefsKey))
    .toBe("cost");

  await wallToggle.click();
  await expect(wallToggle).toContainText("全部 5★");
  await expectWallGroups(page, [
    group("20", "mock-fork-cost-288"),
    group("60", "mock-fork-cost-268", "mock-fork-cost-267"),
    group("10", "mock-fork-cost-208"),
    group("40", "mock-fork-cost-192"),
    group("60", ...sameTenAllRecordIds),
    group("20", "mock-fork-cost-100"),
    group("80", "mock-fork-cost-80"),
  ], "cost");
  await expectWallDistanceSum(page, 290);
  await expectGroupCountBadge(page, sameTenAllGroupKey, "x5");
  await expectGroupTooltipUniform(page, sameTenAllGroupKey);
  await expectUniformTileGeometry(page);
  await expectTenPixelGridGaps(page);

  await wallToggle.click();
  await expect(wallToggle).toContainText("只看 UP");
  await expectWallGroups(page, [
    group("80", "mock-fork-cost-288"),
    group("10", "mock-fork-cost-208"),
    group("40", "mock-fork-cost-192"),
    group("80", ...sameTenFocusedRecordIds),
    group("80", "mock-fork-cost-80"),
  ], "cost");
  await expectGroupCountBadge(page, sameTenFocusedRecordIds.join(":"), "x2");

  await page.reload();
  await selectForkPool(page);
  await expect(distanceToggle).toContainText("成本抽數");
  await expect(distanceToggle).toHaveAttribute("aria-pressed", "true");
  await expectWallGroups(page, [
    group("80", "mock-fork-cost-288"),
    group("10", "mock-fork-cost-208"),
    group("40", "mock-fork-cost-192"),
    group("80", ...sameTenFocusedRecordIds),
    group("80", "mock-fork-cost-80"),
  ], "cost");
  await expectWallDistanceSum(page, 290);
  await expectGroupCountBadge(page, sameTenFocusedRecordIds.join(":"), "x2");

  await createProfile(page, altProfile);
  await selectForkPool(page);
  await expect(distanceToggle).toContainText("實際抽數");
  await expect(distanceToggle).toHaveAttribute("aria-pressed", "false");
  await expectWallGroups(page, [
    group("80", "mock-fork-cost-288"),
    group("16", "mock-fork-cost-208"),
    group("32", "mock-fork-cost-192"),
    group("4", "mock-fork-cost-160"),
    group("76", "mock-fork-cost-156"),
    group("80", "mock-fork-cost-80"),
  ], "actual");

  await page.locator(`[data-agent-id="profile-select-${defaultProfile}"]`).click();
  await expect(page.locator(`[data-agent-id="profile-row-${defaultProfile}"]`)).toHaveClass(/active/);
  await selectForkPool(page);
  await expect(distanceToggle).toContainText("成本抽數");
  await expect(distanceToggle).toHaveAttribute("aria-pressed", "true");
  await expectWallGroups(page, [
    group("80", "mock-fork-cost-288"),
    group("10", "mock-fork-cost-208"),
    group("40", "mock-fork-cost-192"),
    group("80", ...sameTenFocusedRecordIds),
    group("80", "mock-fork-cost-80"),
  ], "cost");
  await expectWallDistanceSum(page, 290);
});

test("800px wraps frame without enclosing neighbors and swaps collapsed distance proxy", async ({ page }) => {
  await useMockScenario(page, "latest-five-cost-distance", [defaultPrefsKey]);
  await page.setViewportSize({ width: 800, height: 900 });
  await page.goto("/");
  await selectForkPool(page);

  const wall = latestFiveWall(page);
  await wall.locator(".latest-distance-toggle").click();
  await wall.locator(".latest-item-toggle").click();
  await expectWallDistanceSum(page, 290);

  const frame = wall.locator(`.five-wall-group-frame[data-five-wall-group-key="${sameTenAllGroupKey}"]`);
  await expect(frame).toHaveAttribute("data-five-wall-segment-count", "2");
  await expect(frame).toHaveAttribute("data-five-wall-crosses-row", "true");
  await expect(frame).toHaveAttribute("data-five-wall-crosses-first-row", "true");
  await expect(frame).toHaveAttribute("data-five-wall-palette-index", "1");
  await expect(frame.locator("path")).toHaveCount(0);
  await expectGroupCountBadge(page, sameTenAllGroupKey, "x5");

  const shell = wall.locator(".five-wall-shell");
  await expect(shell).toHaveClass(/is-collapsed/);
  await expectDistancePlacement(page, sameTenAllGroupKey, "proxy");
  const collapsed = await wallGeometry(page, sameTenAllGroupKey);
  expect(collapsed.framePointerEvents).toBe("none");
  expect(collapsed.maskZ).toBeGreaterThan(collapsed.frameZ);
  expect(collapsed.shellHeight).toBeCloseTo(collapsed.tileHeight + 62, 0);
  expect(collapsed.segments).toHaveLength(2);
  expect(collapsed.segments.every((segment) => segment.rx === "10")).toBe(true);
  expect(new Set(collapsed.segments.map((segment) => segment.fill)).size).toBe(1);
  expect(new Set(collapsed.segments.map((segment) => segment.stroke)).size).toBe(1);
  expect(collapsed.segments[0].fill).not.toBe("none");
  expect(collapsed.segments[0].stroke).not.toBe("none");
  expect(collapsed.segments[0].right).toBeCloseTo(collapsed.gridWidth - 0.5, 0);
  expect(collapsed.segments[1].left).toBeCloseTo(0.5, 0);
  expect(collapsed.groupTiles[0].left).toBeGreaterThan(collapsed.firstColumnLeft + collapsed.tileWidth);
  expect(collapsed.groupTiles.some((tile) => tile.top > collapsed.groupTiles[0].top + 1)).toBe(true);
  expect(collapsed.unrelatedCentersInsideSegments).toEqual([]);
  expect(collapsed.countBadge.right).toBeCloseTo(collapsed.gridLeft + collapsed.segments[0].right + 3, 0);
  expect(collapsed.countBadge.top).toBeCloseTo(collapsed.gridTop + collapsed.segments[0].top - 3, 0);
  expect(collapsed.distanceBadge.right).toBeCloseTo(collapsed.gridLeft + collapsed.segments[0].right + 3, 0);
  expect(collapsed.distanceBadge.bottom).toBeCloseTo(collapsed.gridTop + collapsed.segments[0].bottom + 3, 0);

  await wall.locator('[data-agent-id="dashboard-five-wall-toggle"]').click();
  await expect(shell).toHaveClass(/is-expanded/);
  await expectDistancePlacement(page, sameTenAllGroupKey, "terminal");
  const expanded = await wallGeometry(page, sameTenAllGroupKey);
  expect(expanded.shellHeight).toBeGreaterThan(collapsed.shellHeight);
  expect(expanded.paletteIndex).toBe(collapsed.paletteIndex);
  expect(expanded.firstRowTiles).toEqual(collapsed.firstRowTiles);
  expect(expanded.distanceBadge.right).toBeCloseTo(expanded.gridLeft + expanded.segments[1].right + 3, 0);
  expect(expanded.distanceBadge.bottom).toBeCloseTo(expanded.gridTop + expanded.segments[1].bottom + 3, 0);
});

test("pool-kind carry drives fork and character distances across banners", async ({ page }) => {
  await useMockScenario(page, "latest-five-cross-banner", [defaultPrefsKey]);
  await page.setViewportSize({ width: 1130, height: 900 });
  await page.goto("/");

  await selectForkPool(page);
  const wall = latestFiveWall(page);
  const distanceToggle = wall.locator(".latest-distance-toggle");
  const wallToggle = wall.locator(".latest-item-toggle");

  await expectWallGroups(page, [group("60", "mock-cross-fork-new-up")], "actual");
  await expectNoCrossBannerBreakdown(page);

  await distanceToggle.click();
  await expectWallGroups(page, [group("60", "mock-cross-fork-new-up")], "cost");

  await wallToggle.click();
  await expectWallGroups(page, [
    group("20", "mock-cross-fork-new-up"),
    group("40", "mock-cross-fork-old-off-rate"),
  ], "cost");

  await distanceToggle.click();
  await expectWallGroups(page, [
    group("25", "mock-cross-fork-new-up"),
    group("35", "mock-cross-fork-old-off-rate"),
  ], "actual");

  await wallToggle.click();
  await expectWallGroups(page, [group("60", "mock-cross-fork-new-up")], "actual");
  await selectBanner(page, "New Fork Banner");
  await expect(page.locator(".summary-metric-card--total-pulls")).toContainText("20");
  await expectCrossBannerGroup(page, {
    recordId: "mock-cross-fork-new-up",
    distance: "60",
    mode: "actual",
    current: "20",
    other: "40",
  });

  await distanceToggle.click();
  await expectCrossBannerGroup(page, {
    recordId: "mock-cross-fork-new-up",
    distance: "60",
    mode: "cost",
    current: "20",
    other: "40",
  });

  await wallToggle.click();
  await expectWallGroups(page, [group("20", "mock-cross-fork-new-up")], "cost");
  await expectNoCrossBannerBreakdown(page);
  await expectGroupLabelNotToContain(page, "+");

  await distanceToggle.click();
  await expectCrossBannerGroup(page, {
    recordId: "mock-cross-fork-new-up",
    distance: "25",
    mode: "actual",
    current: "20",
    other: "5",
  });

  await page.locator('.pool-strip [data-pool-kind="monopoly_limited"]').click();
  await expectWallGroups(page, [group("60", "mock-cross-character-new-up")], "actual");
  await expectNoCrossBannerBreakdown(page);
  await expect(wall.locator(".latest-distance-toggle")).toHaveCount(0);

  await selectBanner(page, "New Character Banner");
  await expect(page.locator(".summary-metric-card--total-pulls")).toContainText("20");
  await expectCrossBannerGroup(page, {
    recordId: "mock-cross-character-new-up",
    distance: "60",
    mode: "actual",
    current: "20",
    other: "40",
  });
});

type ExpectedWallGroup = {
  distance: string;
  recordIds: string[];
};

function group(distance: string, ...recordIds: string[]): ExpectedWallGroup {
  return { distance, recordIds };
}

async function selectForkPool(page: Page) {
  await page.locator('.pool-strip [data-pool-kind="fork_lottery"]').click();
  await expect(latestFiveWall(page).locator(".latest-distance-toggle")).toBeVisible();
}

async function selectBanner(page: Page, name: string) {
  await page.locator('[data-agent-id="dashboard-banner-rail"]').getByRole("button", { name }).click();
  await expect(page.locator('[data-agent-id="dashboard-banner-rail"]').getByRole("button", { name })).toHaveAttribute("aria-pressed", "true");
  await expect(latestFiveWall(page).locator(".five-wall-item")).toBeVisible();
}

async function useMockScenario(page: Page, scenario: "latest-five-cost-distance" | "latest-five-cross-banner", prefsKeys: string[]) {
  await page.addInitScript(
    ({ mockScenarioKey, scenario, prefsKeys }) => {
      window.localStorage.setItem(mockScenarioKey, scenario);
      const initKey = `nte.e2eScenarioInit:${scenario}`;
      if (window.sessionStorage.getItem(initKey)) return;
      for (const key of prefsKeys) window.localStorage.removeItem(key);
      window.sessionStorage.setItem(initKey, "1");
    },
    { mockScenarioKey, scenario, prefsKeys },
  );
}

async function createProfile(page: Page, profileName: string) {
  await page.locator('[data-agent-id="profile-create-open"]').click();
  await page.locator('[data-agent-id="profile-create-input"]').fill(profileName);
  await page.locator('[data-agent-id="profile-create-submit"]').click();
  await expect(page.locator(`[data-agent-id="profile-row-${profileName}"]`)).toHaveClass(/active/);
}

async function expectWallGroups(page: Page, expected: ExpectedWallGroup[], mode: "actual" | "cost") {
  await expect(latestFiveWall(page).locator('.five-wall-item[data-five-wall-group-anchor="true"]')).toHaveCount(expected.length);
  await expect(latestFiveWall(page).locator(".five-wall-item")).toHaveCount(expected.reduce((sum, item) => sum + item.recordIds.length, 0));
  await expect.poll(() => wallGroupAttrs(page)).toEqual(
    expected.map((item) => ({
      distance: item.distance,
      mode,
      hitCount: String(item.recordIds.length),
      recordIds: item.recordIds,
      tileRecordIds: item.recordIds,
      badgeCount: 1,
      badge: item.distance,
      titleIncludesDistance: true,
      ariaIncludesDistance: true,
    })),
  );
}

async function expectWallDistanceSum(page: Page, expected: number) {
  await expect.poll(async () => (await wallGroupAttrs(page)).reduce((sum, item) => sum + Number(item.distance), 0)).toBe(expected);
}

async function expectCrossBannerGroup(
  page: Page,
  expected: { recordId: string; distance: string; mode: "actual" | "cost"; current: string; other: string },
) {
  await expectWallGroups(page, [group(expected.distance, expected.recordId)], expected.mode);
  const anchor = latestFiveWall(page).locator('.five-wall-item[data-five-wall-group-anchor="true"]');
  await expect(anchor).toHaveAttribute("data-five-wall-current-banner-pulls", expected.current);
  await expect(anchor).toHaveAttribute("data-five-wall-other-banner-pulls", expected.other);
  const breakdownPattern = new RegExp(`${expected.distance}.*${expected.current}.*\\+.*${expected.other}`);
  await expect(anchor).toHaveAttribute("title", breakdownPattern);
  await expect(anchor).toHaveAttribute("aria-label", breakdownPattern);
}

async function expectNoCrossBannerBreakdown(page: Page) {
  const anchors = latestFiveWall(page).locator('.five-wall-item[data-five-wall-group-anchor="true"]');
  await expect.poll(() => anchors.evaluateAll((items) => items.every((item) =>
    !item.hasAttribute("data-five-wall-current-banner-pulls")
    && !item.hasAttribute("data-five-wall-other-banner-pulls"),
  ))).toBe(true);
}

async function expectGroupLabelNotToContain(page: Page, text: string) {
  const anchor = latestFiveWall(page).locator('.five-wall-item[data-five-wall-group-anchor="true"]');
  await expect(anchor).not.toHaveAttribute("title", new RegExp(escapeRegExp(text)));
  await expect(anchor).not.toHaveAttribute("aria-label", new RegExp(escapeRegExp(text)));
}

async function expectGroupTooltipUniform(page: Page, groupKey: string) {
  const titles = await latestFiveWall(page).locator(`.five-wall-item[data-five-wall-group-key="${groupKey}"]`).evaluateAll((items) =>
    items.map((item) => ({ title: item.getAttribute("title"), aria: item.getAttribute("aria-label") })),
  );
  expect(new Set(titles.map((item) => item.title)).size).toBe(1);
  expect(new Set(titles.map((item) => item.aria)).size).toBe(1);
  expect(titles[0].title).toContain("Rose, Off-rate Arc E, Off-rate Arc F, Off-rate Arc G, Rose");
}

async function expectGroupCountBadge(page: Page, groupKey: string, expected: string) {
  const badges = latestFiveWall(page).locator(".five-wall-group-count");
  await expect.poll(async () => badges.evaluateAll((items, key) =>
    items.filter((item) => item.getAttribute("data-five-wall-group-key") === key).map((item) => item.textContent?.trim()), groupKey,
  )).toEqual([expected]);
}

async function expectDistancePlacement(page: Page, groupKey: string, placement: "proxy" | "terminal") {
  await expect.poll(async () => latestFiveWall(page).locator(".five-wall-pity").evaluateAll((items, key) =>
    items.filter((item) => item.getAttribute("data-five-wall-group-key") === key).map((item) => item.getAttribute("data-five-wall-distance-placement")), groupKey,
  )).toEqual([placement]);
}

async function expectUniformTileGeometry(page: Page) {
  const boxes = await latestFiveWall(page).locator(".five-wall-item").evaluateAll((items) =>
    items.map((item) => {
      const rect = item.getBoundingClientRect();
      return { width: rect.width, height: rect.height };
    }),
  );
  expect(boxes.length).toBeGreaterThan(1);
  for (const box of boxes.slice(1)) {
    expect(box.width).toBeCloseTo(boxes[0].width, 1);
    expect(box.height).toBeCloseTo(boxes[0].height, 1);
  }
}

async function expectTenPixelGridGaps(page: Page) {
  const gaps = await latestFiveWall(page).locator(".five-wall-item").evaluateAll((items) => {
    const rows = new Map<number, DOMRect[]>();
    for (const item of items) {
      const rect = item.getBoundingClientRect();
      const row = [...rows.keys()].find((top) => Math.abs(top - rect.top) <= 1) ?? rect.top;
      rows.set(row, [...(rows.get(row) ?? []), rect]);
    }
    return [...rows.values()].flatMap((row) => row.sort((left, right) => left.left - right.left).slice(1).map((rect, index) => rect.left - row[index].right));
  });
  expect(gaps.length).toBeGreaterThan(0);
  for (const gap of gaps) expect(gap).toBeCloseTo(10, 1);
}

function latestFiveWall(page: Page) {
  return page.locator('[data-agent-id="dashboard-latest-five-wall"]');
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

async function wallGroupAttrs(page: Page) {
  return latestFiveWall(page).locator('.five-wall-item[data-five-wall-group-anchor="true"]').evaluateAll((anchors) =>
    anchors.map((anchor) => {
      const grid = anchor.parentElement!;
      const key = anchor.getAttribute("data-five-wall-group-key") ?? "";
      const distance = anchor.getAttribute("data-five-wall-group-distance") ?? "";
      const groupElements = [...grid.querySelectorAll<HTMLElement>("[data-five-wall-group-key]")]
        .filter((item) => item.getAttribute("data-five-wall-group-key") === key);
      const badges = groupElements.filter((item) => item.classList.contains("five-wall-pity"));
      return {
        distance,
        mode: anchor.getAttribute("data-five-wall-group-distance-mode") ?? "",
        hitCount: anchor.getAttribute("data-five-wall-group-hit-count") ?? "",
        recordIds: (anchor.getAttribute("data-five-wall-group-record-ids") ?? "").split(" ").filter(Boolean),
        tileRecordIds: groupElements.filter((item) => item.classList.contains("five-wall-item")).map((item) => item.getAttribute("data-record-id")),
        badgeCount: badges.length,
        badge: badges[0]?.textContent?.trim() ?? "",
        titleIncludesDistance: anchor.getAttribute("title")?.includes(distance) ?? false,
        ariaIncludesDistance: anchor.getAttribute("aria-label")?.includes(distance) ?? false,
      };
    }),
  );
}

async function wallGeometry(page: Page, groupKey: string) {
  return latestFiveWall(page).locator(".five-wall-grid").evaluate((grid, key) => {
    const gridRect = grid.getBoundingClientRect();
    const shell = grid.parentElement!;
    const allTiles = [...grid.querySelectorAll<HTMLElement>(".five-wall-item")].map((item) => {
      const rect = item.getBoundingClientRect();
      return {
        recordId: item.dataset.recordId ?? "",
        groupKey: item.dataset.fiveWallGroupKey ?? "",
        left: rect.left - gridRect.left,
        top: rect.top - gridRect.top,
        right: rect.right - gridRect.left,
        bottom: rect.bottom - gridRect.top,
        centerX: rect.left - gridRect.left + rect.width / 2,
        centerY: rect.top - gridRect.top + rect.height / 2,
        width: rect.width,
        height: rect.height,
      };
    });
    const groupTiles = allTiles.filter((tile) => tile.groupKey === key);
    const frame = [...grid.querySelectorAll<SVGGElement>(".five-wall-group-frame")]
      .find((item) => item.getAttribute("data-five-wall-group-key") === key)!;
    const segments = [...frame.querySelectorAll<SVGRectElement>(".five-wall-frame-segment")].map((segment) => {
      const left = Number(segment.getAttribute("x"));
      const top = Number(segment.getAttribute("y"));
      const style = getComputedStyle(segment);
      return {
        left,
        top,
        right: left + Number(segment.getAttribute("width")),
        bottom: top + Number(segment.getAttribute("height")),
        rx: segment.getAttribute("rx"),
        fill: style.fill,
        stroke: style.stroke,
      };
    });
    const distanceBadge = [...grid.querySelectorAll<HTMLElement>(".five-wall-pity")]
      .find((item) => item.dataset.fiveWallGroupKey === key)!;
    const countBadge = [...grid.querySelectorAll<HTMLElement>(".five-wall-group-count")]
      .find((item) => item.dataset.fiveWallGroupKey === key)!;
    const badgeRect = distanceBadge.getBoundingClientRect();
    const countBadgeRect = countBadge.getBoundingClientRect();
    const firstTop = Math.min(...allTiles.map((tile) => tile.top));
    const firstRowTiles = allTiles
      .filter((tile) => Math.abs(tile.top - firstTop) <= 1)
      .map((tile) => ({ recordId: tile.recordId, left: tile.left, top: tile.top, width: tile.width, height: tile.height }));
    const unrelatedCentersInsideSegments = allTiles
      .filter((tile) => tile.groupKey !== key)
      .filter((tile) => segments.some((segment) => tile.centerX >= segment.left && tile.centerX <= segment.right && tile.centerY >= segment.top && tile.centerY <= segment.bottom))
      .map((tile) => tile.recordId);
    const frameLayer = grid.querySelector<SVGElement>(".five-wall-frame-layer")!;

    return {
      gridLeft: gridRect.left,
      gridTop: gridRect.top,
      gridWidth: gridRect.width,
      shellHeight: shell.getBoundingClientRect().height,
      tileWidth: allTiles[0].width,
      tileHeight: allTiles[0].height,
      firstColumnLeft: Math.min(...allTiles.map((tile) => tile.left)),
      firstRowTiles,
      groupTiles,
      segments,
      paletteIndex: frame.getAttribute("data-five-wall-palette-index"),
      unrelatedCentersInsideSegments,
      framePointerEvents: getComputedStyle(frameLayer).pointerEvents,
      frameZ: Number(getComputedStyle(frameLayer).zIndex),
      maskZ: Number(getComputedStyle(shell, "::after").zIndex),
      countBadge: { right: countBadgeRect.right, top: countBadgeRect.top },
      distanceBadge: { right: badgeRect.right, bottom: badgeRect.bottom },
    };
  }, groupKey);
}
