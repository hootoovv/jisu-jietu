/** UI 常量（逻辑像素，DOM 层使用）。 */

/** 工具条外框尺寸（含内边距；按钮实际内容决定，先取保守上限） */
export const TOOLBAR_SIZE = { w: 486, h: 44 };

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

/** 尺寸读数框估计尺寸（逻辑像素，用于防出屏翻转） */
export const BADGE_SIZE = { w: 78, h: 24 };
