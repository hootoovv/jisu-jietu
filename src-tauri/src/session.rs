//! 截图会话编排（开发计划 4.3 时序图）：
//! 热键 → 后台线程捕获+枚举+编码 → 主线程创建隐藏覆盖窗口 →
//! 前端拉取 session 并加载 snapshot:// → reveal 显示。
//!
//! 事件采用「拉取为主、事件兜底」策略，规避页面尚未注册监听的竞态。

use std::sync::atomic::Ordering;

use jetsnap_win::{MonitorInfo, PhysRect, WindowInfo};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::overlay;
use crate::state::{AppState, SessionData};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FrozenPayload {
    pub snapshot_id: u32,
    pub virtual_rect: PhysRect,
    pub monitors: Vec<MonitorInfo>,
    pub windows: Vec<WindowInfo>,
    pub hotkey_elapsed_ms: f64,
}

pub fn start_session(app: &AppHandle) {
    let state = app.state::<AppState>();
    // 防重入（S1 异常分支）：会话进行中忽略重复热键
    if state.active.swap(true, Ordering::SeqCst) {
        return;
    }

    let app = app.clone();
    std::thread::spawn(move || {
        let t0 = std::time::Instant::now();
        match jetsnap_win::capture::capture_virtual_screen() {
            Ok(cap) => {
                let state = app.state::<AppState>();
                let id = state.next_id.fetch_add(1, Ordering::SeqCst);

                let jpeg = match jetsnap_win::compose::bgra_to_jpeg(
                    &cap.bgra,
                    cap.virtual_rect.w,
                    cap.virtual_rect.h,
                    92,
                ) {
                    Ok(j) => j,
                    Err(e) => {
                        abort(&app, &format!("JPEG 编码失败: {e}"));
                        return;
                    }
                };

                let payload = FrozenPayload {
                    snapshot_id: id,
                    virtual_rect: cap.virtual_rect,
                    monitors: cap.monitors.clone(),
                    windows: cap.windows.clone(),
                    hotkey_elapsed_ms: t0.elapsed().as_secs_f64() * 1000.0,
                };

                *state.session.lock().unwrap() = Some(SessionData {
                    id,
                    bgra: cap.bgra,
                    jpeg,
                    virtual_rect: cap.virtual_rect,
                    monitors: cap.monitors,
                    windows: cap.windows,
                    hotkey_elapsed_ms: payload.hotkey_elapsed_ms,
                });

                // 主线程创建隐藏覆盖窗口（负坐标副屏用物理像素定位）
                let app2 = app.clone();
                let vr = payload.virtual_rect;
                let r = app.run_on_main_thread(move || {
                    if let Err(e) = overlay::create_overlay(&app2, vr) {
                        abort(&app2, &e);
                    }
                });
                if let Err(e) = r {
                    abort(&app, &format!("调度主线程失败: {e}"));
                    return;
                }

                // 事件兜底（页面若已在监听则立即刷新；拉取是主通道）
                let _ = app.emit("session-frozen", &payload);
            }
            Err(e) => abort(&app, &e),
        }
    });
}

/// 会话异常终止：清理状态并广播原因（前端 toast / 退出等待态）。
pub fn abort(app: &AppHandle, reason: &str) {
    eprintln!("[jisu-jietu] 会话终止: {reason}");
    let state = app.state::<AppState>();
    *state.session.lock().unwrap() = None;
    state.active.store(false, Ordering::SeqCst);
    let _ = app.emit("session-aborted", reason);
}

/// 会话正常结束（确认 / 取消），由 end_session 命令调用。
pub fn finish(app: &AppHandle) {
    let state = app.state::<AppState>();
    *state.session.lock().unwrap() = None;
    state.active.store(false, Ordering::SeqCst);
    overlay::destroy_overlay(app);
}
