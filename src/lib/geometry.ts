/**
 * 纯几何逻辑（无 DOM 依赖，全部可单测）。
 *
 * 坐标约定（开发计划 5.2.1 坐标纪律）：
 * - IPC 边界与导出一律物理像素（虚拟屏坐标系）；
 * - 本模块同时服务物理（选区/命中）与逻辑（工具条/手柄）两类调用方，
 *   函数本身与单位无关，由调用方保证一致。
 */

import type { Annotation, Pt, Rect, ToolKind } from './types';

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

// ---------- 标注对象编辑（对象化：命中 / 包围盒 / 移动 / 缩放） ----------

/** 标记笔/马赛克为效果类标注（高亮/像素化依附于底层画面）：禁止改尺寸，
 *  仅允许移动与改笔宽（厚度/块径走工具条线宽控件）。 */
export function isScalableKind(kind: ToolKind): boolean {
  return kind !== 'highlighter' && kind !== 'mosaic';
}

/** 点到线段的最近距离。 */
export function distToSegment(p: Pt, a: Pt, b: Pt): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const len2 = dx * dx + dy * dy;
  if (len2 === 0) return dist(p, a);
  let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / len2;
  t = Math.max(0, Math.min(1, t));
  return dist(p, { x: a.x + t * dx, y: a.y + t * dy });
}

/**
 * 标注包围盒（物理像素；用于选中指示框与缩放锚点）。
 * 笔画类外扩半线宽；text 需提前把测量尺寸写入 rect.w/h；
 * step 为圆心外接方框（半径与绘制一致：fontSizeForWidth(width)×dpr×0.8）。
 */
export function annBBox(a: Annotation, dpr = 1): Rect {
  const empty = { x: 0, y: 0, w: 0, h: 0 };
  const pad = (a.width * dpr) / 2;
  switch (a.kind) {
    case 'rect':
    case 'ellipse':
      return a.rect
        ? { x: a.rect.x - pad, y: a.rect.y - pad, w: a.rect.w + pad * 2, h: a.rect.h + pad * 2 }
        : empty;
    case 'text':
      return a.rect ? { ...a.rect } : empty;
    case 'arrow': {
      if (!a.line) return empty;
      const x = Math.min(a.line.x1, a.line.x2) - pad;
      const y = Math.min(a.line.y1, a.line.y2) - pad;
      return {
        x,
        y,
        w: Math.abs(a.line.x2 - a.line.x1) + pad * 2,
        h: Math.abs(a.line.y2 - a.line.y1) + pad * 2,
      };
    }
    case 'pen':
    case 'highlighter':
    case 'mosaic': {
      if (!a.points || a.points.length === 0) return empty;
      let minX = Infinity;
      let minY = Infinity;
      let maxX = -Infinity;
      let maxY = -Infinity;
      for (const q of a.points) {
        minX = Math.min(minX, q.x);
        minY = Math.min(minY, q.y);
        maxX = Math.max(maxX, q.x);
        maxY = Math.max(maxY, q.y);
      }
      return { x: minX - pad, y: minY - pad, w: maxX - minX + pad * 2, h: maxY - minY + pad * 2 };
    }
    case 'step': {
      const r = fontSizeForWidth(a.width) * dpr * 0.8;
      return a.rect ? { x: a.rect.x - r, y: a.rect.y - r, w: r * 2, h: r * 2 } : empty;
    }
  }
}

/**
 * 标注命中测试（坐标物理像素，tol 为逻辑容差 ×dpr）。
 * 矩形/椭圆命中边框条带；笔画/箭头命中轨迹带；文本/标号命中包围区域。
 */
