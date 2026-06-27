use std::{thread, time::Duration};

use anyhow::{Result, bail};
use serde_json::{Value, json};

use crate::api::eval_js;

pub(super) fn audit_dashboard_five_wall_contract(addr: &str) -> Result<Value> {
    eval_js(
        addr,
        r#"
        const fail = (message, detail = {}) => {
          throw new Error(`${message} ${JSON.stringify(detail)}`);
        };
        const round = (value) => Math.round(value * 1000) / 1000;
        const metricsFor = (el) => {
          const rect = el.getBoundingClientRect();
          return {
            width: round(rect.width),
            height: round(rect.height),
            clientHeight: el.clientHeight,
            scrollHeight: el.scrollHeight,
          };
        };
        const fixture = document.createElement("section");
        fixture.className = "panel latest-five-wall";
        fixture.style.cssText = [
          "position: fixed",
          "left: -10000px",
          "top: 0",
          "width: 420px",
          "visibility: hidden",
          "pointer-events: none",
        ].join(";");
        const items = Array.from({ length: 24 }, (_, index) => (
          `<div class="five-wall-item rarity-5"><span class="five-wall-thumb empty">${index}</span><span class="five-wall-pity">${index + 1}</span></div>`
        )).join("");
        fixture.innerHTML = `
          <div class="panel-head"><h2>contract</h2></div>
          <div class="five-wall-shell is-expanded" style="--five-wall-row-height: 74px">
            <div class="five-wall-grid">${items}</div>
            <div class="five-wall-toolbar"><button type="button">toggle</button></div>
          </div>
        `;
        document.body.appendChild(fixture);
        try {
          const shell = fixture.querySelector(".five-wall-shell");
          void shell.offsetHeight;

          const expandedOverflowY = window.getComputedStyle(shell).overflowY;
          const expandedOverflow = shell.scrollHeight - shell.clientHeight;
          if (["auto", "scroll"].includes(expandedOverflowY)) {
            fail("synthetic expanded five wall is configured as inner scroll region", {
              overflowY: expandedOverflowY,
            });
          }
          if (expandedOverflow > 1) {
            fail("synthetic expanded five wall has inner vertical overflow", {
              overflow: round(expandedOverflow),
              shell: metricsFor(shell),
            });
          }

          shell.classList.remove("is-expanded");
          shell.classList.add("is-collapsed");
          void shell.offsetHeight;

          const collapsedOverflowY = window.getComputedStyle(shell).overflowY;
          const collapsedOverflow = shell.scrollHeight - shell.clientHeight;
          if (collapsedOverflowY !== "hidden") {
            fail("synthetic collapsed five wall does not mask preview", {
              overflowY: collapsedOverflowY,
            });
          }
          if (collapsedOverflow <= 1) {
            fail("synthetic collapsed five wall did not create preview overflow", {
              overflow: round(collapsedOverflow),
              shell: metricsFor(shell),
            });
          }

          return {
            expanded: {
              overflow: round(expandedOverflow),
              overflowY: expandedOverflowY,
            },
            collapsed: {
              overflow: round(collapsedOverflow),
              overflowY: collapsedOverflowY,
            },
          };
        } finally {
          fixture.remove();
        }
        "#,
        5000,
    )
}

pub(super) fn ensure_dashboard_five_star_items_visible(addr: &str) -> Result<Value> {
    let result = eval_js(
        addr,
        r#"
        const toggle = document.querySelector(".latest-item-toggle");
        if (!toggle) return { skipped: true, reason: "toggle not visible" };
        const pressedBefore = toggle.getAttribute("aria-pressed") === "true";
        if (!pressedBefore) toggle.click();
        return { skipped: false, pressedBefore, clicked: !pressedBefore };
        "#,
        5000,
    )?;
    thread::sleep(Duration::from_millis(250));
    Ok(result)
}

