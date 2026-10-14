//! JetSnap 应用入口：常驻托盘 + 全局热键 + snapshot 协议 + 命令注册。

mod commands;
mod hotkey;
mod overlay;
mod protocol;
mod session;
mod settings;
mod state;
mod tray;

use tauri::Manager;

use crate::state::AppState;

pub fn run() {
    // panic hook：会话内 panic 至少留下日志（开发计划 M6；窗口清理由
    // WindowEvent::Destroyed 兜底）
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("[jisu-jietu] panic: {info}");
        default_hook(info);
    }));

    tauri::Builder::default()
        // 单实例限制：必须最先注册；二次启动时唤起已有实例的设置窗口并立即退出
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            overlay::open_settings(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        session::start_session(app);
                    }
                })
                .build(),
        )
        .register_uri_scheme_protocol("snapshot", protocol::handle_snapshot)
        .manage(AppState::new(settings::load()))
        .invoke_handler(tauri::generate_handler![
            commands::fetch_session,
            commands::reveal_overlay,
            commands::end_session,
            commands::trigger_capture,
            commands::get_settings,
            commands::set_hotkey,
            commands::set_autostart,
            commands::update_preferences,
            commands::copy_image,
            commands::save_image,
            commands::log_metric,
        ])
        .setup(|app| {
            let handle = app.handle();
            // 注册默认/用户热键（失败仅告警，不阻塞启动）
            let state = handle.state::<AppState>();
            let combo = state.settings.lock().unwrap().hotkey.clone();
            if let Err(e) = hotkey::register(handle, &combo) {
                eprintln!("[jisu-jietu] 热键注册失败（{combo}）: {e}");
            }
            tray::create(handle)?;
            // 预建隐藏覆盖窗口：把 WebView2 建进程 + 页面加载的冷启动成本从
            // 首次热键路径移到应用启动期（几何在每次会话 ensure_overlay 时重设）
            if let Ok(vr) = jetsnap_win::capture::virtual_screen_rect() {
                if let Err(e) = overlay::create_overlay(handle, vr) {
                    eprintln!("[jisu-jietu] 预建覆盖窗口失败（退化为热键时创建）: {e}");
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // 兜底清理：覆盖窗口被外部销毁（如 Alt+F4）时回收会话
            if window.label() == overlay::OVERLAY_LABEL {
                if let tauri::WindowEvent::Destroyed = event {
                    let handle = window.app_handle();
                    if let Some(state) = handle.try_state::<AppState>() {
                        *state.session.lock().unwrap() = None;
                        state
                            .active
                            .store(false, std::sync::atomic::Ordering::SeqCst);
                    }
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building jisu-jietu")
        .run(|_app, event| {
            // 托盘常驻：最后一个窗口关闭（会话结束）触发的退出请求必须阻止，
            // 否则截一次图整个应用就退出；显式退出仅走托盘菜单的 app.exit(0)
            if let tauri::RunEvent::ExitRequested { code, api, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
