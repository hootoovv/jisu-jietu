/** geometry.ts 纯逻辑单测 —— 对应开发计划 2.3 / 5.2 各交互规范。 */

import { describe, expect, it } from 'vitest';

import {
  annBBox,
  arrowHead,
  badgePosition,
  clampRect,
  contrastColor,
  distToSegment,
  fontSizeForWidth,
  hitTestAnnotation,
  hitTestWindows,
  inRect,
  isScalableKind,
  mosaicBlockForWidth,
  moveAnnotation,
  moveRect,
  normalizeRect,
  placeToolbar,
  resizeAnnotation,
  stepNumberMap,
} from './geometry';
import type { Annotation } from './types';

const V = { x: -1920, y: 0, w: 3840, h: 1080 }; // 双屏虚拟屏（负原点）
const TB = { w: 486, h: 44 };

describe('normalizeRect 反向拖拽归一化', () => {
  it('从右下往左上拖拽', () => {
    const r = normalizeRect({ x: 500, y: 500 }, { x: 100, y: 200 });
    expect(r).toEqual({ x: 100, y: 200, w: 400, h: 300 });
  });
  it('正向拖拽', () => {
    const r = normalizeRect({ x: 100, y: 200 }, { x: 500, y: 500 });
    expect(r).toEqual({ x: 100, y: 200, w: 400, h: 300 });
  });
});

describe('clampRect 选区钳制', () => {
  it('越界位置拉回边界内', () => {
    const c = clampRect({ x: 3000, y: 2000, w: 100, h: 100 }, V);
    expect(c.x + c.w).toBeLessThanOrEqual(V.x + V.w);
    expect(c.y + c.h).toBeLessThanOrEqual(V.y + V.h);
  });
  it('最小尺寸强制（8px）', () => {
    const c = clampRect({ x: 0, y: 0, w: 2, h: 2 }, V);
    expect(c.w).toBe(8);
    expect(c.h).toBe(8);
  });
  it('反向拖出边界：归一化后钳制（FR-03 边界分支）', () => {
    const r = normalizeRect({ x: 1919, y: 1079 }, { x: -3000, y: -500 });
    const c = clampRect(r, V);
    expect(c.x).toBeGreaterThanOrEqual(V.x);
    expect(c.y).toBeGreaterThanOrEqual(V.y);
  });
});

describe('moveRect 整体移动', () => {
  it('平移后贴边不越界', () => {
    const m = moveRect({ x: 1800, y: 1000, w: 100, h: 50 }, 500, 500, V);
    expect(m.x + m.w).toBeLessThanOrEqual(V.x + V.w);
    expect(m.w).toBe(100);
    expect(m.h).toBe(50);
  });
});

describe('placeToolbar 工具条避让（开发计划 2.3.5 四规则）', () => {
  it('规则1：空间充足时置于选区下方居中', () => {
    const sel = { x: 0, y: 0, w: 800, h: 400 };
    const p = placeToolbar(sel, V, TB);
    expect(p.strategy).toBe('below');
    expect(p.y).toBe(400 + 8);
    expect(p.x).toBe(sel.x + sel.w / 2 - TB.w / 2);
  });

  it('规则1：x 超出左边界时贴边', () => {
    const sel = { x: -1920, y: 0, w: 200, h: 400 };
    const p = placeToolbar(sel, V, TB);
    expect(p.x).toBe(V.x + 8);
  });

  it('规则2：选区贴屏底（非全屏）翻转到上方', () => {
    const sel = { x: 0, y: 800, w: 800, h: 260 };
    const p = placeToolbar(sel, V, TB);
    expect(p.strategy).toBe('above');
    expect(p.y).toBe(sel.y - 8 - TB.h);
  });

  it('规则3：全屏选区置于选区底部内侧（FR-07 核心场景）', () => {
    const sel = { ...V };
    const p = placeToolbar(sel, V, TB);
    expect(p.strategy).toBe('inside');
    expect(p.y).toBe(V.y + V.h - 8 - TB.h);
    // 内置后必须完全可见
    expect(p.y + TB.h).toBeLessThanOrEqual(V.y + V.h);
  });

  it('规则3：选区高度不足时上翻仍优先于内置', () => {
    // 下方不可行（bottom+8+44 超屏），上方可行，h=50 < 44+16 不满足内置
    const sel = { x: 0, y: 1000, w: 800, h: 50 };
    const p = placeToolbar(sel, V, TB);
    expect(p.strategy).toBe('above');
  });

  it('规则4：退化虚拟屏（小于工具条空间）时兑底贴屏底', () => {
    // 正常尺寸屏幕上下两侧数学上必有一侧可行；此用例覆盖防御性兑底分支
    const tiny = { x: 0, y: 0, w: 400, h: 46 };
    const sel = { x: 100, y: 0, w: 200, h: 10 };
    const p = placeToolbar(sel, tiny, TB);
    expect(p.strategy).toBe('bottom');
    expect(p.y).toBe(tiny.y + tiny.h - 8 - TB.h);
  });

  it('负坐标虚拟屏：单屏 1920x1080 原点 0', () => {
    const bounds = { x: 0, y: 0, w: 1920, h: 1080 };
    const sel = { x: 100, y: 100, w: 500, h: 300 };
    const p = placeToolbar(sel, bounds, TB);
    expect(p.strategy).toBe('below');
    expect(p.x).toBeGreaterThan(0);
  });
});

