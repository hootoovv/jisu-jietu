//! 应用状态（会话与设置），全部经 Mutex 保护。

use std::sync::atomic::AtomicU32;
use std::sync::Mutex;

use jetsnap_win::{MonitorInfo, PhysRect, WindowInfo};

use crate::settings::Settings;

/// 一次截图会话的全部资源（开发计划 R-07：生命周期统一托管）。
pub struct SessionData {
    pub id: u32,
    /// 原始全虚拟屏 BGRA（自顶向下），导出用（无损）
    pub bgra: Vec<u8>,
    /// 显示用 JPEG（q92），经 snapshot:// 协议供 <img> 加载
    pub jpeg: Vec<u8>,
    pub virtual_rect: PhysRect,
    pub monitors: Vec<MonitorInfo>,
    pub windows: Vec<WindowInfo>,
    /// 热键按下到冻结完成的耗时（N1 指标测量用）
    pub hotkey_elapsed_ms: f64,
}

#[derive(Default)]
pub struct AppState {
    pub settings: Mutex<Settings>,
    pub session: Mutex<Option<SessionData>>,
    pub next_id: AtomicU32,
}

impl AppState {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings: Mutex::new(settings),
            session: Mutex::new(None),
            next_id: AtomicU32::new(1),
        }
    }
}
