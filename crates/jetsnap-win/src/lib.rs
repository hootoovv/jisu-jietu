//! # jetsnap-win
//!
//! JetSnap 的 Windows 系统能力与纯逻辑层：
//! - [`hotkey_combo`]：快捷键组合解析（纯逻辑，跨平台可测）
//! - [`compose`]：位图裁剪 / 合成 / 编码（纯逻辑，跨平台可测）
//! - `capture`：GDI 全虚拟屏捕获（仅 Windows）
//! - `window_enum`：可见窗口枚举 + DWM 边界修正（仅 Windows）
//! - `autostart`：注册表开机自启（仅 Windows）
//!
//! 本 crate 是 Cargo workspace 成员，被 `src-tauri`（Tauri 应用壳）依赖。
//! 纯逻辑模块不依赖 Win32，可在任意平台运行 `cargo test`。

pub mod compose;
pub mod hotkey_combo;

pub mod autostart;
pub mod capture;
pub mod window_enum;

use serde::{Deserialize, Serialize};

/// 物理像素矩形，虚拟屏坐标系（主显示器左上角为原点）。
///
/// IPC 边界上所有矩形一律使用物理像素（见开发计划 5.2.1 坐标纪律）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysRect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl PhysRect {
    pub fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    pub fn right(&self) -> i32 {
        self.x + self.w as i32
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.h as i32
    }

    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }

    /// 两矩形交集；无交集返回 None。
    pub fn intersect(&self, other: &PhysRect) -> Option<PhysRect> {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = self.right().min(other.right());
        let y1 = self.bottom().min(other.bottom());
        if x1 > x0 && y1 > y0 {
            Some(PhysRect::new(x0, y0, (x1 - x0) as u32, (y1 - y0) as u32))
        } else {
            None
        }
    }
}

/// 单台显示器的几何与 DPI 元数据（物理像素）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    pub id: String,
    pub rect: PhysRect,
    /// DPI 缩放系数（如 1.0 / 1.25 / 1.5 / 2.0）
    pub dpi_scale: f32,
    pub is_primary: bool,
}

/// 一枚可见顶层窗口的识别信息（边界已经 DWM 修正）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub hwnd: usize,
    pub rect: PhysRect,
    pub title: String,
    pub process: String,
    /// z 序，越小越顶层
    pub z: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_contains_basic() {
        let r = PhysRect::new(10, 10, 100, 50);
        assert!(r.contains(10, 10));
        assert!(r.contains(109, 59));
        assert!(!r.contains(110, 10));
        assert!(!r.contains(10, 60));
        assert!(!r.contains(9, 9));
    }

    #[test]
    fn rect_intersect_overlapping() {
        let a = PhysRect::new(0, 0, 100, 100);
        let b = PhysRect::new(50, 50, 100, 100);
        let i = a.intersect(&b).expect("应有交集");
        assert_eq!(i, PhysRect::new(50, 50, 50, 50));
    }

    #[test]
    fn rect_intersect_disjoint() {
        let a = PhysRect::new(0, 0, 10, 10);
        let b = PhysRect::new(20, 20, 10, 10);
        assert!(a.intersect(&b).is_none());
    }

    #[test]
    fn rect_intersect_touching_edge_is_empty() {
        let a = PhysRect::new(0, 0, 10, 10);
        let b = PhysRect::new(10, 0, 10, 10);
        assert!(a.intersect(&b).is_none(), "边相邻不算相交");
    }

    #[test]
    fn negative_origin_supported() {
        // 副屏位于主屏左侧时，虚拟屏原点为负
        let r = PhysRect::new(-1920, 0, 3840, 1080);
        assert!(r.contains(-1920, 0));
        assert!(r.contains(1919, 1079));
        assert_eq!(r.right(), 1920);
    }
}