pub(super) fn audit_dashboard_five_wall_data(addr: &str, expected: &Value) -> Result<Value> {
    let mut audits = serde_json::Map::new();
    let mut pool_counters = Value::Null;
    for pool_kind in ["monopoly_limited", "monopoly_standard", "fork_lottery"] {
        select_dashboard_pool(addr, pool_kind)?;
        ensure_dashboard_five_star_items_visible(addr)?;
        wait_dashboard_five_wall_items(addr, pool_kind, expected)?;
        let audit = audit_selected_dashboard_five_wall_data(addr, pool_kind, expected)?;
        if pool_counters.is_null() {
            pool_counters = audit.get("poolCounters").cloned().unwrap_or(Value::Null);
        }
        audits.insert(pool_kind.to_string(), audit);
    }
    Ok(json!({
        "skipped": false,
        "poolCounters": pool_counters,
        "audits": audits,
    }))
}

fn select_dashboard_pool(addr: &str, pool_kind: &str) -> Result<()> {
    let script = format!(
        r#"
        const poolKind = {pool_kind_json};
        const button = document.querySelector(`.pool-strip button[data-pool-kind="${{poolKind}}"]`);
        if (!button) {{
          throw new Error(`dashboard pool button missing ${{poolKind}}`);
        }}
        if (button.getAttribute("aria-pressed") !== "true") button.click();
        return true;
        "#,
        pool_kind_json = serde_json::to_string(pool_kind)?,
    );
    eval_js(addr, &script, 5000)?;
    for _ in 0..20 {
        thread::sleep(Duration::from_millis(100));
        let ready = eval_js(
            addr,
            r#"
            const selectedPoolKind = document.querySelector(".pool-strip button[aria-pressed='true']")?.dataset.poolKind ?? null;
            return { selectedPoolKind };
            "#,
            5000,
        )?;
        let selected = ready
            .get("selectedPoolKind")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if selected == pool_kind {
            return Ok(());
        }
    }
    bail!("dashboard pool did not become ready: {pool_kind}")
}

fn wait_dashboard_five_wall_items(addr: &str, pool_kind: &str, expected: &Value) -> Result<()> {
    let expected_pool_kind_json = serde_json::to_string(pool_kind)?;
    let expected_json = serde_json::to_string(expected)?;
    let script = r#"
        const expectedSmoke = __EXPECTED_SMOKE__;
        const expectedPoolKind = __EXPECTED_POOL_KIND__;
        const expectedItems = expectedSmoke.expectedByPool?.[expectedPoolKind] ?? [];
        const selectedPoolKind = document.querySelector(".pool-strip button[aria-pressed='true']")?.dataset.poolKind ?? null;
        const togglePressed = document.querySelector(".latest-item-toggle")?.getAttribute("aria-pressed") === "true";
        const actualItems = [...document.querySelectorAll(".latest-five-wall .five-wall-grid .five-wall-item")]
          .map((item) => ({
            recordId: item.dataset.recordId ?? "",
            poolKind: item.dataset.poolKind ?? "",
            fiveWallDistance: item.dataset.fiveWallDistance ?? "",
          }));
        return {
          ready:
            selectedPoolKind === expectedPoolKind
            && togglePressed
            && JSON.stringify(actualItems.map((item) => ({
              recordId: item.recordId,
              fiveWallDistance: item.fiveWallDistance,
            }))) === JSON.stringify(expectedItems),
          selectedPoolKind,
          togglePressed,
          actualItems,
          expectedItems,
        };
        "#
    .replace("__EXPECTED_SMOKE__", &expected_json)
    .replace("__EXPECTED_POOL_KIND__", &expected_pool_kind_json);

    let mut last_state = Value::Null;
    for _ in 0..20 {
        thread::sleep(Duration::from_millis(100));
        last_state = eval_js(addr, &script, 5000)?;
        if last_state
            .get("ready")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Ok(());
        }
    }
    bail!("dashboard five wall did not render expected seeded records: {last_state}")
}

