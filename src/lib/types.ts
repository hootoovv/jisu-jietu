/** 与 Rust 侧结构一一对应（serde camelCase）。矩形一律物理像素。 */

export interface PhysRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface MonitorInfo {
  id: string;
  rect: PhysRect;
  dpiScale: number;
  isPrimary: boolean;
}

export interface WindowInfo {
  hwnd: number;
  rect: PhysRect;
  title: string;
  process: string;
  /** 越小越顶层 */
  z: number;
}

export interface FrozenPayload {
  snapshotId: number;
  virtualRect: PhysRect;
  monitors: MonitorInfo[];
  windows: WindowInfo[];
  hotkeyElapsedMs: number;
}

export interface Settings {
  schemaVersion: number;
  hotkey: string;
  defaultSaveDir: string | null;
  lastFormat: string;
  autostart: boolean;
}

// ---------- 会话内前端状态类型 ----------

export type ToolKind =
  | 'rect'
  | 'ellipse'
  | 'arrow'
  | 'pen'
  | 'highlighter'
  | 'mosaic'
  | 'text'
  | 'step';

export interface Pt {
  x: number;
  y: number;
}

/** 一条已提交的标注（几何载荷按 kind 取用）。 */
export interface Annotation {
  id: number;
  kind: ToolKind;
  color: string;
  /** 逻辑像素线宽；text 映射字号档；mosaic 映射块尺寸档 */
  width: number;
  rect?: PhysRect;
  line?: { x1: number; y1: number; x2: number; y2: number };
  points?: Pt[];
  text?: string;
}

export interface ToolStyle {
  color: string;
  width: number;
}

export type Phase =
  | 'loading' // 等待冻结数据
  | 'frozen' // 蒙版显现，未开始选择
  | 'dragSelect' // 拖拽画选区中
  | 'selected' // 选区已定
  | 'adjust' // 手柄调整 / 移动选区中
  | 'annotate' // 标注绘制中
  | 'textEditing'; // 文本输入中
