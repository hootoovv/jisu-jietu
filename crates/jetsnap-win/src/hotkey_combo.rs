//! 快捷键组合解析（纯逻辑）。
//!
//! 与 `tauri-plugin-global-shortcut` 的字符串解析解耦：
//! 本模块产出结构化 [`KeyCombo`]，由应用壳映射为插件类型注册。
//! 这样解析逻辑（含合法性校验）可以在任意平台单测。

/// 组合键中的主键。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComboKey {
    /// 字母 / 数字（'a'..='z' | '0'..='9'）
    Key(char),
    /// F1..=F12
    FKey(u8),
    PrintScreen,
    Space,
}

/// 一枚完整的组合键。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyCombo {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    pub key: Option<ComboKey>,
}

impl KeyCombo {
    /// 人类可读形式（规范化输出，用于设置界面回显）。
    pub fn display(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.ctrl {
            parts.push("Ctrl".into());
        }
        if self.alt {
            parts.push("Alt".into());
        }
        if self.shift {
            parts.push("Shift".into());
        }
        if self.win {
            parts.push("Win".into());
        }
        if let Some(k) = self.key {
            parts.push(
                match k {
                    ComboKey::Key(c) => c.to_ascii_uppercase().to_string(),
                    ComboKey::FKey(n) => format!("F{n}"),
                    ComboKey::PrintScreen => "PrintScreen".into(),
                    ComboKey::Space => "Space".into(),
                },
            );
        }
        parts.join("+")
    }
}

/// 解析形如 `"Ctrl+PrintScreen"` / `"ctrl+shift+a"` / `"Alt+F5"` 的组合键串。
///
/// 规则：
/// - 修饰键（Ctrl/Alt/Shift/Win）顺序任意，但不可重复；
/// - 必须恰好一个主键，纯修饰键非法；
/// - 主键支持 a-z、0-9、F1-F12、PrintScreen/PrtSc、Space；
/// - 大小写不敏感，允许 `+` 两侧空白。
pub fn parse_combo(input: &str) -> Result<KeyCombo, String> {
    let mut combo = KeyCombo::default();
    let mut key: Option<ComboKey> = None;

    for token in input.split('+') {
        let t = token.trim().to_ascii_lowercase();
        if t.is_empty() {
            return Err(format!("存在空的按键段：\"{token}\""));
        }
        match t.as_str() {
            "ctrl" | "control" => {
                if combo.ctrl {
                    return Err("Ctrl 重复".into());
                }
                combo.ctrl = true;
            }
            "alt" => {
                if combo.alt {
                    return Err("Alt 重复".into());
                }
                combo.alt = true;
            }
            "shift" => {
                if combo.shift {
                    return Err("Shift 重复".into());
                }
                combo.shift = true;
            }
            "win" | "super" | "meta" => {
                if combo.win {
                    return Err("Win 重复".into());
                }
                combo.win = true;
            }
            _ => {
                if key.is_some() {
                    return Err(format!("存在多个主键：\"{input}\""));
                }
                key = Some(parse_key(&t)?);
            }
        }
    }

    combo.key = Some(key.ok_or("缺少主键：纯修饰键不能构成快捷键")?);
    Ok(combo)
}

fn parse_key(t: &str) -> Result<ComboKey, String> {
    if t == "printscreen" || t == "prtsc" || t == "prtscn" {
        return Ok(ComboKey::PrintScreen);
    }
    if t == "space" {
        return Ok(ComboKey::Space);
    }
    if let Some(rest) = t.strip_prefix('f') {
        if let Ok(n) = rest.parse::<u8>() {
            if (1..=12).contains(&n) {
                return Ok(ComboKey::FKey(n));
            }
        }
    }
    let mut chars = t.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if c.is_ascii_alphanumeric() {
            return Ok(ComboKey::Key(c));
        }
    }
    Err(format!("无法识别的按键：\"{t}\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_combo() {
        let c = parse_combo("Ctrl+PrintScreen").unwrap();
        assert!(c.ctrl && !c.alt && !c.shift && !c.win);
        assert_eq!(c.key, Some(ComboKey::PrintScreen));
        assert_eq!(c.display(), "Ctrl+PrintScreen");
    }

    #[test]
    fn parses_case_and_space_insensitively() {
        let a = parse_combo("ctrl+shift+a").unwrap();
        let b = parse_combo(" Shift + CTRL + A ").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.display(), "Ctrl+Shift+A");
    }

    #[test]
    fn parses_fkey_and_digits() {
        assert_eq!(parse_combo("Alt+F5").unwrap().key, Some(ComboKey::FKey(5)));
        assert_eq!(parse_combo("ctrl+7").unwrap().key, Some(ComboKey::Key('7')));
    }

    #[test]
    fn rejects_modifier_only() {
        assert!(parse_combo("ctrl").is_err());
        assert!(parse_combo("Ctrl+Shift").is_err());
        assert!(parse_combo("").is_err());
    }

    #[test]
    fn rejects_multiple_keys() {
        assert!(parse_combo("a+b").is_err());
    }

    #[test]
    fn rejects_duplicates_and_unknown() {
        assert!(parse_combo("ctrl+ctrl+a").is_err());
        assert!(parse_combo("ctrl+foo").is_err());
        assert!(parse_combo("ctrl++a").is_err());
        assert!(parse_combo("ctrl+f13").is_err());
    }

    #[test]
    fn rejects_empty_token_display() {
        // "+a" 之类的输入
        assert!(parse_combo("+a").is_err());
    }
}