fn audit_selected_dashboard_five_wall_data(
    addr: &str,
    expected_pool_kind: &str,
    expected: &Value,
) -> Result<Value> {
    let expected_pool_kind_json = serde_json::to_string(expected_pool_kind)?;
    let expected_json = serde_json::to_string(expected)?;
    let script = r#"
        const fail = (message, detail = {}) => {
          throw new Error(`${message} ${JSON.stringify(detail)}`);
        };
        const expectedSmoke = __EXPECTED_SMOKE__;
        const expectedPoolKind = __EXPECTED_POOL_KIND__;
        const readSelectedPoolKind = () => document.querySelector(".pool-strip button[aria-pressed='true']")?.dataset.poolKind ?? null;
        const readItems = () => [...document.querySelectorAll(".latest-five-wall .five-wall-grid .five-wall-item")]
          .map((el, index) => ({
            index,
            recordId: el.dataset.recordId ?? "",
            sourceOrder: Number(el.dataset.sourceOrder),
            time: el.dataset.time || null,
            poolKind: el.dataset.poolKind ?? "",
            poolId: el.dataset.poolId ?? "",
            itemId: el.dataset.itemId ?? "",
            rarity: el.dataset.rarity ?? "",
            fiveWallDistance: el.dataset.fiveWallDistance ?? "",
          }));

        const expectedPoolCounters = expectedSmoke.poolCounters ?? {};
        const poolCounters = [...document.querySelectorAll(".pool-strip button[data-pool-kind]")]
          .reduce((acc, button) => {
            acc[button.dataset.poolKind] = {
              currentPity: button.dataset.currentPity ?? "",
              hardPity: button.dataset.hardPity ?? "",
              currentTenPullProgress: button.dataset.currentTenPullProgress ?? "",
            };
            return acc;
          }, {});
        const counterMismatches = Object.entries(expectedPoolCounters)
          .filter(([poolKind, expected]) => JSON.stringify(poolCounters[poolKind] ?? null) !== JSON.stringify(expected))
          .map(([poolKind, expected]) => ({ poolKind, actual: poolCounters[poolKind] ?? null, expected }));
        if (counterMismatches.length) {
          fail("dashboard seeded pity counters mismatch", { counters: counterMismatches });
        }

        const expectedByPool = expectedSmoke.expectedByPool ?? {};

        const selectedPoolKind = readSelectedPoolKind();
        if (selectedPoolKind !== expectedPoolKind) {
          fail("dashboard seeded pool kind mismatch", { selectedPoolKind, expected: expectedPoolKind });
        }
        const items = readItems();
        if (!items.length) {
          fail("dashboard five wall has no records", { selectedPoolKind });
        }

        const duplicateRecordIds = [...items.reduce((counts, item) => {
          counts.set(item.recordId, (counts.get(item.recordId) ?? 0) + 1);
          return counts;
        }, new Map()).entries()].filter(([, count]) => count > 1);
        if (duplicateRecordIds.length) {
          fail("dashboard five wall has duplicate record ids", {
            poolKind: expectedPoolKind,
            duplicates: duplicateRecordIds.map(([recordId, count]) => ({ recordId, count })),
          });
        }

        const badPoolItems = items.filter((item) => {
          if (item.poolKind !== selectedPoolKind) return true;
          if (item.poolKind === "monopoly_limited") return item.poolId !== "CardPool_Character";
          if (item.poolKind === "monopoly_standard") return item.poolId !== "CardPool_NewRole";
          if (item.poolKind === "fork_lottery") return !item.poolId.startsWith("ForkLottery_");
          return true;
        });
        if (badPoolItems.length) {
          fail("dashboard five wall item pool mismatch", {
            selectedPoolKind,
            items: badPoolItems.slice(0, 8),
          });
        }

        const invalidSourceOrder = items.filter((item) => !Number.isSafeInteger(item.sourceOrder));
        if (invalidSourceOrder.length) {
          fail("dashboard five wall item source_order missing or invalid", {
            poolKind: expectedPoolKind,
            items: invalidSourceOrder.slice(0, 8),
          });
        }

        const expected = [...items].sort((left, right) => {
          if (left.time !== null && right.time === null) return -1;
          if (left.time === null && right.time !== null) return 1;
          const timeOrder = String(right.time ?? "").localeCompare(String(left.time ?? ""));
          return timeOrder || right.sourceOrder - left.sourceOrder || left.recordId.localeCompare(right.recordId);
        });
        const mismatches = items
          .map((item, index) => ({ index, actual: item, expected: expected[index] }))
          .filter((entry) => entry.actual.recordId !== entry.expected.recordId);
        if (mismatches.length) {
          fail("dashboard five wall is not newest-first", {
            poolKind: expectedPoolKind,
            actual: items.slice(0, 12).map((item) => ({
              recordId: item.recordId,
              time: item.time,
              sourceOrder: item.sourceOrder,
              poolKind: item.poolKind,
              poolId: item.poolId,
              itemId: item.itemId,
            })),
            expected: expected.slice(0, 12).map((item) => ({
              recordId: item.recordId,
              time: item.time,
              sourceOrder: item.sourceOrder,
              poolKind: item.poolKind,
              poolId: item.poolId,
              itemId: item.itemId,
            })),
          });
        }

        const phantomItems = items.filter((item) => item.rarity !== "5");
        if (phantomItems.length) {
          fail("dashboard five wall contains non-5-star item", {
            poolKind: expectedPoolKind,
            items: phantomItems.slice(0, 8),
          });
        }

        const expectedSeedItems = expectedByPool[expectedPoolKind];
        const actualSeedItems = items.map((item) => ({
          recordId: item.recordId,
          fiveWallDistance: item.fiveWallDistance,
        }));
        if (JSON.stringify(actualSeedItems) !== JSON.stringify(expectedSeedItems)) {
          fail("dashboard five wall seeded records mismatch", {
            poolKind: expectedPoolKind,
            actual: actualSeedItems,
            expected: expectedSeedItems,
          });
        }

        return {
          selectedPoolKind,
          poolCounters,
          count: items.length,
          first: {
            recordId: items[0].recordId,
            time: items[0].time,
            sourceOrder: items[0].sourceOrder,
            poolKind: items[0].poolKind,
            poolId: items[0].poolId,
            itemId: items[0].itemId,
            fiveWallDistance: items[0].fiveWallDistance,
          },
          last: {
            recordId: items[items.length - 1].recordId,
            time: items[items.length - 1].time,
            sourceOrder: items[items.length - 1].sourceOrder,
            poolKind: items[items.length - 1].poolKind,
            poolId: items[items.length - 1].poolId,
            itemId: items[items.length - 1].itemId,
            fiveWallDistance: items[items.length - 1].fiveWallDistance,
          },
        };
        "#
    .replace("__EXPECTED_SMOKE__", &expected_json)
    .replace("__EXPECTED_POOL_KIND__", &expected_pool_kind_json);
    eval_js(addr, &script, 5000)
}

