//! i18n - Internationalization Module
//!
//! Provides simple translation functions using HashMap-based lookups.

use std::collections::HashMap;
use std::sync::OnceLock;

use gpui::SharedString;

/// Supported locales
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Locale {
    /// English (US)
    EnUS,
    /// Chinese (Simplified)
    #[default]
    ZhCN,
}

impl Locale {
    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            Locale::EnUS => "English",
            Locale::ZhCN => "中文",
        }
    }
}

/// Translation resources
static TRANSLATIONS: OnceLock<HashMap<&'static str, (&'static str, &'static str)>> = OnceLock::new();

/// Initialize translations (key -> (en, zh))
fn init_translations() -> HashMap<&'static str, (&'static str, &'static str)> {
    let mut map = HashMap::new();

    // App
    map.insert("app-title", ("DFC Communication Simulator", "DFC 通信模拟器"));

    // Navigation
    map.insert("nav-home", ("Home", "首页"));
    map.insert("nav-properties", ("Properties", "属性"));
    map.insert("nav-events", ("Events", "事件"));
    map.insert("nav-commands", ("Commands", "命令"));
    map.insert("nav-curve", ("Power Curve", "功率曲线"));
    map.insert("nav-one-min", ("1-Min Data", "1分钟数据"));
    map.insert("nav-ten-min", ("10-Min Data", "10分钟数据"));

    // Actions
    map.insert("action-start", ("Start", "启动"));
    map.insert("action-stop", ("Stop", "停止"));
    map.insert("action-save", ("Save", "保存"));
    map.insert("action-save-favorite", ("Save Favorite", "保存收藏"));
    map.insert("action-refresh", ("Refresh", "刷新"));
    map.insert("action-send", ("Send", "发送"));
    map.insert("action-clear", ("Clear", "清除"));

    // Home page
    map.insert("home-basic-config", ("Basic Configuration", "基本配置"));
    map.insert("home-device-id", ("Device ID", "设备号"));
    map.insert("home-cfgid", ("Config ID", "配置号"));
    map.insert("home-running", ("Running", "运行中"));
    map.insert("home-redis-config", ("Redis Configuration", "Redis 配置"));
    map.insert("home-pulsar-config", ("Pulsar Configuration", "Pulsar 配置"));
    map.insert("home-ip", ("IP Address", "IP 地址"));
    map.insert("home-port", ("Port", "端口"));
    map.insert("home-password", ("Password", "密码"));
    map.insert("home-pulsar-redis-ip", ("Pulsar Redis IP", "Pulsar Redis IP"));
    map.insert("home-pulsar-redis-port", ("Pulsar Redis Port", "Pulsar Redis 端口"));

    // Table columns
    map.insert("col-device-id", ("Device ID", "设备号"));
    map.insert("col-name", ("Name", "名称"));
    map.insert("col-topic", ("Topic", "主题"));
    map.insert("col-value", ("Value", "值"));
    map.insert("col-quality", ("Quality", "质量"));
    map.insert("col-data-time", ("Data Time", "数据时间"));
    map.insert("col-event-code", ("Event Code", "事件代码"));
    map.insert("col-description", ("Description", "描述"));
    map.insert("col-level", ("Level", "级别"));
    map.insert("col-state", ("State", "状态"));
    map.insert("col-event-time", ("Event Time", "事件时间"));

    // Commands page
    map.insert("commands-title", ("Send Command", "发送命令"));
    map.insert("commands-service", ("Service", "服务"));
    map.insert("commands-method", ("Method", "方法"));
    map.insert("commands-params", ("Parameters", "参数"));
    map.insert("commands-timeout", ("Timeout (s)", "超时 (秒)"));
    map.insert("commands-history", ("History", "历史记录"));

    // Log panel
    map.insert("log-title", ("Logs", "日志"));
    map.insert("log-clear", ("Clear", "清除"));

    // Table
    map.insert("table-no-data", ("No data", "无数据"));
    map.insert("table-loading", ("Loading...", "加载中..."));

    map
}

/// Get translations
fn translations() -> &'static HashMap<&'static str, (&'static str, &'static str)> {
    TRANSLATIONS.get_or_init(init_translations)
}

/// Translate a key
pub fn t(locale: Locale, key: &str) -> SharedString {
    if let Some(&(en, zh)) = translations().get(key) {
        match locale {
            Locale::EnUS => SharedString::from(en),
            Locale::ZhCN => SharedString::from(zh),
        }
    } else {
        // Fallback: return the key itself
        SharedString::from(key.to_string())
    }
}

/// Convenience function for translating with a count argument (placeholder)
pub fn t_count(locale: Locale, key: &str, _count: i64) -> SharedString {
    t(locale, key)
}