export function hitTestAnnotation(a: Annotation, p: Pt, dpr = 1, tolLogical = 6): boolean {
  const tol = tolLogical * dpr;
  switch (a.kind) {
    case 'rect': {
      if (!a.rect) return false;
      const band = Math.max(tol, (a.width * dpr) / 2 + 2 * dpr);
      const r = a.rect;
      const dx = Math.max(r.x - p.x, 0, p.x - (r.x + r.w));
      const dy = Math.max(r.y - p.y, 0, p.y - (r.y + r.h));
      if (dx > 0 || dy > 0) return Math.hypot(dx, dy) <= band; // 外部距边框
      const inner = Math.min(p.x - r.x, r.x + r.w - p.x, p.y - r.y, r.y + r.h - p.y);
      return inner <= band; // 内部距最近边
    }
    case 'ellipse': {
      if (!a.rect) return false;
      const rx = Math.max(1, a.rect.w / 2);
      const ry = Math.max(1, a.rect.h / 2);
      const cx = a.rect.x + rx;
      const cy = a.rect.y + ry;
      const nx = (p.x - cx) / rx;
      const ny = (p.y - cy) / ry;
      const rr = Math.hypot(nx, ny);
      return Math.abs(rr - 1) <= Math.max(tol, (a.width * dpr) / 2 + 2 * dpr) / Math.max(1, Math.min(rx, ry));
    }
    case 'arrow': {
      if (!a.line) return false;
      const band = Math.max(tol, (a.width * dpr) / 2 + 2 * dpr);
      return distToSegment(p, { x: a.line.x1, y: a.line.y1 }, { x: a.line.x2, y: a.line.y2 }) <= band;
    }
    case 'pen':
    case 'highlighter':
    case 'mosaic': {
      if (!a.points || a.points.length < 2) {
        return a.points?.length === 1 ? dist(p, a.points[0]) <= Math.max(tol, (a.width * dpr) / 2) : false;
      }
      const band = Math.max(tol, (a.width * dpr) / 2 + 2 * dpr);
      for (let i = 1; i < a.points.length; i++) {
        if (distToSegment(p, a.points[i - 1], a.points[i]) <= band) return true;
      }
      return false;
    }
    case 'text': {
      if (!a.rect) return false;
      return inRect({ x: a.rect.x - tol, y: a.rect.y - tol, w: a.rect.w + tol * 2, h: a.rect.h + tol * 2 }, p);
    }
    case 'step': {
      if (!a.rect) return false;
      const r = fontSizeForWidth(a.width) * dpr * 0.8;
      return dist(p, { x: a.rect.x, y: a.rect.y }) <= r + tol;
    }
  }
}

/** 整体平移标注（dx/dy 物理像素）。 */
export function moveAnnotation(a: Annotation, dx: number, dy: number): Annotation {
  const sh = (q: Pt): Pt => ({ x: q.x + dx, y: q.y + dy });
  if (a.rect) return { ...a, rect: { ...sh({ x: a.rect.x, y: a.rect.y }), w: a.rect.w, h: a.rect.h } };
  if (a.line) {
    return { ...a, line: { x1: a.line.x1 + dx, y1: a.line.y1 + dy, x2: a.line.x2 + dx, y2: a.line.y2 + dy } };
  }
  if (a.points) return { ...a, points: a.points.map(sh) };
  return a;
}

/**
 * 以包围盒对角锚点缩放标注（基于拖拽起始快照计算，幂等无漂移）。
 * **任意宽高比**：x/y 两轴独立缩放（各钳制 [0.1, 8]）；文本字号与标号尺寸
 * 取两轴较大比例；标记笔/马赛克的笔宽（高亮厚度/块径）随较大比例同步缩放，
 * 使效果按新尺寸重新施加到覆盖区域。
 */
export function resizeAnnotation(a: Annotation, anchor: Pt, from: Pt, to: Pt): Annotation {
  const fdx = from.x - anchor.x;
  const fdy = from.y - anchor.y;
  if (Math.hypot(fdx, fdy) < 4) return a;
  const clampS = (v: number) => Math.min(8, Math.max(0.1, v));
  const sx = clampS(fdx === 0 ? 1 : (to.x - anchor.x) / fdx);
  const sy = clampS(fdy === 0 ? 1 : (to.y - anchor.y) / fdy);
  const sMax = Math.max(sx, sy);
  const scx = (v: number) => anchor.x + (v - anchor.x) * sx;
  const scy = (v: number) => anchor.y + (v - anchor.y) * sy;
  switch (a.kind) {
    case 'rect':
    case 'ellipse':
      return a.rect
        ? {
            ...a,
            rect: {
              x: scx(a.rect.x),
              y: scy(a.rect.y),
              w: Math.max(4, a.rect.w * sx),
              h: Math.max(4, a.rect.h * sy),
            },
          }
        : a;
    case 'text':
      return a.rect
        ? {
            ...a,
            size: Math.min(120, Math.max(8, (a.size ?? 18) * sMax)),
            rect: {
              x: scx(a.rect.x),
              y: scy(a.rect.y),
              w: Math.max(4, a.rect.w * sMax),
              h: Math.max(4, a.rect.h * sMax),
            },
          }
        : a;
    case 'arrow':
      return a.line
        ? {
            ...a,
            line: {
              x1: scx(a.line.x1),
              y1: scy(a.line.y1),
              x2: scx(a.line.x2),
              y2: scy(a.line.y2),
            },
          }
        : a;
    case 'pen':
      return a.points
        ? { ...a, points: a.points.map((q) => ({ x: scx(q.x), y: scy(q.y) })) }
        : a;
    case 'highlighter':
    case 'mosaic':
      return a.points
        ? {
            ...a,
            width: Math.min(40, Math.max(1, a.width * sMax)),
            points: a.points.map((q) => ({ x: scx(q.x), y: scy(q.y) })),
          }
        : a;
    case 'step':
      return a.rect
        ? {
            ...a,
            width: Math.min(20, Math.max(1, a.width * sMax)),
            rect: { x: scx(a.rect.x), y: scy(a.rect.y), w: 0, h: 0 },
          }
        : a;
  }
}