describe('badgePosition 尺寸读数防出屏', () => {
  it('常规位置在右下角外 8px', () => {
    const p = badgePosition({ x: 0, y: 0, w: 100, h: 100 }, V, { w: 78, h: 24 });
    expect(p).toEqual({ x: 108, y: 108 });
  });
  it('右下贴边时翻转到左上', () => {
    const sel = { x: V.x + V.w - 100, y: V.y + V.h - 100, w: 100, h: 100 };
    const p = badgePosition(sel, V, { w: 78, h: 24 });
    expect(p.x + 78).toBeLessThanOrEqual(sel.x);
    expect(p.y + 24).toBeLessThanOrEqual(sel.y);
  });
});

describe('hitTestWindows 智能窗口识别（FR-04）', () => {
  const wins = [
    { rect: { x: 0, y: 0, w: 800, h: 600 }, z: 5 }, // 底层
    { rect: { x: 100, y: 100, w: 400, h: 300 }, z: 1 }, // 顶层（嵌套）
    { rect: { x: 900, y: 0, w: 200, h: 100 }, z: 3 },
  ];

  it('嵌套窗口取 z 序更高者', () => {
    const hit = hitTestWindows(wins, { x: 200, y: 200 });
    expect(hit).toEqual({ x: 100, y: 100, w: 400, h: 300 });
  });
  it('非嵌套点命中所属窗口', () => {
    const hit = hitTestWindows(wins, { x: 700, y: 500 });
    expect(hit).toEqual({ x: 0, y: 0, w: 800, h: 600 });
  });
  it('未命中返回 null', () => {
    expect(hitTestWindows(wins, { x: 5000, y: 5000 })).toBeNull();
  });
  it('负坐标副屏上的窗口', () => {
    const w2 = [{ rect: { x: -1920, y: 0, w: 1920, h: 1080 }, z: 0 }];
    expect(hitTestWindows(w2, { x: -1000, y: 500 })).toEqual(w2[0].rect);
  });
});

describe('arrowHead 箭头几何', () => {
  it('两翼关于轴线对称且距端点为头长', () => {
    const tail = { x: 0, y: 100 };
    const tip = { x: 100, y: 100 };
    const { left, right } = arrowHead(tail, tip, 10);
    const dl = Math.hypot(left.x - tip.x, left.y - tip.y);
    const dr = Math.hypot(right.x - tip.x, right.y - tip.y);
    expect(dl).toBeCloseTo(10);
    expect(dr).toBeCloseTo(10);
    // 对称：两翼相对轴线（y=100）镜像
    expect(left.y + right.y).toBeCloseTo(200);
  });
});

describe('stepNumberMap 数字标号重排（FR-14）', () => {
  const mk = (id: number, kind: Annotation['kind']): Annotation => ({
    id,
    kind,
    color: '#000000',
    width: 4,
  });

  it('初始顺序编号', () => {
    const list = [mk(1, 'rect'), mk(2, 'step'), mk(3, 'step'), mk(4, 'pen'), mk(5, 'step')];
    const m = stepNumberMap(list);
    expect(m.get(2)).toBe(1);
    expect(m.get(3)).toBe(2);
    expect(m.get(5)).toBe(3);
    expect(m.has(1)).toBe(false);
  });

  it('撤销中间标号后自动重排（无空洞）', () => {
    const list = [mk(2, 'step'), mk(5, 'step')]; // 3 号被撤销
    const m = stepNumberMap(list);
    expect(m.get(2)).toBe(1);
    expect(m.get(5)).toBe(2);
  });
});

