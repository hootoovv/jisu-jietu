//! 可见顶层窗口枚举 + DWM 边界修正（Windows 实现；其他平台返回空表）。
//!
//! 产出带 z 序的窗口矩形表，供前端命中测试（智能窗口识别 FR-04）。
//! 过滤链：可见 && 非最小化 && 非工具提示/无激活/穿透窗口 && 非 cloaked UWP
//! && 边界有效。任务栏（Shell_TrayWnd）与桌面作为特例保留 ——
//! 与 QQ 截图「点任务栏选任务栏、点桌面选全屏」一致。

use crate::{PhysRect, WindowInfo};

#[cfg(not(windows))]
pub fn enumerate_visible_windows() -> Vec<WindowInfo> {
    Vec::new()
}

#[cfg(windows)]
mod win_impl {
    use super::*;

    use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
    use windows::Win32::Graphics::Dwm::{
        DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindowLongW, GetWindowRect, GetWindowTextLengthW,
        GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, GWL_EXSTYLE,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    };

    struct RawWin {
        hwnd: HWND,
        rect: RECT,
        z: u32,
    }

    pub fn enumerate_visible_windows() -> Vec<WindowInfo> {
        let mut raw: Vec<RawWin> = Vec::new();
        unsafe {
            let _ = EnumWindows(Some(enum_proc), LPARAM(&mut raw as *mut _ as isize));
        }

        let mut items: Vec<WindowInfo> = raw
            .into_iter()
            .filter(|w| keep_window(w.hwnd, &w.rect))
            .map(|w| to_info(w.hwnd, &w.rect, w.z))
            .collect();

        // 桌面作为 z 序最低的兜底候选（点击空白桌面 = 选中整个虚拟屏，
        // 与 QQ 行为一致）。矩形取所有已枚举窗口的并集包围盒近似虚拟屏。
        if let Some((min_x, min_y, max_x, max_y)) = items.iter().fold(
            None,
            |acc: Option<(i32, i32, i32, i32)>, w| {
                let r = &w.rect;
                Some(match acc {
                    None => (r.x, r.y, r.right(), r.bottom()),
                    Some((x0, y0, x1, y1)) => {
                        (x0.min(r.x), y0.min(r.y), x1.max(r.right()), y1.max(r.bottom()))
                    }
                })
            },
        ) {
            if max_x > min_x && max_y > min_y {
                items.push(WindowInfo {
                    hwnd: 0,
                    rect: PhysRect::new(
                        min_x,
                        min_y,
                        (max_x - min_x) as u32,
                        (max_y - min_y) as u32,
                    ),
                    title: "桌面".into(),
                    process: "desktop".into(),
                    z: u32::MAX,
                });
            }
        }
        items
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, data: LPARAM) -> BOOL {
        let raw = &mut *(data.0 as *mut Vec<RawWin>);
        if IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() {
            let rect = window_rect(hwnd);
            raw.push(RawWin {
                hwnd,
                rect,
                z: raw.len() as u32,
            });
        }
        BOOL(1)
    }

    /// 取窗口真实视觉边界：优先 DWM 扩展边界（去除阴影），失败回退 GetWindowRect。
    unsafe fn window_rect(hwnd: HWND) -> RECT {
        let mut rect = RECT::default();
        let ok = DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut RECT as *mut core::ffi::c_void,
            std::mem::size_of::<RECT>() as u32,
        );
        if ok.is_err() {
            let _ = GetWindowRect(hwnd, &mut rect);
        }
        rect
    }

    fn keep_window(hwnd: HWND, rect: &RECT) -> bool {
        unsafe {
            // 尺寸有效
            if rect.right <= rect.left || rect.bottom <= rect.top {
                return false;
            }
            let class = class_name(hwnd);
            // 任务栏特例：Shell_TrayWnd 带 TOOLWINDOW 样式但必须可选
            if class == "Shell_TrayWnd" || class == "Shell_SecondaryTrayWnd" {
                return true;
            }
            let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            if (ex & WS_EX_TOOLWINDOW.0) != 0 {
                return false; // 工具提示 / 浮窗
            }
            if (ex & WS_EX_NOACTIVATE.0) != 0 && class != "Progman" && class != "WorkerW" {
                return false; // 无激活浮层
            }
            if (ex & WS_EX_TRANSPARENT.0) != 0 && class != "Progman" && class != "WorkerW" {
                return false; // 鼠标穿透窗口
            }
            // cloaked 的 UWP 挂起窗口
            let mut cloaked: u32 = 0;
            let ok = DwmGetWindowAttribute(
                hwnd,
                DWMWA_CLOAKED,
                &mut cloaked as *mut u32 as *mut core::ffi::c_void,
                std::mem::size_of::<u32>() as u32,
            );
            if ok.is_ok() && cloaked != 0 {
                return false;
            }
            true
        }
    }

    fn to_info(hwnd: HWND, rect: &RECT, z: u32) -> WindowInfo {
        WindowInfo {
            hwnd: hwnd.0 as usize,
            rect: PhysRect::new(
                rect.left,
                rect.top,
                (rect.right - rect.left).max(0) as u32,
                (rect.bottom - rect.top).max(0) as u32,
            ),
            title: unsafe { window_title(hwnd) },
            process: unsafe { process_name(hwnd) },
            z,
        }
    }

    unsafe fn class_name(hwnd: HWND) -> String {
        let mut buf = [0u16; 64];
        let n = GetClassNameW(hwnd, &mut buf);
        if n <= 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..n as usize])
    }

    unsafe fn window_title(hwnd: HWND) -> String {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (len + 1) as usize];
        let n = GetWindowTextW(hwnd, &mut buf);
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }

    unsafe fn process_name(hwnd: HWND) -> String {
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return String::new();
        }
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return String::new();
        };
        let mut buf = [0u16; 512];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        let _ = windows::Win32::Foundation::CloseHandle(handle);
        if ok.is_err() || len == 0 {
            return String::new();
        }
        let full = String::from_utf16_lossy(&buf[..len as usize]);
        full.rsplit(['\\', '/']).next().unwrap_or("").to_string()
    }
}

#[cfg(windows)]
pub use win_impl::enumerate_visible_windows;
