//! JetSnap 应用入口（M0 骨架版本）。
//!
//! - 常驻托盘 + 全局热键在 M1 接入；
//! - 本文件在后续里程碑中逐步补全会话管线。

#![allow(dead_code)] // M0 阶段部分模块尚未接线

mod settings;
mod state;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running JetSnap");
}