pub(super) fn audit_dashboard_scroll_regions(addr: &str, phase: &str) -> Result<Value> {
    let script = format!(
        "const phase = {};\n{}",
        serde_json::to_string(phase)?,
        r#"
        const fail = (message, detail = {}) => {
          throw new Error(`${message} ${JSON.stringify(detail)}`);
        };
        const round = (value) => Math.round(value * 1000) / 1000;
        const metricsFor = (el) => {
          if (!el) return null;
          const rect = el.getBoundingClientRect();
          return {
            left: round(rect.left),
            top: round(rect.top),
            right: round(rect.right),
            bottom: round(rect.bottom),
            width: round(rect.width),
            height: round(rect.height),
            clientWidth: el.clientWidth,
            scrollWidth: el.scrollWidth,
            clientHeight: el.clientHeight,
            scrollHeight: el.scrollHeight,
          };
        };

        const workspace = document.querySelector(".workspace");
        const detailBody = document.querySelector(".selected-detail-body");
        const detailPanel = document.querySelector(".selected-detail-panel");
        const fiveWallShell = document.querySelector(".five-wall-shell");
        if (!workspace || !detailBody || !detailPanel || !fiveWallShell) {
          fail("dashboard scroll region missing", {
            phase,
            hasWorkspace: Boolean(workspace),
            hasDetailBody: Boolean(detailBody),
            hasDetailPanel: Boolean(detailPanel),
            hasFiveWallShell: Boolean(fiveWallShell),
          });
        }

        const detailBodyStyle = window.getComputedStyle(detailBody);
        const fiveWallStyle = window.getComputedStyle(fiveWallShell);
        const detailBodyOverflow = detailBody.scrollHeight - detailBody.clientHeight;
        const detailPanelOverflow = detailPanel.scrollHeight - detailPanel.clientHeight;
        const fiveWallOverflow = fiveWallShell.scrollHeight - fiveWallShell.clientHeight;
        const fiveWallClasses = [...fiveWallShell.classList];
        const fiveWallCollapsed = fiveWallShell.classList.contains("is-collapsed");

        if (detailBodyOverflow > 1) {
          fail("dashboard detail body has inner vertical overflow", {
            phase,
            detailBodyOverflow: round(detailBodyOverflow),
            detailBody: metricsFor(detailBody),
          });
        }
        if (["auto", "scroll"].includes(detailBodyStyle.overflowY)) {
          fail("dashboard detail body is configured as inner scroll region", {
            phase,
            overflowY: detailBodyStyle.overflowY,
          });
        }
        if (["auto", "scroll"].includes(fiveWallStyle.overflowY)) {
          fail("dashboard five wall shell is configured as inner scroll region", {
            phase,
            overflowY: fiveWallStyle.overflowY,
            classes: fiveWallClasses,
          });
        }
        if (fiveWallCollapsed && fiveWallStyle.overflowY !== "hidden") {
          fail("dashboard collapsed five wall shell does not mask preview", {
            phase,
            overflowY: fiveWallStyle.overflowY,
            classes: fiveWallClasses,
          });
        }
        if (!fiveWallCollapsed && fiveWallOverflow > 1) {
          fail("dashboard five wall shell has inner vertical overflow", {
            phase,
            fiveWallOverflow: round(fiveWallOverflow),
            classes: fiveWallClasses,
            fiveWallShell: metricsFor(fiveWallShell),
          });
        }

        return {
          phase,
          workspace: {
            ...metricsFor(workspace),
            verticalOverflow: round(workspace.scrollHeight - workspace.clientHeight),
            overflowY: window.getComputedStyle(workspace).overflowY,
          },
          dashboardDetail: {
            body: metricsFor(detailBody),
            panel: metricsFor(detailPanel),
            bodyOverflow: round(detailBodyOverflow),
            panelOverflow: round(detailPanelOverflow),
            bodyOverflowY: detailBodyStyle.overflowY,
          },
          fiveWall: {
            shell: metricsFor(fiveWallShell),
            overflow: round(fiveWallOverflow),
            overflowY: fiveWallStyle.overflowY,
            classes: fiveWallClasses,
          },
        };
        "#,
    );
    eval_js(addr, &script, 5000)
}