describe('样式映射', () => {
  it('线宽→字号：预设档 2/4/6/8/12 → 14/18/22/26/32', () => {
    expect(fontSizeForWidth(2)).toBe(14);
    expect(fontSizeForWidth(4)).toBe(18);
    expect(fontSizeForWidth(6)).toBe(22);
    expect(fontSizeForWidth(8)).toBe(26);
    expect(fontSizeForWidth(12)).toBe(32);
  });
  it('自定义线宽映射到 14~32 区间', () => {
    expect(fontSizeForWidth(1)).toBe(14);
    expect(fontSizeForWidth(20)).toBe(32);
  });
  it('线宽→马赛克块 = 线宽×4（默认档 4 → 16）', () => {
    expect(mosaicBlockForWidth(4)).toBe(16);
    expect(mosaicBlockForWidth(1)).toBe(4);
  });
  it('对比色：深色配白、浅色配黑', () => {
    expect(contrastColor('#000000')).toBe('#FFFFFF');
    expect(contrastColor('#FFFFFF')).toBe('#000000');
    expect(contrastColor('#F7C948')).toBe('#000000'); // 亮黄配黑
    expect(contrastColor('#2196F3')).toBe('#FFFFFF'); // 材质蓝亮度~126 配白
  });
});

describe('inRect 基础', () => {
  it('含左上不含右下（半开区间）', () => {
    const r = { x: 10, y: 10, w: 100, h: 50 };
    expect(inRect(r, { x: 10, y: 10 })).toBe(true);
    expect(inRect(r, { x: 109, y: 59 })).toBe(true);
    expect(inRect(r, { x: 110, y: 10 })).toBe(false);
    expect(inRect(r, { x: 10, y: 60 })).toBe(false);
  });
});

describe('标注对象编辑：distToSegment', () => {
  it('点到水平线段的距离', () => {
    const a = { x: 0, y: 0 };
    const b = { x: 10, y: 0 };
    expect(distToSegment({ x: 5, y: 3 }, a, b)).toBe(3);
    expect(distToSegment({ x: 5, y: -4 }, a, b)).toBe(4);
  });
  it('线段外投影钳制到端点', () => {
    const a = { x: 0, y: 0 };
    const b = { x: 10, y: 0 };
    expect(distToSegment({ x: 20, y: 5 }, a, b)).toBe(Math.hypot(10, 5));
    expect(distToSegment({ x: -3, y: 0 }, a, b)).toBe(3);
  });
  it('零长线段退化为点距', () => {
    const p0 = { x: 1, y: 1 };
    expect(distToSegment({ x: 4, y: 5 }, p0, p0)).toBe(5);
  });
});

describe('标注对象编辑：annBBox / hitTestAnnotation', () => {
  it('矩形标注命中边框条带、中心不命中', () => {
    const ann: Annotation = { id: 1, kind: 'rect', color: '#f00', width: 4, rect: { x: 100, y: 100, w: 200, h: 100 } };
    expect(hitTestAnnotation(ann, { x: 100, y: 150 }, 1)).toBe(true); // 左边框
    expect(hitTestAnnotation(ann, { x: 300, y: 200 }, 1)).toBe(true); // 右下角
    expect(hitTestAnnotation(ann, { x: 200, y: 150 }, 1)).toBe(false); // 内部深处
    expect(hitTestAnnotation(ann, { x: 80, y: 120 }, 1)).toBe(false); // 左侧外 20px
  });
  it('笔画命中轨迹带', () => {
    const ann: Annotation = {
      id: 2, kind: 'pen', color: '#0f0', width: 4,
      points: [{ x: 0, y: 0 }, { x: 100, y: 0 }],
    };
    expect(hitTestAnnotation(ann, { x: 50, y: 4 }, 1)).toBe(true);
    expect(hitTestAnnotation(ann, { x: 50, y: 30 }, 1)).toBe(false);
  });
  it('文本命中测量包围盒', () => {
    const ann: Annotation = { id: 3, kind: 'text', color: '#00f', width: 4, text: 'ab', rect: { x: 50, y: 50, w: 40, h: 24 } };
    expect(hitTestAnnotation(ann, { x: 60, y: 60 }, 1)).toBe(true);
    expect(hitTestAnnotation(ann, { x: 200, y: 200 }, 1)).toBe(false);
  });
  it('标号命中圆域', () => {
    const ann: Annotation = { id: 4, kind: 'step', color: '#f0f', width: 4, rect: { x: 100, y: 100, w: 0, h: 0 } };
    expect(hitTestAnnotation(ann, { x: 105, y: 100 }, 1)).toBe(true);
    expect(hitTestAnnotation(ann, { x: 200, y: 100 }, 1)).toBe(false);
  });
  it('annBBox 笔画外扩半线宽', () => {
    const ann: Annotation = {
      id: 5, kind: 'pen', color: '#000', width: 8,
      points: [{ x: 10, y: 10 }, { x: 110, y: 10 }],
    };
    const bb = annBBox(ann, 1); // 半线宽 4
    expect(bb).toEqual({ x: 6, y: 6, w: 108, h: 8 });
  });
});

