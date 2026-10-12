//! 覆盖窗口与设置窗口管理（开发计划 5.1.4）。
//!
//! 关键设计：覆盖窗口**不透明**——冻结位图直接渲染为窗口内容，
//! 蒙版是 canvas 绘制内容而非窗口透明度，绕开 WS_EX_LAYERED +
//! WebView2 透明的性能坑。窗口先隐藏创建（visible=false），
//! 前端加载完 snapshot 后调用 reveal_overlay 再显示，避免黑窗闪现。

use jetsnap_win::PhysRect;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const OVERLAY_LABEL: &str = "overlay";
pub const SETTINGS_LABEL: &str = "settings";

pub fn create_overlay(app: &AppHandle, virtual_rect: PhysRect) -> Result<(), String> {
    // 防御：残留窗口先关闭
    if let Some(existing) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = existing.destroy();
    }

    let win = WebviewWindowBuilder::new(app, OVERLAY_LABEL, WebviewUrl::App("index.html".into()))
        .title("JetSnap")
        .decorations(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .visible(false)
        .build()
        .map_err(|e| format!("创建覆盖窗口失败: {e}"))?;

    // 物理像素几何：虚拟屏原点可能为负（副屏在主屏左侧/上方）
    win.set_position(tauri::PhysicalPosition::new(virtual_rect.x, virtual_rect.y))
        .map_err(|e| format!("定位覆盖窗口失败: {e}"))?;
    win.set_size(tauri::PhysicalSize::new(virtual_rect.w, virtual_rect.h))
        .map_err(|e| format!("调整覆盖窗口尺寸失败: {e}"))?;
    Ok(())
}

pub fn destroy_overlay(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = win.destroy();
    }
}

pub fn reveal(app: &AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(OVERLAY_LABEL) {
        win.show().map_err(|e| format!("显示覆盖窗口失败: {e}"))?;
        win.set_focus().map_err(|e| format!("聚焦覆盖窗口失败: {e}"))?;
        Ok(())
    } else {
        Err("覆盖窗口不存在".into())
    }
}

pub fn open_settings(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(SETTINGS_LABEL) {
        let _ = win.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(
        app,
        SETTINGS_LABEL,
        WebviewUrl::App("settings.html".into()),
    )
    .title("JetSnap 设置")
    .inner_size(440.0, 400.0)
    .resizable(false)
    .maximizable(false)
    .build();
}
