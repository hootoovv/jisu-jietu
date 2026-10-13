//! 位图裁剪 / 合成 / 编码（纯逻辑，跨平台可测）。
//!
//! 数据约定：所有 BGRA/RGBA 缓冲均为**自顶向下**行序。
//! - 显示路径：虚拟屏 BGRA → 32bpp BMP 头封装（零像素转换）→ `snapshot://` 协议；
//! - 导出路径：虚拟屏 BGRA 按选区裁剪 → 与标注层 PNG 逐像素合成 → PNG/JPEG/剪贴板。

use crate::PhysRect;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::{DynamicImage, ExtendedColorType, ImageEncoder, RgbaImage};

/// 行内 BGRA → RGBA（交换 R/B 通道）。
pub fn bgra_to_rgba_inplace(buf: &mut [u8]) {
    for px in buf.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
}

/// 将选区从全虚拟屏 BGRA 缓冲中裁出，输出 RGBA。
///
/// 选区会被钳制到缓冲边界内；越界部分按可用区域截断。
pub fn crop_bgra_to_rgba(
    bgra: &[u8],
    full_w: u32,
    full_h: u32,
    sel: &PhysRect,
) -> Result<Vec<u8>, String> {
    let full = PhysRect::new(0, 0, full_w, full_h);
    let clip = full
        .intersect(sel)
        .ok_or_else(|| format!("选区 ({},{},{},{}) 超出画面边界", sel.x, sel.y, sel.w, sel.h))?;

    let stride = full_w as usize * 4;
    let mut out = Vec::with_capacity(clip.w as usize * clip.h as usize * 4);
    for row in clip.y..clip.bottom() {
        let base = row as usize * stride + clip.x as usize * 4;
        let line = &bgra[base..base + clip.w as usize * 4];
        // BGRA -> RGBA
        for px in line.chunks_exact(4) {
            out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
    }
    Ok(out)
}

/// Source-Over Alpha 合成：`over` 叠加到 `base` 上（原地），输出保持不透明。
pub fn composite_over(base: &mut [u8], over: &[u8]) {
    for (b, o) in base.chunks_exact_mut(4).zip(over.chunks_exact(4)) {
        let a = o[3] as u32;
        if a == 0 {
            continue;
        }
        if a == 255 {
            b.copy_from_slice(o);
            continue;
        }
        let inv = 255 - a;
        b[0] = ((o[0] as u32 * a + b[0] as u32 * inv) / 255) as u8;
        b[1] = ((o[1] as u32 * a + b[1] as u32 * inv) / 255) as u8;
        b[2] = ((o[2] as u32 * a + b[2] as u32 * inv) / 255) as u8;
        b[3] = 255;
    }
}

/// 把标注层 PNG（带 Alpha）合成到选区裁剪结果上，返回合成后的 RGBA。
pub fn composite_annotation_png(mut base_rgba: Vec<u8>, w: u32, h: u32, png: &[u8]) -> Result<Vec<u8>, String> {
    if png.is_empty() {
        return Ok(base_rgba);
    }
    let overlay = image::load_from_memory(png)
        .map_err(|e| format!("标注层 PNG 解码失败: {e}"))?
        .to_rgba8();
    let (ow, oh) = overlay.dimensions();
    if ow != w || oh != h {
        return Err(format!("标注层尺寸 {ow}x{oh} 与选区 {w}x{h} 不一致"));
    }
    let overlay = overlay.into_raw();
    composite_over(&mut base_rgba, &overlay);
    Ok(base_rgba)
}

/// RGBA → PNG。
pub fn encode_png(rgba: &[u8], w: u32, h: u32) -> Result<Vec<u8>, String> {
    let expected = w as usize * h as usize * 4;
    if rgba.len() != expected {
        return Err(format!("RGBA 长度 {} 与 {}x{} 不符", rgba.len(), w, h));
    }
    let mut out = Vec::new();
    PngEncoder::new(&mut out)
        .write_image(rgba, w, h, ExtendedColorType::Rgba8)
        .map_err(|e| format!("PNG 编码失败: {e}"))?;
    Ok(out)
}

/// RGBA → JPEG（内部压平为不透明 RGB）。
pub fn encode_jpeg(rgba: &[u8], w: u32, h: u32, quality: u8) -> Result<Vec<u8>, String> {
    let expected = w as usize * h as usize * 4;
    if rgba.len() != expected {
        return Err(format!("RGBA 长度 {} 与 {}x{} 不符", rgba.len(), w, h));
    }
    let img = RgbaImage::from_raw(w, h, rgba.to_vec())
        .ok_or_else(|| "图像缓冲构造失败".to_string())?;
    let rgb = DynamicImage::ImageRgba8(img).to_rgb8();
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, quality)
        .write_image(rgb.as_raw(), w, h, ExtendedColorType::Rgb8)
        .map_err(|e| format!("JPEG 编码失败: {e}"))?;
    Ok(out)
}

