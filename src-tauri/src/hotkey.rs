//! 热键服务（开发计划 5.1.1）：解析 / 注册 / 重绑 / 回滚。

use jetsnap_win::hotkey_combo::{parse_combo, ComboKey, KeyCombo};
use tauri::AppHandle;
use tauri::Manager;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

use crate::state::AppState;

/// KeyCombo → 插件 Shortcut（显式映射，避免依赖插件的字符串解析格式）。
pub fn to_shortcut(combo: &KeyCombo) -> Result<Shortcut, String> {
    let mut mods = Modifiers::empty();
    if combo.ctrl {
        mods |= Modifiers::CONTROL;
    }
    if combo.alt {
        mods |= Modifiers::ALT;
    }
    if combo.shift {
        mods |= Modifiers::SHIFT;
    }
    if combo.win {
        mods |= Modifiers::SUPER;
    }
    let code = match combo.key {
        Some(ComboKey::PrintScreen) => Code::PrintScreen,
        Some(ComboKey::Space) => Code::Space,
        Some(ComboKey::FKey(n)) => match n {
            1 => Code::F1,
            2 => Code::F2,
            3 => Code::F3,
            4 => Code::F4,
            5 => Code::F5,
            6 => Code::F6,
            7 => Code::F7,
            8 => Code::F8,
            9 => Code::F9,
            10 => Code::F10,
            11 => Code::F11,
            _ => Code::F12,
        },
        Some(ComboKey::Key(c)) => match c.to_ascii_lowercase() {
            'a' => Code::KeyA,
            'b' => Code::KeyB,
            'c' => Code::KeyC,
            'd' => Code::KeyD,
            'e' => Code::KeyE,
            'f' => Code::KeyF,
            'g' => Code::KeyG,
            'h' => Code::KeyH,
            'i' => Code::KeyI,
            'j' => Code::KeyJ,
            'k' => Code::KeyK,
            'l' => Code::KeyL,
            'm' => Code::KeyM,
            'n' => Code::KeyN,
            'o' => Code::KeyO,
            'p' => Code::KeyP,
            'q' => Code::KeyQ,
            'r' => Code::KeyR,
            's' => Code::KeyS,
            't' => Code::KeyT,
            'u' => Code::KeyU,
            'v' => Code::KeyV,
            'w' => Code::KeyW,
            'x' => Code::KeyX,
            'y' => Code::KeyY,
            'z' => Code::KeyZ,
            '0' => Code::Digit0,
            '1' => Code::Digit1,
            '2' => Code::Digit2,
            '3' => Code::Digit3,
            '4' => Code::Digit4,
            '5' => Code::Digit5,
            '6' => Code::Digit6,
            '7' => Code::Digit7,
            '8' => Code::Digit8,
            _ => Code::Digit9,
        },
        None => return Err("缺少主键".into()),
    };
    Ok(Shortcut::new(Some(mods), code))
}

/// 注册组合键（应用启动 / 重绑共用）。
pub fn register(app: &AppHandle, combo_str: &str) -> Result<(), String> {
    let combo = parse_combo(combo_str)?;
    let shortcut = to_shortcut(&combo)?;
    app.global_shortcut()
        .register(shortcut)
        .map_err(|e| format!("{e}"))
}

/// 重绑：先解析新键 → 注销旧键 → 注册新键；失败回滚旧键并上抛人话错误。
pub fn rebind(app: &AppHandle, new_combo: &str) -> Result<(), String> {
    // 先解析，语法错误直接返回，不动现有热键
    parse_combo(new_combo)?;

    let state = app.state::<AppState>();
    let old_combo = state.settings.lock().unwrap().hotkey.clone();

    let _ = unregister_combo(app, &old_combo); // 旧键可能已失效，忽略错误
    match register(app, new_combo) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = register(app, &old_combo); // 回滚
            Err(format!("注册 {new_combo} 失败：{e}（可能已被其他程序占用）"))
        }
    }
}

fn unregister_combo(app: &AppHandle, combo_str: &str) -> Result<(), String> {
    let combo = parse_combo(combo_str)?;
    let shortcut = to_shortcut(&combo)?;
    app.global_shortcut()
        .unregister(shortcut)
        .map_err(|e| format!("{e}"))
}
