//! GDI 全虚拟屏捕获（仅 Windows）。
//!
//! 冻结流水线的第一步（开发计划 5.1.2）：
//! 1. `GetSystemMetrics` 读取虚拟屏几何（支持负坐标副屏）；
//! 2. 单次 `BitBlt(SRCCOPY|CAPTUREBLT)` 拷贝整个虚拟桌面（CAPTUREBLT
//!    确保分层窗口/通知横幅一并捕获）；
//! 3. `GetDIBits` 以负 biHeight 直接取**自顶向下** BGRA，免去手工翻行；
//! 4. 顺带枚举显示器 DPI 元数据与可见窗口表（在覆盖窗口创建之前完成，
//!    避免枚举到自身，见开发计划 5.1.3）。

use crate::window_enum;
use crate::{MonitorInfo, PhysRect, WindowInfo};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, EnumDisplayMonitors,
    GetDC, GetDeviceCaps, GetDIBits, GetMonitorInfoW, MonitorFromPoint, ReleaseDC, SelectObject,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS, HDC, HMONITOR, LOGPIXELSX,
    MONITORINFO, MONITORINFOEXW, MONITORINFOF_PRIMARY, MONITORENUMPROC, SRCCOPY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

pub struct CaptureResult {
    /// 自顶向下 BGRA，尺寸 = virtual_rect
    pub bgra: Vec<u8>,
    pub virtual_rect: PhysRect,
    pub monitors: Vec<MonitorInfo>,
    pub windows: Vec<WindowInfo>,
}

/// 捕获整个虚拟屏幕（全部显示器拼合区域）。
pub fn capture_virtual_screen() -> Result<CaptureResult, String> {
    let (vx, vy, vw, vh) = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    if vw <= 0 || vh <= 0 {
        return Err(format!("虚拟屏几何非法: origin=({vx},{vy}) size={vw}x{vh}"));
    }
    let virtual_rect = PhysRect::new(vx, vy, vw as u32, vh as u32);

    let bgra = unsafe { bitblt_virtual_screen(vx, vy, vw, vh)? };
    let monitors = unsafe { enumerate_monitors() };
    let windows = window_enum::enumerate_visible_windows();

    Ok(CaptureResult {
        bgra,
        virtual_rect,
        monitors,
        windows,
    })
}

unsafe fn bitblt_virtual_screen(vx: i32, vy: i32, vw: i32, vh: i32) -> Result<Vec<u8>, String> {
    let hdc_screen = GetDC(None);
    if hdc_screen.is_invalid() {
        return Err("GetDC(NULL) 失败".into());
    }
    let result = (|| -> Result<Vec<u8>, String> {
        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        let hbmp = CreateCompatibleBitmap(hdc_screen, vw, vh);
        if hbmp.is_invalid() {
            return Err("CreateCompatibleBitmap 失败".into());
        }
        let old = SelectObject(hdc_mem, hbmp.into());

        let ok = BitBlt(
            hdc_mem,
            0,
            0,
            vw,
            vh,
            Some(hdc_screen),
            vx,
            vy,
            SRCCOPY | CAPTUREBLT,
        )
        .ok();
        if let Err(e) = ok {
            return Err(format!("BitBlt 失败: {e}"));
        }

        // 负 biHeight => 自顶向下行序，免手工翻转
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: vw,
                biHeight: -vh,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: (vw * vh * 4) as u32,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut pixels = vec![0u8; (vw * vh * 4) as usize];
        let lines = GetDIBits(
            hdc_mem,
            hbmp,
            0,
            vh as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        if lines != vh {
            return Err(format!("GetDIBits 仅返回 {lines}/{vh} 行"));
        }

        SelectObject(hdc_mem, old);
        let _ = DeleteObject(hbmp.into());
        let _ = DeleteDC(hdc_mem);
        Ok(pixels)
    })();

    ReleaseDC(None, hdc_screen);
    result
}

struct MonitorItem {
    hmon: HMONITOR,
    rect: RECT,
    primary: bool,
}

unsafe extern "system" fn monitor_enum_proc(
    hmon: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    data: LPARAM,
) -> windows::core::BOOL {
    let out = &mut *(data.0 as *mut Vec<MonitorItem>);
    let mut mi = MONITORINFOEXW {
        ..Default::default()
    };
    mi.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if GetMonitorInfoW(hmon, &mut mi as *mut MONITORINFOEXW as *mut MONITORINFO).as_bool() {
        out.push(MonitorItem {
            hmon,
            rect: mi.monitorInfo.rcMonitor,
            primary: (mi.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0,
        });
    }
    windows::core::BOOL(1)
}

unsafe fn enumerate_monitors() -> Vec<MonitorInfo> {
    let mut items: Vec<MonitorItem> = Vec::new();
    let _ = EnumDisplayMonitors(
        None,
        None,
        Some(monitor_enum_proc),
        LPARAM(&mut items as *mut _ as isize),
    );

    items
        .into_iter()
        .enumerate()
        .map(|(i, m)| MonitorInfo {
            id: format!("monitor-{i}"),
            rect: PhysRect::new(
                m.rect.left,
                m.rect.top,
                (m.rect.right - m.rect.left).max(0) as u32,
                (m.rect.bottom - m.rect.top).max(0) as u32,
            ),
            dpi_scale: monitor_dpi_scale(m.hmon),
            is_primary: m.primary,
        })
        .collect()
}

/// 通过显示器 DC 的 LOGPIXELSX 读取 DPI 缩放系数（失败回退 1.0）。
unsafe fn monitor_dpi_scale(hmon: HMONITOR) -> f32 {
    // 直接用 HMONITOR 句柄构建 DC：借助 MONITORINFOEXW 拿设备名
    let mut mi = MONITORINFOEXW {
        ..Default::default()
    };
    mi.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if !GetMonitorInfoW(hmon, &mut mi as *mut MONITORINFOEXW as *mut MONITORINFO).as_bool() {
        return 1.0;
    }
    let device = PCWSTR(mi.szDevice.as_ptr());
    let hdc = windows::Win32::Graphics::Gdi::CreateDCW(Some(device), None, None, None);
    if hdc.is_invalid() {
        return 1.0;
    }
    let dpi = GetDeviceCaps(Some(hdc), LOGPIXELSX);
    let _ = DeleteDC(hdc);
    if dpi > 0 {
        dpi as f32 / 96.0
    } else {
        1.0
    }
}
