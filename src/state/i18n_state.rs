//! I18nState - Internationalization State

use crate::i18n::Locale;

/// State for internationalization
#[derive(Debug, Clone)]
pub struct I18nState {
    /// Current locale
    pub locale: Locale,
}

impl Default for I18nState {
    fn default() -> Self {
        Self {
            locale: Locale::ZhCN,
        }
    }
}

impl I18nState {
    /// Set the locale
    pub fn set_locale(&mut self, locale: Locale) {
        self.locale = locale;
    }

    /// Toggle between Chinese and English
    pub fn toggle_locale(&mut self) {
        self.locale = match self.locale {
            Locale::ZhCN => Locale::EnUS,
            Locale::EnUS => Locale::ZhCN,
        };
    }
}