pub(super) fn audit_dashboard_expanded_layout(addr: &str) -> Result<Value> {
    let interaction = eval_js(
        addr,
        r#"
        const toggle = document.querySelector('[data-agent-id="dashboard-five-wall-toggle"]');
        if (!toggle) {
          return { skipped: true, reason: "five wall toggle not visible" };
        }
        const expandedBefore = toggle.getAttribute("aria-expanded") === "true";
        if (!expandedBefore) toggle.click();
        return { skipped: false, expandedBefore, clicked: !expandedBefore };
        "#,
        5000,
    )?;
    thread::sleep(Duration::from_millis(250));

    let skipped = interaction
        .get("skipped")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let expanded_state = eval_js(
        addr,
        r#"
        const toggle = document.querySelector('[data-agent-id="dashboard-five-wall-toggle"]');
        const shell = document.querySelector(".five-wall-shell");
        return {
          hasToggle: Boolean(toggle),
          ariaExpanded: toggle?.getAttribute("aria-expanded") ?? null,
          shellClasses: shell ? [...shell.classList] : [],
          shellExpanded: shell?.classList.contains("is-expanded") ?? false,
        };
        "#,
        5000,
    )?;
    if !skipped
        && !expanded_state
            .get("shellExpanded")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        bail!("dashboard five wall did not enter expanded state: {expanded_state}");
    }

    let regions = audit_dashboard_scroll_regions(
        addr,
        if skipped {
            "expanded-skipped"
        } else {
            "expanded"
        },
    )?;
    let restore = if skipped {
        json!({ "skipped": true })
    } else {
        eval_js(
            addr,
            r#"
            const toggle = document.querySelector('[data-agent-id="dashboard-five-wall-toggle"]');
            if (toggle?.getAttribute("aria-expanded") === "true") toggle.click();
            return { clicked: Boolean(toggle), ariaExpanded: toggle?.getAttribute("aria-expanded") ?? null };
            "#,
            5000,
        )?
    };
    thread::sleep(Duration::from_millis(100));

    Ok(json!({
        "interaction": interaction,
        "expandedState": expanded_state,
        "regions": regions,
        "restore": restore,
    }))
}
