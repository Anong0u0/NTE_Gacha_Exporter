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

    fn to_status(self, elapsed_seconds: f64) -> AutoPageStatus {
        AutoPageStatus {
            elapsed_seconds,
            message: self.message.to_string(),
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
