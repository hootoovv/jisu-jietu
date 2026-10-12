#!/usr/bin/env python3
"""生成 JetSnap 应用图标（icon.png / icon.ico / store 图标集）。"""
from PIL import Image, ImageDraw
import os

OUT = os.path.join(os.path.dirname(__file__), "..", "src-tauri", "icons")
os.makedirs(OUT, exist_ok=True)

S = 256  # 主尺寸


def rounded_gradient_bg(size: int) -> Image.Image:
    """蓝色渐变圆角方块背景。"""
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    top, bottom = (0x21, 0x96, 0xF3), (0x0D, 0x47, 0xA1)  # #2196F3 -> #0D47A1
    for y in range(size):
        t = y / max(size - 1, 1)
        row = Image.new(
            "RGBA",
            (size, 1),
            tuple(int(a + (b - a) * t) for a, b in zip(top, bottom)) + (255,),
        )
        img.paste(row, (0, y))
    mask = Image.new("L", (size, size), 0)
    md = ImageDraw.Draw(mask)
    md.rounded_rectangle([0, 0, size - 1, size - 1], radius=int(size * 0.22), fill=255)
    img.putalpha(mask)
    return img


def draw_glyph(img: Image.Image) -> None:
    """白色取景框四角括号 + 中心快门圆（截图/聚焦意象）。"""
    d = ImageDraw.Draw(img)
    s = img.width
    lw = max(3, int(s * 0.055))
    m = int(s * 0.24)     # 边距
    L = int(s * 0.18)     # 括号臂长
    white = (255, 255, 255, 235)
    # 左上 / 右上 / 左下 / 右下
    d.line([(m, m + L), (m, m), (m + L, m)], fill=white, width=lw, joint="curve")
    d.line([(s - m - L, m), (s - m, m), (s - m, m + L)], fill=white, width=lw, joint="curve")
    d.line([(m, s - m - L), (m, s - m), (m + L, s - m)], fill=white, width=lw, joint="curve")
    d.line([(s - m - L, s - m), (s - m, s - m), (s - m, s - m - L)], fill=white, width=lw, joint="curve")
    # 快门圆
    r = int(s * 0.13)
    cx = s // 2
    d.ellipse([cx - r, cx - r, cx + r, cx + r], outline=white, width=lw)
    d.ellipse([cx - r // 2.6, cx - r // 2.6, cx + r // 2.6, cx + r // 2.6], fill=white)


master = rounded_gradient_bg(S)
draw_glyph(master)

# icon.png（256）
master.save(os.path.join(OUT, "icon.png"))

# icon.ico（多尺寸，含 256 PNG 压缩条目）
master.save(
    os.path.join(OUT, "icon.ico"),
    sizes=[(256, 256), (64, 64), (48, 48), (32, 32), (16, 16)],
)

# 各尺寸 PNG（供商店/托盘使用）
for size in (32, 128):
    img = master.resize((size, size), Image.LANCZOS)
    img.save(os.path.join(OUT, f"{size}x{size}.png"))

print("icons generated:", os.listdir(OUT))