/// 全虚拟屏 BGRA → 32bpp BMP（显示路径）。
///
/// BGRA 内存布局与 32bpp BI_BITFIELDS BMP 逐字节一致，只需拼 122 字节
/// 文件头，无逐像素转换、无压缩编码——JPEG 编码（纯 Rust 实现）曾是
/// 「热键→蒙版」时延的最大单项。alpha 掩码置 0：GDI 捕获的 alpha 字节
/// 不可信，按不透明处理；负高度表示自顶向下行序，免去翻行。
pub fn bgra_to_bmp(bgra: &[u8], w: u32, h: u32) -> Result<Vec<u8>, String> {
    let expected = w as usize * h as usize * 4;
    if bgra.len() != expected {
        return Err(format!("BGRA 长度 {} 与 {}x{} 不符", bgra.len(), w, h));
    }

    const FILE_HEADER: usize = 14;
    const INFO_HEADER: usize = 108; // BITMAPV4HEADER
    let payload = expected;
    let mut out = Vec::with_capacity(FILE_HEADER + INFO_HEADER + payload);

    // BITMAPFILEHEADER
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&((FILE_HEADER + INFO_HEADER + payload) as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // 保留字
    out.extend_from_slice(&((FILE_HEADER + INFO_HEADER) as u32).to_le_bytes()); // 像素偏移

    // BITMAPV4HEADER
    out.extend_from_slice(&(INFO_HEADER as u32).to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(-(h as i32)).to_le_bytes()); // 负高度 = 自顶向下
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&32u16.to_le_bytes()); // bpp
    out.extend_from_slice(&3u32.to_le_bytes()); // BI_BITFIELDS
    out.extend_from_slice(&(payload as u32).to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes()); // x pels/meter
    out.extend_from_slice(&0i32.to_le_bytes()); // y pels/meter
    out.extend_from_slice(&0u32.to_le_bytes()); // clr used
    out.extend_from_slice(&0u32.to_le_bytes()); // clr important
    out.extend_from_slice(&0x00FF_0000u32.to_le_bytes()); // R 掩码
    out.extend_from_slice(&0x0000_FF00u32.to_le_bytes()); // G 掩码
    out.extend_from_slice(&0x0000_00FFu32.to_le_bytes()); // B 掩码
    out.extend_from_slice(&0u32.to_le_bytes()); // A 掩码 = 0（不透明）
    out.extend_from_slice(&0u32.to_le_bytes()); // CSType
    out.extend_from_slice(&[0u8; 36]); // endpoints
    out.extend_from_slice(&[0u8; 12]); // gamma r/g/b

    debug_assert_eq!(out.len(), FILE_HEADER + INFO_HEADER);
    out.extend_from_slice(bgra);
    Ok(out)
}

/// RGBA → 系统剪贴板（Windows 上经 arboard 写 CF_DIBV5/CF_DIB）。
#[cfg(windows)]
pub fn copy_rgba_to_clipboard(rgba: &[u8], w: u32, h: u32) -> Result<(), String> {
    use arboard::{Clipboard, ImageData};
    let mut clip = Clipboard::new().map_err(|e| format!("打开剪贴板失败: {e}"))?;
    let img = ImageData {
        width: w as usize,
        height: h as usize,
        bytes: std::borrow::Cow::Borrowed(rgba),
    };
    clip.set_image(img).map_err(|e| format!("写入剪贴板失败: {e}"))
}

