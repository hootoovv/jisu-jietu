/**
 * Canvas 渲染层（蒙版 / 选区 / 标注）。
 *
 * 画布尺寸 = 虚拟屏物理像素；所有绘制坐标为物理像素；
 * 线宽/字号等「逻辑像素样式值」在入口处 ×dpr 转换。
 * 导出合成复用同一套绘制函数（offset 平移到选区原点），保证所见即所得。
 */

import type { Annotation, Pt, Rect } from './types';
import {
  annBBox,
  arrowHead,
  contrastColor,
  fontSizeForWidth,
  mosaicBlockForWidth,
  stepNumberMap,
} from './geometry';
import { HIGHLIGHTER_ALPHA, MASK_OPACITY, SEL_BORDER } from './constants';

const FONT_STACK = '"Segoe UI", "Microsoft YaHei", system-ui, sans-serif';

// ---------- 蒙版 ----------

export interface MaskOptions {
  selection: Rect | null;
  /** 悬停预高亮窗口（仅在无选区时显示） */
  hover: Rect | null;
  dpr: number;
}

export function drawMask(ctx: CanvasRenderingContext2D, opts: MaskOptions): void {
  const { width: W, height: H } = ctx.canvas;
  ctx.clearRect(0, 0, W, H);

  const { selection, hover, dpr } = opts;
  if (selection) {
    // 选区外 50% 压暗，选区内挖洞全透明（evenodd）
    ctx.fillStyle = `rgba(0,0,0,${MASK_OPACITY})`;
    ctx.beginPath();
    ctx.rect(0, 0, W, H);
    ctx.rect(selection.x, selection.y, selection.w, selection.h);
    ctx.fill('evenodd');
    // 选区边框
    ctx.strokeStyle = SEL_BORDER;
    ctx.lineWidth = Math.max(1, dpr);
    ctx.strokeRect(selection.x, selection.y, selection.w, selection.h);
  } else {
    ctx.fillStyle = `rgba(0,0,0,${MASK_OPACITY})`;
    ctx.fillRect(0, 0, W, H);
    if (hover) {
      // 悬停预高亮：轻微提亮 + 亮边框（预览即所得，2.3.3）
      // 边框 2px 且提亮蓝色：50% 压暗背景下需高可见
      ctx.fillStyle = 'rgba(255,255,255,0.10)';
      ctx.fillRect(hover.x, hover.y, hover.w, hover.h);
      ctx.strokeStyle = 'rgba(90,169,255,1)';
      ctx.lineWidth = 2 * dpr;
      ctx.strokeRect(hover.x, hover.y, hover.w, hover.h);
    }
  }
}

// ---------- 标注层 ----------

export interface AnnotateOptions {
  annotations: Annotation[];
  /** 绘制中的预览项（半透明） */
  drawing?: Annotation | null;
  /** 当前选区（用于裁剪显示；null 表示不裁剪） */
  selection: Rect | null;
  /** 绘制坐标偏移：显示层 {0,0}，导出层 {selection.x, selection.y} */
  offset: Pt;
  /** 马赛克纹理源（冻结画面） */
  img: HTMLImageElement | null;
  /** 虚拟屏（物理像素，马赛克纹理尺寸） */
  virtual: Rect;
  dpr: number;
  /** 是否裁剪到选区（显示层 true；导出层画布本身就是选区尺寸，false） */
  clip: boolean;
  /** 选中的标注 id（显示层画选中指示框；导出层必须为 null） */
  selectedId?: number | null;
}