describe('标注对象编辑：moveAnnotation / resizeAnnotation', () => {
  it('平移矩形与笔画', () => {
    const rect: Annotation = { id: 1, kind: 'rect', color: '#f00', width: 4, rect: { x: 10, y: 10, w: 20, h: 30 } };
    expect(moveAnnotation(rect, 5, -3).rect).toEqual({ x: 15, y: 7, w: 20, h: 30 });
    const pen: Annotation = { id: 2, kind: 'pen', color: '#0f0', width: 4, points: [{ x: 1, y: 2 }, { x: 3, y: 4 }] };
    expect(moveAnnotation(pen, 10, 10).points).toEqual([{ x: 11, y: 12 }, { x: 13, y: 14 }]);
  });
  it('对角锚点放大 2 倍（矩形，两轴同比例）', () => {
    const ann: Annotation = { id: 1, kind: 'rect', color: '#f00', width: 4, rect: { x: 100, y: 100, w: 50, h: 50 } };
    const anchor = { x: 100, y: 100 }; // 左上锚，拖右下角
    const out = resizeAnnotation(ann, anchor, { x: 150, y: 150 }, { x: 200, y: 200 }) as Annotation;
    expect(out.rect?.w).toBe(100);
    expect(out.rect?.h).toBe(100);
    expect(out.rect?.x).toBe(100); // 锚点不动
    expect(out.rect?.y).toBe(100);
  });
  it('任意宽高比：仅横向拉伸时高度不变', () => {
    const ann: Annotation = { id: 1, kind: 'rect', color: '#f00', width: 4, rect: { x: 0, y: 0, w: 100, h: 50 } };
    const out = resizeAnnotation(ann, { x: 0, y: 0 }, { x: 100, y: 50 }, { x: 300, y: 50 }) as Annotation;
    expect(out.rect?.w).toBe(300); // sx=3
    expect(out.rect?.h).toBe(50); // sy=1，纵轴不动
  });
  it('缩放比例逐轴钳制 [0.1, 8]', () => {
    const ann: Annotation = { id: 1, kind: 'rect', color: '#f00', width: 4, rect: { x: 0, y: 0, w: 100, h: 100 } };
    const tiny = resizeAnnotation(ann, { x: 0, y: 0 }, { x: 100, y: 100 }, { x: 1, y: 1 }) as Annotation;
    expect(tiny.rect?.w).toBe(10); // 100 × 0.1
    const huge = resizeAnnotation(ann, { x: 0, y: 0 }, { x: 100, y: 100 }, { x: 100000, y: 100000 }) as Annotation;
    expect(huge.rect?.w).toBe(800);
  });
  it('文本缩放同步字号 size（取两轴较大比例）', () => {
    const ann: Annotation = { id: 3, kind: 'text', color: '#00f', size: 18, text: 'x', rect: { x: 0, y: 0, w: 10, h: 20 } };
    const out = resizeAnnotation(ann, { x: 0, y: 0 }, { x: 100, y: 100 }, { x: 150, y: 150 }) as Annotation;
    expect(out.size).toBe(27);
    const back = resizeAnnotation(out, { x: 0, y: 0 }, { x: 150, y: 150 }, { x: 100, y: 100 }) as Annotation;
    expect(back.size).toBe(18);
  });
  it('标记笔缩放：点位逐轴缩放 + 笔宽（高亮厚度）随较大比例', () => {
    const ann: Annotation = {
      id: 4, kind: 'highlighter', color: '#ff0', width: 16,
      points: [{ x: 10, y: 10 }, { x: 110, y: 10 }],
    };
    const out = resizeAnnotation(ann, { x: 10, y: 10 }, { x: 110, y: 10 }, { x: 210, y: 30 }) as Annotation;
    expect(out.points?.[0]).toEqual({ x: 10, y: 10 });
    expect(out.points?.[1]?.x).toBe(210); // sx=2
    expect(out.width).toBe(32); // 宽度 ×2
  });
  it('马赛克缩放同步块径档', () => {
    const ann: Annotation = {
      id: 5, kind: 'mosaic', color: '#999', width: 8,
      points: [{ x: 0, y: 0 }, { x: 50, y: 0 }],
    };
    const out = resizeAnnotation(ann, { x: 0, y: 0 }, { x: 50, y: 0 }, { x: 100, y: 0 }) as Annotation;
    expect(out.width).toBe(16);
  });
});

describe('isScalableKind 效果类标注禁改尺寸', () => {
  it('标记笔与马赛克不可缩放，其余可', () => {
    expect(isScalableKind('highlighter')).toBe(false);
    expect(isScalableKind('mosaic')).toBe(false);
    expect(isScalableKind('rect')).toBe(true);
    expect(isScalableKind('ellipse')).toBe(true);
    expect(isScalableKind('arrow')).toBe(true);
    expect(isScalableKind('pen')).toBe(true);
    expect(isScalableKind('text')).toBe(true);
    expect(isScalableKind('step')).toBe(true);
  });
});
