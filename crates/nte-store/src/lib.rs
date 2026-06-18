mod store;

pub use store::{DataBackup, JsonStore, load_locale_or_settings};

#[cfg(test)]
mod store_tests;