#[cfg(not(windows))]
pub fn copy_rgba_to_clipboard(_rgba: &[u8], _w: u32, _h: u32) -> Result<(), String> {
    Err("剪贴板仅支持 Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient_bgra(w: u32, h: u32) -> Vec<u8> {
        // 像素 (x,y) = B=x, G=y, R=128, A=255（BGRA）
        let mut v = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                v[i] = x as u8;
                v[i + 1] = y as u8;
                v[i + 2] = 128;
                v[i + 3] = 255;
            }
        }
        v
    }

    #[test]
    fn bgra_swap_inplace() {
        let mut buf = vec![1, 2, 3, 4, 5, 6, 7, 8]; // BGRA BGRA
        bgra_to_rgba_inplace(&mut buf);
        assert_eq!(buf, vec![3, 2, 1, 4, 7, 6, 5, 8]);
    }

    #[test]
    fn bmp_header_and_payload() {
        let bgra = gradient_bgra(4, 3);
        let bmp = bgra_to_bmp(&bgra, 4, 3).unwrap();
        assert_eq!(bmp.len(), 14 + 108 + 4 * 3 * 4);
        assert_eq!(&bmp[0..2], b"BM");
        let u32at = |o: usize| u32::from_le_bytes(bmp[o..o + 4].try_into().unwrap());
        assert_eq!(u32at(2), bmp.len() as u32); // 文件长度
        assert_eq!(u32at(10), 14 + 108); // 像素偏移
        assert_eq!(u32at(14), 108); // V4 头长度
        assert_eq!(u32at(18), 4); // 宽
        assert_eq!(u32at(22) as i32, -3); // 负高度 = 自顶向下
        assert_eq!(u16::from_le_bytes(bmp[26..28].try_into().unwrap()), 1); // planes
        assert_eq!(u16::from_le_bytes(bmp[28..30].try_into().unwrap()), 32); // bpp
        assert_eq!(u32at(30), 3); // BI_BITFIELDS
        assert_eq!(u32at(54), 0x00FF_0000); // R 掩码
        assert_eq!(u32at(58), 0x0000_FF00); // G 掩码
        assert_eq!(u32at(62), 0x0000_00FF); // B 掩码
        assert_eq!(u32at(66), 0); // A 掩码 = 不透明
        assert_eq!(&bmp[122..], &bgra[..]); // 像素原样透传（BGRA 布局即 BMP 布局）
    }

    #[test]
    fn bmp_len_mismatch_errors() {
        assert!(bgra_to_bmp(&[0u8; 7], 4, 3).is_err());
    }

    #[test]
    fn crop_matches_expected_pixels() {
        let bgra = gradient_bgra(10, 10);
        let sel = PhysRect::new(2, 3, 4, 2);
        let rgba = crop_bgra_to_rgba(&bgra, 10, 10, &sel).unwrap();
        assert_eq!(rgba.len(), 4 * 2 * 4);
        // 第一行第一个像素：x=2,y=3 → R=128,G=3,B=2
        assert_eq!(&rgba[0..4], &[128, 3, 2, 255]);
        // 最后一行最后像素：x=5,y=4
        let last = &rgba[(4 * 2 * 4 - 4)..];
        assert_eq!(last, &[128, 4, 5, 255]);
    }

    #[test]
    fn crop_clamps_out_of_bounds() {
        let bgra = gradient_bgra(10, 10);
        let sel = PhysRect::new(8, 8, 10, 10); // 越界
        let rgba = crop_bgra_to_rgba(&bgra, 10, 10, &sel).unwrap();
        assert_eq!(rgba.len(), 2 * 2 * 4, "应裁剪为 2x2");
    }

    #[test]
    fn crop_fully_outside_errors() {
        let bgra = gradient_bgra(10, 10);
        assert!(crop_bgra_to_rgba(&bgra, 10, 10, &PhysRect::new(100, 100, 5, 5)).is_err());
    }

    #[test]
    fn png_roundtrip() {
        let rgba = gradient_bgra(8, 6);
        let mut rgba = rgba.clone();
        bgra_to_rgba_inplace(&mut rgba);
        let png = encode_png(&rgba, 8, 6).unwrap();
        let dec = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(dec.as_raw(), &rgba);
    }

    #[test]
    fn composite_full_and_partial_alpha() {
        let mut base = vec![0, 0, 255, 255]; // 蓝
        let over = vec![255, 0, 0, 255]; // 不透明红
        composite_over(&mut base, &over);
        assert_eq!(base, vec![255, 0, 0, 255]);

        let mut base = vec![0, 0, 255, 255];
        let over = vec![255, 0, 0, 128]; // 半透明红
        composite_over(&mut base, &over);
        // (255*128 + 0*127)/255 ≈ 128；蓝通道 (0*128+255*127)/255 ≈ 127
        assert_eq!(base[0], 128);
        assert_eq!(base[2], 127);
        assert_eq!(base[3], 255);
    }

    #[test]
    fn composite_annotation_png_end_to_end() {
        // 底图 4x4 纯蓝
        let mut base = vec![0u8; 4 * 4 * 4];
        for px in base.chunks_exact_mut(4) {
            px.copy_from_slice(&[0, 0, 255, 255]);
        }
        // 标注层 4x4：左半透明红、右全透明
        let mut layer = vec![0u8; 4 * 4 * 4];
        for y in 0..4 {
            for x in 0..4 {
                let i = (y * 4 + x) as usize * 4;
                if x < 2 {
                    layer[i..i + 4].copy_from_slice(&[255, 0, 0, 255]);
                }
            }
        }
        let png = encode_png(&layer, 4, 4).unwrap();
        let out = composite_annotation_png(base, 4, 4, &png).unwrap();
        // x=0,1 为红（x<2），x=2,3 保持底色蓝
        assert_eq!(&out[0..4], &[255, 0, 0, 255]);
        assert_eq!(&out[4..8], &[255, 0, 0, 255]);
        assert_eq!(&out[8..12], &[0, 0, 255, 255]);
    }

    #[test]
    fn jpeg_encodes() {
        use image::GenericImageView;
        let mut rgba = gradient_bgra(16, 16);
        bgra_to_rgba_inplace(&mut rgba);
        let jpg = encode_jpeg(&rgba, 16, 16, 92).unwrap();
        assert!(!jpg.is_empty());
        let dec = image::load_from_memory(&jpg).unwrap();
        assert_eq!(dec.dimensions(), (16, 16));
    }

    #[test]
    fn size_mismatch_rejected() {
        assert!(encode_png(&[0; 10], 4, 4).is_err());
    }
}
