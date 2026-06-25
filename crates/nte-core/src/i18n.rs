const UI_LOCALES: &[&str] = &["en", "zh-CN", "zh-Hant"];

pub fn available_ui_locales() -> Vec<String> {
    UI_LOCALES
        .iter()
        .map(|locale| (*locale).to_string())
        .collect()
}

pub fn is_ui_locale(locale: &str) -> bool {
    UI_LOCALES.contains(&locale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_locales_only_include_completed_dictionaries() {
        assert_eq!(available_ui_locales(), vec!["en", "zh-CN", "zh-Hant"]);
        assert!(is_ui_locale("en"));
        assert!(is_ui_locale("zh-CN"));
        assert!(is_ui_locale("zh-Hant"));
    }
}
