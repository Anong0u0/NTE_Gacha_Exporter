#[derive(Debug, Clone, Copy)]
struct StatusEvent<'a> {
    message: &'a str,
    kind: &'a str,
    step: Option<&'a str>,
    pool: Option<&'a str>,
    current_page: Option<u32>,
    total_pages: Option<u32>,
    technical_detail: &'a str,
    replaceable: bool,
}

impl<'a> StatusEvent<'a> {
    fn new(message: &'a str, kind: &'a str) -> Self {
        Self {
            message,
            kind,
            step: None,
            pool: None,
            current_page: None,
            total_pages: None,
            technical_detail: "",
            replaceable: true,
        }
    }

    fn step(mut self, step: &'a str) -> Self {
        self.step = Some(step);
        self
    }

    fn pool(mut self, pool: &'a str) -> Self {
        self.pool = Some(pool);
        self
    }

    fn page(mut self, current_page: u32, total_pages: u32) -> Self {
        self.current_page = Some(current_page);
        self.total_pages = Some(total_pages);
        self
    }

    fn technical_detail(mut self, technical_detail: &'a str) -> Self {
        self.technical_detail = technical_detail;
        self
    }

    fn persistent(mut self) -> Self {
        self.replaceable = false;
        self
    }

    fn to_status(self, elapsed_seconds: f64, labels: &BTreeMap<String, String>) -> AutoPageStatus {
        AutoPageStatus {
            elapsed_seconds,
            message: localized_status_message(&self, labels),
            kind: self.kind.to_string(),
            step: self.step.map(str::to_string),
            pool: self.pool.map(str::to_string),
            current_page: self.current_page,
            total_pages: self.total_pages,
            technical_detail: self.technical_detail.to_string(),
            replaceable: self.replaceable,
        }
    }
}

const AUTO_PAGE_STEP_LABEL_KEYS: &[(&str, &str)] = &[
    ("limited", "BPUI_LotteryDiceRecord_xiandingqipan"),
    ("limitedBoard", "BPUI_LotteryDiceRecord_xiandingqipan"),
    ("limitedBoardPages", "BPUI_LotteryDiceRecord_xiandingqipan"),
    ("standard", "BPUI_LotteryDiceRecord_biaozhunqipan"),
    ("standardBoard", "BPUI_LotteryDiceRecord_biaozhunqipan"),
    ("standardBoardPages", "BPUI_LotteryDiceRecord_biaozhunqipan"),
    ("boardType", "BPUI_LotteryDiceRecord_qipanleixing"),
    ("fork", "UW_LotteryBase_BP_Hupanyanmu"),
    ("arcShop", "UW_LotteryBase_BP_Hupanyanmu"),
    ("arcResearch", "ui_forkshop_03"),
    ("arcResearchDetails", "ui_forkshop_07"),
    ("arcResearchRecords", "ui_forkshop_10"),
    ("arcResearchPages", "ui_forkshop_10"),
];

fn localized_status_message(event: &StatusEvent<'_>, labels: &BTreeMap<String, String>) -> String {
    event
        .step
        .and_then(|step| auto_page_label(step, labels))
        .or_else(|| event.pool.and_then(|pool| auto_page_label(pool, labels)))
        .unwrap_or(event.message)
        .to_string()
}

fn auto_page_label<'a>(key: &str, labels: &'a BTreeMap<String, String>) -> Option<&'a str> {
    AUTO_PAGE_STEP_LABEL_KEYS
        .iter()
        .find_map(|(step, label_key)| (*step == key).then_some(*label_key))
        .and_then(|label_key| labels.get(label_key))
        .map(String::as_str)
        .filter(|value| !value.is_empty())
}

fn required<'a>(value: Option<&'a str>, name: &str) -> AutomationResult<&'a str> {
    value.ok_or_else(|| AutomationError::message(format!("workflow step missing {name}")))
}

fn sleep_until(deadline: Instant) {
    let now = Instant::now();
    if deadline > now {
        thread::sleep(deadline - now);
    }
}

fn consecutive_known_record_count(
    records: &[RecordSnapshot],
    known_counts: &BTreeMap<String, u64>,
) -> usize {
    let mut remaining = known_counts.clone();
    records
        .iter()
        .rev()
        .take_while(|record| {
            let Some(count) = remaining.get_mut(record.record_key.as_str()) else {
                return false;
            };
            if *count == 0 {
                return false;
            }
            *count -= 1;
            true
        })
        .count()
}

fn record_key_counts(keys: &[String]) -> BTreeMap<String, u64> {
    let mut counts = BTreeMap::new();
    for key in keys {
        *counts.entry(key.clone()).or_default() += 1;
    }
    counts
}

fn status_text(status: &AutoPageStatus) -> String {
    let mut text = status.message.clone();
    if let (Some(current), Some(total)) = (status.current_page, status.total_pages) {
        text.push_str(&format!(" page={current}/{total}"));
    }
    if !status.technical_detail.is_empty() {
        text.push_str(": ");
        text.push_str(&status.technical_detail);
    }
    text
}

fn record_pool(record: &RecordSnapshot) -> Option<String> {
    if record.pool_id == "CardPool_Character" {
        return Some("limited".to_string());
    }
    if record.pool_id == "CardPool_NewRole" {
        return Some("standard".to_string());
    }
    if record.record_type == "fork" || record.pool_id.starts_with("ForkLottery_") {
        return Some("fork".to_string());
    }
    None
}
