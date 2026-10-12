//! Tauri Command 层（开发计划 6.1 清单）。

use jetsnap_win::PhysRect;
use tauri::{AppHandle, Manager};

use crate::hotkey;
use crate::overlay;
use crate::session::{self, FrozenPayload};
use crate::settings;
use crate::state::AppState;

/// 前端挂载后拉取当前会话（主通道；事件为兜底）。
#[tauri::command]
pub fn fetch_session(app: AppHandle) -> Option<FrozenPayload> {
    let state = app.state::<AppState>();
    let guard = state.session.lock().unwrap();
    guard.as_ref().map(|s| FrozenPayload {
        snapshot_id: s.id,
        virtual_rect: s.virtual_rect,
        monitors: s.monitors.clone(),
        windows: s.windows.clone(),
        hotkey_elapsed_ms: s.hotkey_elapsed_ms,
    })
}

/// 前端加载完 snapshot 图片后显示覆盖窗口（N1 截止点）。
#[tauri::command]
pub fn reveal_overlay(app: AppHandle) -> Result<(), String> {
    overlay::reveal(&app)
}

/// 结束会话：清理资源并销毁覆盖窗口。
#[tauri::command]
pub fn end_session(app: AppHandle, outcome: String) {
    eprintln!("[JetSnap] 会话结束: {outcome}");
    session::finish(&app);
}

/// 手动触发截图（托盘 / 设置窗口测试按钮）。
#[tauri::command]
pub fn trigger_capture(app: AppHandle) {
    session::start_session(&app);
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> settings::Settings {
    app.state::<AppState>().settings.lock().unwrap().clone()
}

/// 重绑热键（失败自动回滚旧键，错误含人话原因）。
#[tauri::command]
pub fn set_hotkey(app: AppHandle, combo: String) -> Result<String, String> {
    hotkey::rebind(&app, &combo)?;
    let state = app.state::<AppState>();
    let mut s = state.settings.lock().unwrap();
    s.hotkey = combo.clone();
    settings::save(&s)?;
    Ok(combo)
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("取可执行文件路径失败: {e}"))?
        .to_string_lossy()
        .to_string();
    jetsnap_win::autostart::set_autostart(enabled, &exe)?;
    let state = app.state::<AppState>();
    let mut s = state.settings.lock().unwrap();
    s.autostart = enabled;
    settings::save(&s)?;
    Ok(())
}

/// 更新保存偏好（默认目录 / 默认格式）。
#[tauri::command]
pub fn update_preferences(
    app: AppHandle,
    default_save_dir: Option<String>,
    last_format: String,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut s = state.settings.lock().unwrap();
    if s.default_save_dir != default_save_dir {
        s.default_save_dir = default_save_dir;
    }
    if last_format == "png" || last_format == "jpeg" {
        s.last_format = last_format;
    }
    settings::save(&s)
}

/// 复制选区（含标注）到剪贴板（FR-18）。
#[tauri::command]
pub fn copy_image(
    app: AppHandle,
    selection: PhysRect,
    annotations_png: Vec<u8>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let guard = state.session.lock().unwrap();
    let s = guard.as_ref().ok_or("会话已结束")?;
    let rgba = jetsnap_win::compose::crop_bgra_to_rgba(
        &s.bgra,
        s.virtual_rect.w,
        s.virtual_rect.h,
        &selection,
    )?;
    let composed = jetsnap_win::compose::composite_annotation_png(
        rgba,
        selection.w,
        selection.h,
        &annotations_png,
    )?;
    jetsnap_win::compose::copy_rgba_to_clipboard(&composed, selection.w, selection.h)
}

/// 保存选区（含标注）到文件（FR-19），并记住目录与格式偏好。
#[tauri::command]
pub fn save_image(
    app: AppHandle,
    selection: PhysRect,
    annotations_png: Vec<u8>,
    path: String,
    format: String,
) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        let guard = state.session.lock().unwrap();
        let s = guard.as_ref().ok_or("会话已结束")?;
        let rgba = jetsnap_win::compose::crop_bgra_to_rgba(
            &s.bgra,
            s.virtual_rect.w,
            s.virtual_rect.h,
            &selection,
        )?;
        let composed = jetsnap_win::compose::composite_annotation_png(
            rgba,
            selection.w,
            selection.h,
            &annotations_png,
        )?;
        let bytes = match format.as_str() {
            "jpeg" | "jpg" => jetsnap_win::compose::encode_jpeg(&composed, selection.w, selection.h, 95)?,
            _ => jetsnap_win::compose::encode_png(&composed, selection.w, selection.h)?,
        };
        std::fs::write(&path, bytes).map_err(|e| format!("写入文件失败: {e}"))?;
    }
    // 保存成功后记忆目录与格式
    let state = app.state::<AppState>();
    let mut s = state.settings.lock().unwrap();
    if let Some(parent) = std::path::Path::new(&path).parent() {
        s.default_save_dir = Some(parent.to_string_lossy().to_string());
    }
    s.last_format = format;
    let _ = settings::save(&s);
    Ok(())
}

/// 性能打点（N1~N6 测量通道，开发计划 9.3）。
#[tauri::command]
pub fn log_metric(name: String, ms: f64) {
    eprintln!("[metric] {name}: {ms:.1}ms");
}