export function drawAnnotations(ctx: CanvasRenderingContext2D, opts: AnnotateOptions): void {
  const { width: W, height: H } = ctx.canvas;
  ctx.clearRect(0, 0, W, H);

  const { selection, offset } = opts;
  if (opts.clip && selection) {
    ctx.save();
    ctx.beginPath();
    ctx.rect(selection.x - offset.x, selection.y - offset.y, selection.w, selection.h);
    ctx.clip();
  }

  const nums = stepNumberMap(opts.annotations);
  for (const a of opts.annotations) {
    drawAnnotation(ctx, a, opts, nums.get(a.id) ?? 0, false);
  }
  if (opts.drawing) {
    drawAnnotation(ctx, opts.drawing, opts, nums.get(opts.drawing.id) ?? 0, true);
  }

  // 选中指示框（虚线包围盒 + 四角手柄；随 offset 平移以兼容导出坐标系，但导出层不应传 selectedId）
  if (opts.selectedId != null) {
    const sel = opts.annotations.find((a) => a.id === opts.selectedId);
    if (sel) {
      const bb = annBBox(sel, opts.dpr);
      const x = bb.x - offset.x;
      const y = bb.y - offset.y;
      ctx.save();
      ctx.strokeStyle = '#3d82f6';
      ctx.lineWidth = Math.max(1, opts.dpr);
      ctx.setLineDash([5 * opts.dpr, 4 * opts.dpr]);
      ctx.strokeRect(x, y, bb.w, bb.h);
      ctx.setLineDash([]);
      const hs = 7 * opts.dpr; // 手柄边长
      for (const [cx, cy] of [
        [x, y],
        [x + bb.w, y],
        [x + bb.w, y + bb.h],
        [x, y + bb.h],
      ]) {
        ctx.fillStyle = '#fff';
        ctx.strokeStyle = '#3d82f6';
        ctx.lineWidth = 1.5 * opts.dpr;
        ctx.fillRect(cx - hs / 2, cy - hs / 2, hs, hs);
        ctx.strokeRect(cx - hs / 2, cy - hs / 2, hs, hs);
      }
      ctx.restore();
    }
  }

  if (opts.clip && selection) {
    ctx.restore();
  }
}

/** 测量文本标注的物理像素包围尺寸（与绘制同款字体/行高：600 字重，行高 1.25）。 */
export function measureTextSize(text: string, width: number, dpr: number): { w: number; h: number } {
  const fontPx = fontSizeForWidth(width) * dpr;
  const lines = text.split('\n');
  const c = getMeasureCanvas();
  const ctx = c.getContext('2d');
  if (!ctx) return { w: fontPx * lines.length * 2, h: fontPx * 1.25 * lines.length };
  ctx.font = `600 ${fontPx}px ${FONT_STACK}`;
  let w = 0;
  for (const line of lines) w = Math.max(w, ctx.measureText(line).width);
  return { w: Math.ceil(w), h: Math.ceil(fontPx * 1.25 * lines.length) };
}

let measureCanvas: HTMLCanvasElement | null = null;
function getMeasureCanvas(): HTMLCanvasElement {
  if (!measureCanvas) measureCanvas = document.createElement('canvas');
  return measureCanvas;
}

