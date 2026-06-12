from __future__ import annotations

from typing import TYPE_CHECKING

from nte_gacha_exporter.app.summary import capture_count_labels
from nte_gacha_exporter.core.schema import LocalizationMap

if TYPE_CHECKING:
    from nte_gacha_exporter.automation.pager import AutoPageStatus

AUTO_PAGE_LABEL_FALLBACKS = {
    "Abyss_GamepadKeys_1": "Switch",
    "AbyssClone_Award_02": "Completed",
    "BPUI_LotteryDiceRecord_biaozhunqipan": "Standard Board",
    "BPUI_LotteryDiceRecord_qipanleixing": "Board Type",
    "BPUI_LotteryDiceRecord_xiandingqipan": "Limited Board",
    "BPUI_LotteryModuleEntrance_Title": "Scarborough Fair",
    "TreasureBox_2": "Open",
    "UI_CloneSystemChallengeFailed_Retry": "Retry",
    "UI_CloneSystemStaminaTips_Enter": "Enter",
    "UI_Lottery_GachaDetails_Zhitoujilu": "Dice Roll Records",
    "UI_Lottery_GachaDetails_title": "Board Details",
    "UW_LotteryBase_BP_Hupanyanmu": "Arc Research",
    "Mall_8_name": "Arc Shop",
    "W_Vehicle_Button_Choose": "Select",
    "W_HTButton_Next_Page": "Next",
    "common_3": "Back",
    "ui_forkshop_03": "Arc Research Program",
    "ui_forkshop_07": "Details",
    "ui_forkshop_10": "Records",
}
AUTO_PAGE_ACTION_LABEL_KEYS = {
    "back": "common_3",
    "completed": "AbyssClone_Award_02",
    "enter": "UI_CloneSystemStaminaTips_Enter",
    "next": "W_HTButton_Next_Page",
    "open": "TreasureBox_2",
    "retry": "UI_CloneSystemChallengeFailed_Retry",
    "select": "W_Vehicle_Button_Choose",
    "switch": "Abyss_GamepadKeys_1",
}
AUTO_PAGE_STEP_LABEL_KEYS = {
    "arcResearch": "UW_LotteryBase_BP_Hupanyanmu",
    "arcResearchDetails": "ui_forkshop_07",
    "arcResearchPages": "UW_LotteryBase_BP_Hupanyanmu",
    "arcResearchRecords": "ui_forkshop_10",
    "arcShop": "Mall_8_name",
    "boardDetails": "UI_Lottery_GachaDetails_title",
    "boardType": "BPUI_LotteryDiceRecord_qipanleixing",
    "diceRecords": "UI_Lottery_GachaDetails_Zhitoujilu",
    "marketHome": "BPUI_LotteryModuleEntrance_Title",
    "standardBoard": "BPUI_LotteryDiceRecord_biaozhunqipan",
    "standardBoardPages": "BPUI_LotteryDiceRecord_biaozhunqipan",
    "verifyDiceRecords": "UI_Lottery_GachaDetails_Zhitoujilu",
    "verifyMarketHome": "BPUI_LotteryModuleEntrance_Title",
    "limitedBoard": "BPUI_LotteryDiceRecord_xiandingqipan",
    "limitedBoardPages": "BPUI_LotteryDiceRecord_xiandingqipan",
}
AUTO_PAGE_STEP_ACTIONS = {
    "arcResearch": "enter",
    "arcResearchDetails": "enter",
    "arcResearchRecords": "enter",
    "arcShop": "enter",
    "boardDetails": "enter",
    "boardType": "open",
    "diceRecords": "switch",
    "limitedBoard": "select",
    "marketHome": "back",
    "standardBoard": "select",
    "verifyDiceRecords": "validate",
    "verifyMarketHome": "validate",
}


def _join_parts(*parts: str) -> str:
    return " ".join(part for part in parts if part)


class AutoPageStatusFormatter:
    def __init__(self, mapping: LocalizationMap) -> None:
        self.mapping = mapping

    def status_line(
        self,
        status: AutoPageStatus,
        *,
        include_elapsed: bool,
        include_detail: bool,
    ) -> str:
        text = self.status_text(status)
        if include_detail and status.technicalDetail:
            text = f"{text}: {status.technicalDetail}"
        if include_elapsed:
            return f"+{status.elapsedSeconds:.2f}s {text}"
        return text

    def tooltip_text(self, status: AutoPageStatus) -> str:
        return self.status_line(status, include_elapsed=False, include_detail=False)

    def status_text(self, status: AutoPageStatus) -> str:
        if status.kind == "started":
            return "Auto page started; keep game visible"
        if status.kind == "completed":
            return "Auto page completed"
        if status.kind == "retry":
            return f"{self.action_label('retry')}: page did not change"
        if status.kind == "diagnostic":
            return status.message

        text = self.step_text(status.step, status.pool) if status.step else status.message
        if status.kind == "template":
            return _join_parts(text, self.action_label("verified"))
        if status.kind == "page":
            text = self.page_text(status)
        if status.kind == "pool_completed":
            text = _join_parts(self.page_target_text(status.step, status.pool), self.action_label("completed"))
        if status.currentPage is not None and status.totalPages is not None:
            return f"{text} page={status.currentPage}/{status.totalPages}"
        return text

    def step_text(self, step: str | None, pool: str | None) -> str:
        if pool and step and step.endswith("Pages"):
            return self.pool_label(pool)
        if step in {"limitedBoard", "standardBoard"}:
            text = self.pool_label("limited" if step == "limitedBoard" else "standard")
        else:
            key = AUTO_PAGE_STEP_LABEL_KEYS.get(step or "")
            text = self.map_label(key) if key else step or ""
        action = AUTO_PAGE_STEP_ACTIONS.get(step or "")
        if not action:
            return text
        return _join_parts(self.action_label(action), text)

    def page_text(self, status: AutoPageStatus) -> str:
        text = self.page_target_text(status.step, status.pool)
        if status.message == "page next":
            return _join_parts(self.action_label("next"), text)
        return text

    def page_target_text(self, step: str | None, pool: str | None) -> str:
        if pool:
            return self.pool_label(pool)
        return self.step_text(step, pool)

    def pool_label(self, pool: str) -> str:
        labels = capture_count_labels(self.mapping)
        if pool == "limited":
            return labels["character"]
        if pool == "standard":
            return labels["standard"]
        if pool == "fork":
            return labels["fork"]
        return pool

    def action_label(self, action: str) -> str:
        key = AUTO_PAGE_ACTION_LABEL_KEYS.get(action)
        if key:
            return self.map_label(key)
        if action == "validate":
            return "Validate"
        if action == "verified":
            return "verified"
        return action

    def map_label(self, key: str) -> str:
        labels = self.mapping.get("labels", {})
        if isinstance(labels, dict):
            text = labels.get(key)
            if text:
                return str(text)
        return AUTO_PAGE_LABEL_FALLBACKS.get(key, key)
