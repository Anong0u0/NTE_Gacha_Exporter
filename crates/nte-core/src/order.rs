use std::cmp::Ordering;

use crate::{DisplayRecord, InternalRecord};

pub fn compare_records_chronological(left: &InternalRecord, right: &InternalRecord) -> Ordering {
    compare_time_asc(left.time.as_deref(), right.time.as_deref())
        .then_with(|| left.source_order.cmp(&right.source_order))
        .then_with(|| left.record_id.cmp(&right.record_id))
}

pub fn compare_records_for_analysis(left: &InternalRecord, right: &InternalRecord) -> Ordering {
    compare_time_asc(left.time.as_deref(), right.time.as_deref())
        .then_with(|| right.source_order.cmp(&left.source_order))
        .then_with(|| left.record_id.cmp(&right.record_id))
}

pub fn compare_display_chronological(left: &DisplayRecord, right: &DisplayRecord) -> Ordering {
    compare_time_asc(left.time.as_deref(), right.time.as_deref())
        .then_with(|| left.source_order.cmp(&right.source_order))
        .then_with(|| left.record_id.cmp(&right.record_id))
}

pub fn compare_display_newest_first(left: &DisplayRecord, right: &DisplayRecord) -> Ordering {
    compare_time_desc(left.time.as_deref(), right.time.as_deref())
        .then_with(|| left.source_order.cmp(&right.source_order))
        .then_with(|| left.record_id.cmp(&right.record_id))
}

pub fn compare_time_asc(left: Option<&str>, right: Option<&str>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.cmp(right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_time_desc(left: Option<&str>, right: Option<&str>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => right.cmp(left),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}