function drawAnnotation(
  ctx: CanvasRenderingContext2D,
  a: Annotation,
  opts: AnnotateOptions,
  stepNo: number,
  isPreview: boolean,
): void {
  const { offset, dpr } = opts;
  const lw = Math.max(1, a.width * dpr);
  ctx.save();
  if (isPreview) {
    ctx.globalAlpha = 0.75;
  }
  // 统一 1px 阴影增强可见性（开发计划 2.4，仅形状类）
  if (a.kind === 'rect' || a.kind === 'ellipse' || a.kind === 'arrow') {
    ctx.shadowColor = 'rgba(0,0,0,0.55)';
    ctx.shadowOffsetX = dpr;
    ctx.shadowOffsetY = dpr;
  }

  switch (a.kind) {
    case 'rect': {
      if (!a.rect) break;
      ctx.strokeStyle = a.color;
      ctx.lineWidth = lw;
      ctx.strokeRect(a.rect.x - offset.x, a.rect.y - offset.y, a.rect.w, a.rect.h);
      break;
    }
    case 'ellipse': {
      if (!a.rect) break;
      ctx.strokeStyle = a.color;
      ctx.lineWidth = lw;
      ctx.beginPath();
      ctx.ellipse(
        a.rect.x - offset.x + a.rect.w / 2,
        a.rect.y - offset.y + a.rect.h / 2,
        Math.max(1, a.rect.w / 2 - lw / 2),
        Math.max(1, a.rect.h / 2 - lw / 2),
        0,
        0,
        Math.PI * 2,
      );
      ctx.stroke();
      break;
    }
    case 'arrow': {
      if (!a.line) break;
      const tail: Pt = { x: a.line.x1, y: a.line.y1 };
      const tip: Pt = { x: a.line.x2, y: a.line.y2 };
      const headLen = Math.max(lw * 3, 10 * dpr);
      const { left, right } = arrowHead(tail, tip, headLen);
      ctx.strokeStyle = a.color;
      ctx.fillStyle = a.color;
      ctx.lineWidth = lw;
      ctx.lineCap = 'round';
      // 线身止于箭头底部，避免粗线穿出箭头
      const ux = tip.x - tail.x;
      const uy = tip.y - tail.y;
      const len = Math.hypot(ux, uy) || 1;
      const bodyEnd: Pt = {
        x: tail.x + ((len - headLen * 0.8) / len) * ux,
        y: tail.y + ((len - headLen * 0.8) / len) * uy,
      };
      if (len > headLen * 1.2) {
        ctx.beginPath();
        ctx.moveTo(tail.x - offset.x, tail.y - offset.y);
        ctx.lineTo(bodyEnd.x - offset.x, bodyEnd.y - offset.y);
        ctx.stroke();
      }
      ctx.beginPath();
      ctx.moveTo(tip.x - offset.x, tip.y - offset.y);
      ctx.lineTo(left.x - offset.x, left.y - offset.y);
      ctx.lineTo(right.x - offset.x, right.y - offset.y);
      ctx.closePath();
      ctx.fill();
      break;
    }
    case 'pen': {
      if (!a.points || a.points.length === 0) break;
      ctx.strokeStyle = a.color;
      ctx.lineWidth = lw;
      ctx.lineCap = 'round';
      ctx.lineJoin = 'round';
      ctx.beginPath();
      a.points.forEach((p, i) => {
        const x = p.x - offset.x;
        const y = p.y - offset.y;
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      });
      ctx.stroke();
      break;
    }
    case 'highlighter': {
      // 标记笔：宽笔触半透明（文字高亮语义）。单次 stroke 内不自叠加，
      // 多笔重叠自然加深，与真实荧光笔一致
      if (!a.points || a.points.length === 0) break;
      ctx.globalAlpha = HIGHLIGHTER_ALPHA;
      ctx.strokeStyle = a.color;
      ctx.lineWidth = Math.max(2, lw);
      ctx.lineCap = 'round';
      ctx.lineJoin = 'round';
      ctx.beginPath();
      a.points.forEach((p, i) => {
        const x = p.x - offset.x;
        const y = p.y - offset.y;
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      });
      ctx.stroke();
      break;
    }
    case 'mosaic': {
      if (!a.points || a.points.length === 0 || !opts.img) break;
      drawMosaic(ctx, a, opts);
      break;
    }
    case 'text': {
      if (!a.rect || !a.text) break;
      const fontPx = fontSizeForWidth(a.width) * dpr;
      ctx.fillStyle = a.color;
      ctx.font = `600 ${fontPx}px ${FONT_STACK}`;
      ctx.textBaseline = 'top';
      const lines = a.text.split('\n');
      const lineH = fontPx * 1.25;
      lines.forEach((line, i) => {
        ctx.fillText(line, a.rect!.x - offset.x, a.rect!.y - offset.y + i * lineH);
      });
      break;
    }
    case 'step': {
      if (!a.rect) break;
      const fontPx = fontSizeForWidth(a.width) * dpr;
      const r = fontPx * 0.8;
      const cx = a.rect.x - offset.x;
      const cy = a.rect.y - offset.y;
      ctx.fillStyle = a.color;
      ctx.beginPath();
      ctx.arc(cx, cy, r, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = contrastColor(a.color);
      ctx.font = `700 ${Math.round(fontPx * 0.9)}px ${FONT_STACK}`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText(String(stepNo), cx, cy + fontPx * 0.05);
      ctx.textAlign = 'start';
      break;
    }
  }
  ctx.restore();
}

// ---------- 马赛克 ----------

/** 像素化纹理缓存：key = 画布尺寸@块尺寸。会话结束调用 invalidateMosaicCache。 */
const mosaicCache = new Map<string, HTMLCanvasElement>();

export function invalidateMosaicCache(): void {
  mosaicCache.clear();
}

function getMosaicTexture(img: HTMLImageElement, virtual: Rect, block: number): HTMLCanvasElement | null {
  const key = `${virtual.w}x${virtual.h}@${block}`;
  const hit = mosaicCache.get(key);
  if (hit) return hit;
  if (img.naturalWidth === 0) return null;

  // 降采样 → 升采样（关闭平滑，产生块状像素）
  const dw = Math.max(1, Math.round(virtual.w / block));
  const dh = Math.max(1, Math.round(virtual.h / block));
  const small = document.createElement('canvas');
  small.width = dw;
  small.height = dh;
  const sctx = small.getContext('2d');
  if (!sctx) return null;
  sctx.imageSmoothingEnabled = true;
  sctx.drawImage(img, 0, 0, virtual.w, virtual.h, 0, 0, dw, dh);

  const out = document.createElement('canvas');
  out.width = virtual.w;
  out.height = virtual.h;
  const octx = out.getContext('2d');
  if (!octx) return null;
  octx.imageSmoothingEnabled = false;
  octx.drawImage(small, 0, 0, dw, dh, 0, 0, virtual.w, virtual.h);

  mosaicCache.set(key, out);
  return out;
}

/** 马赛克：笔刷路径区域贴像素化纹理（开发计划 5.2.4）。 */
function drawMosaic(ctx: CanvasRenderingContext2D, a: Annotation, opts: AnnotateOptions): void {
  const { offset, dpr, virtual } = opts;
  const img = opts.img!;
  const block = Math.max(4, mosaicBlockForWidth(a.width) * dpr);
  const texture = getMosaicTexture(img, virtual, block);
  if (!texture || !a.points || a.points.length === 0) return;

  // 笔刷路径包围盒（含块半径 padding），离屏合成后贴回，避免整屏操作
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const p of a.points) {
    minX = Math.min(minX, p.x);
    minY = Math.min(minY, p.y);
    maxX = Math.max(maxX, p.x);
    maxY = Math.max(maxY, p.y);
  }
  const pad = block;
  minX = Math.floor(minX - pad);
  minY = Math.floor(minY - pad);
  const bw = Math.ceil(maxX + pad) - minX;
  const bh = Math.ceil(maxY + pad) - minY;
  if (bw <= 0 || bh <= 0) return;

  const off = document.createElement('canvas');
  off.width = bw;
  off.height = bh;
  const c = off.getContext('2d');
  if (!c) return;

  // 1. 画粗线笔刷形状
  c.lineCap = 'round';
  c.lineJoin = 'round';
  c.lineWidth = block;
  c.strokeStyle = '#000';
  c.beginPath();
  a.points.forEach((p, i) => {
    const x = p.x - minX;
    const y = p.y - minY;
    if (i === 0) c.moveTo(x, y);
    else c.lineTo(x, y);
  });
  if (a.points.length === 1) {
    c.lineTo(a.points[0].x - minX + 0.01, a.points[0].y - minY);
  }
  c.stroke();
  // 2. source-in 贴纹理（保留笔刷形状内的像素化内容）
  c.globalCompositeOperation = 'source-in';
  c.imageSmoothingEnabled = false;
  c.drawImage(texture, minX, minY, bw, bh, 0, 0, bw, bh);
  // 3. 贴回主画布（按 offset 平移）
  ctx.drawImage(off, minX - offset.x, minY - offset.y);
}

// ---------- 导出合成 ----------

/** 将标注层渲染为选区尺寸的 PNG 字节（发给 Rust 与原始 BGRA 合成）。 */
export async function composeAnnotationsPng(
  annotations: Annotation[],
  selection: Rect,
  virtual: Rect,
  img: HTMLImageElement | null,
  dpr: number,
): Promise<Uint8Array> {
  const c = document.createElement('canvas');
  c.width = Math.max(1, selection.w);
  c.height = Math.max(1, selection.h);
  const ctx = c.getContext('2d');
  if (!ctx) throw new Error('无法创建导出画布');
  drawAnnotations(ctx, {
    annotations,
    selection,
    offset: { x: selection.x, y: selection.y },
    img,
    virtual,
    dpr,
    clip: false,
  });
  const blob = await new Promise<Blob | null>((resolve) => c.toBlob(resolve, 'image/png'));
  if (!blob) throw new Error('PNG 编码失败');
  return new Uint8Array(await blob.arrayBuffer());
}
