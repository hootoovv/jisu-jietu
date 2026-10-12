//! 设置持久化：%APPDATA%/JetSnap/settings.json（开发计划 5.1.6）。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub schema_version: u16,
    /// 组合键串，如 "Ctrl+PrintScreen"
    pub hotkey: String,
    pub default_save_dir: Option<String>,
    /// "png" | "jpeg"
    pub last_format: String,
    pub autostart: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            hotkey: "Ctrl+PrintScreen".into(),
            default_save_dir: None,
            last_format: "png".into(),
            autostart: false,
        }
    }
}

pub fn settings_dir() -> PathBuf {
    std::env::var("APPDATA")
        .map(|p| PathBuf::from(p).join("JetSnap"))
        .unwrap_or_else(|_| PathBuf::from(".jetsnap"))
}

pub fn settings_path() -> PathBuf {
    settings_dir().join("settings.json")
}

/// 读取设置；文件缺失或损坏时返回默认值（损坏文件重命名备份，不覆盖）。
pub fn load() -> Settings {
    let path = settings_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Settings::default();
    };
    match serde_json::from_str(&text) {
        Ok(s) => s,
        Err(_) => {
            let backup = path.with_extension("json.bak");
            let _ = std::fs::rename(&path, &backup);
            Settings::default()
        }
    }
}

pub fn save(settings: &Settings) -> Result<(), String> {
    let dir = settings_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建设置目录失败: {e}"))?;
    let text = serde_json::to_string_pretty(settings).map_err(|e| format!("序列化设置失败: {e}"))?;
    std::fs::write(settings_path(), text).map_err(|e| format!("写入设置失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_hotkey_is_ctrl_printscreen() {
        assert_eq!(Settings::default().hotkey, "Ctrl+PrintScreen");
    }

    #[test]
    fn settings_roundtrip_json() {
        let s = Settings::default();
        let text = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&text).unwrap();
        assert_eq!(back.hotkey, s.hotkey);
        assert_eq!(back.schema_version, 1);
    }
}
