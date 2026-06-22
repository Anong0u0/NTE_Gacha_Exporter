use crate::maps::available_locales;

pub fn available_ui_locales() -> Vec<String> {
    available_locales()
}

pub fn is_ui_locale(locale: &str) -> bool {
    available_ui_locales()
        .iter()
        .any(|available| available == locale)
}
