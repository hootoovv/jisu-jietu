/**
 * 纯几何逻辑（无 DOM 依赖，全部可单测）。
 *
 * 坐标约定（开发计划 5.2.1 坐标纪律）：
 * - IPC 边界与导出一律物理像素（虚拟屏坐标系）；
 * - 本模块同时服务物理（选区/命中）与逻辑（工具条/手柄）两类调用方，
 *   函数本身与单位无关，由调用方保证一致。
 */

import type { Annotation, Pt, Rect } from './types';

/** 反向拖拽归一化为左上-右下表示。 */
export function normalizeRect(a: Pt, b: Pt): Rect {
  const x = Math.min(a.x, b.x);
  const y = Math.min(a.y, b.y);
  return { x, y, w: Math.abs(a.x - b.x), h: Math.abs(a.y - b.y) };
}

export function inRect(r: Rect, p: Pt): boolean {
  return p.x >= r.x && p.x < r.x + r.w && p.y >= r.y && p.y < r.y + r.h;
}

export function dist(a: Pt, b: Pt): number {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

export function rectCorners(r: Rect): [Pt, Pt, Pt, Pt] {
  return [
    { x: r.x, y: r.y },
    { x: r.x + r.w, y: r.y },
    { x: r.x + r.w, y: r.y + r.h },
    { x: r.x, y: r.y + r.h },
  ];
}

/** 钳制到边界内，并强制最小边长（开发计划 2.3.4）。 */
export function clampRect(r: Rect, bounds: Rect, min = 8): Rect {
  const w = Math.min(Math.max(r.w, min), bounds.w);
  const h = Math.min(Math.max(r.h, min), bounds.h);
  const x = Math.min(Math.max(r.x, bounds.x), bounds.x + bounds.w - w);
  const y = Math.min(Math.max(r.y, bounds.y), bounds.y + bounds.h - h);
  return { x, y, w, h };
}

/** 整体平移选区（保持尺寸），钳制到边界。 */
export function moveRect(r: Rect, dx: number, dy: number, bounds: Rect, min = 8): Rect {
  return clampRect({ x: r.x + dx, y: r.y + dy, w: r.w, h: r.h }, bounds, min);
}

/**
 * 工具条避让定位（开发计划 2.3.5 四规则，优先级从高到低）：
 * 1. 下方 —— 选区底边 + M；
 * 2. 上方 —— 选区顶边 - M - 工具条高；
 * 3. 内置 —— 选区底部内侧（全屏选区的必然归宿）；
 * 4. 兜底 —— 虚拟屏底部。
 */
export type ToolbarStrategy = 'below' | 'above' | 'inside' | 'bottom';

export interface ToolbarPos {
  x: number;
  y: number;
  strategy: ToolbarStrategy;
}

export function placeToolbar(
  sel: Rect,
  bounds: Rect,
  tb: { w: number; h: number },
  margin = 8,
): ToolbarPos {
  const cx = sel.x + sel.w / 2;
  const maxX = bounds.x + bounds.w - tb.w - margin;
  const minX = bounds.x + margin;
  const clampX = (x: number) => Math.max(minX, Math.min(x, maxX));

  // 1. 下方
  const belowY = sel.y + sel.h + margin;
  if (belowY + tb.h <= bounds.y + bounds.h) {
    return { x: clampX(cx - tb.w / 2), y: belowY, strategy: 'below' };
  }
  // 2. 上方
  const aboveY = sel.y - margin - tb.h;
  if (aboveY >= bounds.y) {
    return { x: clampX(cx - tb.w / 2), y: aboveY, strategy: 'above' };
  }
  // 3. 内置（全屏选区：上下都无空间，选区高度必然 >= 工具条高 + 2M）
  const insideY = sel.y + sel.h - margin - tb.h;
  if (sel.h >= tb.h + 2 * margin) {
    return { x: clampX(cx - tb.w / 2), y: insideY, strategy: 'inside' };
  }
  // 4. 兜底：贴虚拟屏底部
  return {
    x: clampX(bounds.x + bounds.w / 2 - tb.w / 2),
    y: bounds.y + bounds.h - margin - tb.h,
    strategy: 'bottom',
  };
}

/** 尺寸读数定位：选区右下角 + 偏移，出屏则翻转（开发计划 2.3.2）。 */
export function badgePosition(
  sel: Rect,
  bounds: Rect,
  badge: { w: number; h: number },
  margin = 8,
): Pt {
  let x = sel.x + sel.w + margin;
  let y = sel.y + sel.h + margin;
  if (x + badge.w > bounds.x + bounds.w) x = sel.x - badge.w - margin;
  if (y + badge.h > bounds.y + bounds.h) y = sel.y - badge.h - margin;
  return { x, y };
}

/** 命中测试：z 序小者优先（顶层在前），含点即中（开发计划 2.3.3）。 */
export function hitTestWindows(
  windows: { rect: Rect; z: number }[],
  p: Pt,
): Rect | null {
  const sorted = [...windows].sort((a, b) => a.z - b.z);
  for (const w of sorted) {
    if (inRect(w.rect, p)) return w.rect;
  }
  return null;
}

/** 箭头两翼端点（开发计划 2.4：头长 max(线宽×3, 10px)，夹角 26°）。 */
export function arrowHead(
  tail: Pt,
  tip: Pt,
  len: number,
  spreadDeg = 26,
): { left: Pt; right: Pt } {
  const ang = Math.atan2(tail.y - tip.y, tail.x - tip.x);
  const s = (spreadDeg * Math.PI) / 180;
  return {
    left: { x: tip.x + len * Math.cos(ang - s), y: tip.y + len * Math.sin(ang - s) },
    right: { x: tip.x + len * Math.cos(ang + s), y: tip.y + len * Math.sin(ang + s) },
  };
}

/** 数字标号派生序号：撤销后按时间顺序自动重排（开发计划 5.2.5）。 */
export function stepNumberMap(annotations: Annotation[]): Map<number, number> {
  const map = new Map<number, number>();
  let n = 0;
  for (const a of annotations) {
    if (a.kind === 'step') {
      n += 1;
      map.set(a.id, n);
    }
  }
  return map;
}

/** 线宽 → 文本字号（预设档 2/4/6/8/12 → 14/18/22/26/32）。 */
export function fontSizeForWidth(width: number): number {
  return Math.min(32, Math.max(14, Math.round(10 + width * 2)));
}

/** 线宽 → 马赛克块尺寸（逻辑像素）。 */
export function mosaicBlockForWidth(width: number): number {
  return Math.max(4, Math.round(width * 4));
}

/** 颜色亮度判断：深色底配白字，浅色底配黑字（数字标号用）。 */
export function contrastColor(hex: string): string {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return '#FFFFFF';
  const v = parseInt(m[1], 16);
  const r = (v >> 16) & 0xff;
  const g = (v >> 8) & 0xff;
  const b = v & 0xff;
  // ITU-R BT.601 相对亮度
  const lum = 0.299 * r + 0.587 * g + 0.114 * b;
  return lum > 140 ? '#000000' : '#FFFFFF';
}
