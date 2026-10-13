/** UI 常量（逻辑像素，DOM 层使用）。 */

import type { ToolKind, ToolStyle } from './types';

/** 工具条外框尺寸（含内边距；按钮实际内容决定，先取保守上限） */
export const TOOLBAR_SIZE = { w: 520, h: 44 };

/** 选区最小边长（物理像素） */
export const MIN_SELECTION = 8;

/** 按下-松开位移小于该值视为单击（物理像素） */
export const CLICK_SLOP_PX = 4;

/** 四角手柄边长（逻辑像素） */
export const HANDLE_SIZE = 12;

/** 蒙版不透明度 */
export const MASK_OPACITY = 0.5;

/** 选区边框颜色 */
export const SEL_BORDER = '#4A9EFF';

/** 8×2 预设色板（开发计划 2.5） */
export const PRESET_COLORS: string[] = [
  '#000000', '#FFFFFF', '#9E9E9E', '#E8424C',
  '#F08C2E', '#F7C948', '#4CAF50', '#2196F3',
  '#5E35B1', '#EC407A', '#795548', '#00BCD4',
  '#3F51B5', '#8BC34A', '#FF7043', '#37474F',
];

/** 5 档预设线宽（逻辑像素） */
export const PRESET_WIDTHS = [2, 4, 6, 8, 12] as const;

/** 自定义线宽范围（逻辑像素） */
export const CUSTOM_WIDTH_MIN = 1;
export const CUSTOM_WIDTH_MAX = 20;

/** 标记笔（高亮）不透明度 */
export const HIGHLIGHTER_ALPHA = 0.4;

/** 各工具默认样式：默认颜色互异；样式按工具独立记忆（切工具互不覆盖）。
 *  width 为逻辑像素：text 映射字号档、mosaic 映射块尺寸档。 */
export const TOOL_DEFAULT_STYLES: Record<ToolKind, ToolStyle> = {
  rect: { color: '#E8424C', width: 4 },
  ellipse: { color: '#7C4DFF', width: 4 },
  arrow: { color: '#FF8A00', width: 4 },
  pen: { color: '#17B26A', width: 4 },
  highlighter: { color: '#FFE44D', width: 16 },
  mosaic: { color: '#9AA4B2', width: 4 },
  text: { color: '#1273EB', width: 4 },
  step: { color: '#D6336C', width: 4 },
};

// ---------- 自定义光标（白色高亮 + 1px 黑描边，压暗蒙版上高可见） ----------
// data URI 中 # 须转义为 %23；SVG 属性用单引号，避免与 CSS url("...") 冲突。

/** 高亮白色十字（矩形 / 椭圆及默认态） */
export const CURSOR_CROSSHAIR =
  "url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'><g stroke='%23000' stroke-width='4'><line x1='12' y1='1.5' x2='12' y2='22.5'/><line x1='1.5' y1='12' x2='22.5' y2='12'/></g><g stroke='%23fff' stroke-width='1.6'><line x1='12' y1='2' x2='12' y2='22'/><line x1='2' y1='12' x2='22' y2='12'/></g></svg>\") 12 12, crosshair";

const cursorOf = (svg: string, hx: number, hy: number, fb = 'crosshair') =>
  `url("data:image/svg+xml;utf8,${svg}") ${hx} ${hy}, ${fb}`;

/** 画笔：斜向细线笔触，左下热点处黑色「笔头」圆点（白描边勾轮廓，
 *  深浅背景均可见），点即落笔精确位置 */
export const CURSOR_PEN = cursorOf(
  "<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'><g stroke='%23000' stroke-width='4' stroke-linecap='round'><line x1='5' y1='19' x2='17' y2='7'/></g><g stroke='%23fff' stroke-width='2' stroke-linecap='round'><line x1='5' y1='19' x2='17' y2='7'/></g><circle cx='5' cy='19' r='2.6' fill='%23000' stroke='%23fff' stroke-width='1'/></svg>",
  5,
  19,
);

/** 标记笔：斜向粗线笔触（比画笔更粗），左下热点处黑色「笔头」圆点 */
export const CURSOR_MARKER = cursorOf(
  "<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'><g stroke='%23000' stroke-width='9' stroke-linecap='round'><line x1='6' y1='18' x2='16' y2='8'/></g><g stroke='%23fff' stroke-width='5.5' stroke-linecap='round'><line x1='6' y1='18' x2='16' y2='8'/></g><circle cx='6' cy='18' r='3' fill='%23000' stroke='%23fff' stroke-width='1'/></svg>",
  6,
  18,
);

/** 箭头：右上指向箭形（箭尖在右上） */
export const CURSOR_ARROW = cursorOf(
  "<svg xmlns='http://www.w3.org/2000/svg' width='26' height='26'><g stroke='%23000' stroke-width='5' stroke-linecap='round' stroke-linejoin='round'><path d='M5 21L21 5M21 5h-9M21 5v9'/></g><g stroke='%23fff' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='M5 21L21 5M21 5h-9M21 5v9'/></g></svg>",
  21,
  5,
);

/** 马赛克：方形环（对应块状笔刷范围） */
export const CURSOR_SQUARE = cursorOf(
  "<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'><rect x='4' y='4' width='16' height='16' fill='none' stroke='%23000' stroke-width='4'/><rect x='4' y='4' width='16' height='16' fill='none' stroke='%23fff' stroke-width='1.8'/></svg>",
  12,
  12,
);

/** 数字标号：圆形环（对应标号圆点落点） */
export const CURSOR_CIRCLE = cursorOf(
  "<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'><circle cx='12' cy='12' r='8' fill='none' stroke='%23000' stroke-width='4'/><circle cx='12' cy='12' r='8' fill='none' stroke='%23fff' stroke-width='1.8'/></svg>",
  12,
  12,
);

/** 各绘图工具对应光标（矩形/椭圆沿用白色十字） */
export const TOOL_CURSORS: Record<ToolKind, string> = {
  rect: CURSOR_CROSSHAIR,
  ellipse: CURSOR_CROSSHAIR,
  arrow: CURSOR_ARROW,
  pen: CURSOR_PEN,
  highlighter: CURSOR_MARKER,
  mosaic: CURSOR_SQUARE,
  text: 'text',
  step: CURSOR_CIRCLE,
};

/** 尺寸读数框估计尺寸（逻辑像素，用于防出屏翻转） */
export const BADGE_SIZE = { w: 78, h: 24 };
