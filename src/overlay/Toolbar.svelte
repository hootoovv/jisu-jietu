<script lang="ts">
  /**
   * 截图工具条（开发计划 2.5）：
   * 撤销 | 七工具 | 颜色(8×2 色板+自定义) | 线宽(5 档+自定义滑杆) | 保存 复制 关闭
   */
  import type { ToolKind, ToolStyle } from '../lib/types';
  import { CUSTOM_WIDTH_MAX, CUSTOM_WIDTH_MIN, PRESET_COLORS, PRESET_WIDTHS } from '../lib/constants';

  interface Props {
    hasUndo: boolean;
    tool: ToolKind | null;
    style: ToolStyle;
    styleEnabled: boolean;
    busy: boolean;
    onundo: () => void;
    ontool: (t: ToolKind | null) => void;
    onstyle: (patch: Partial<ToolStyle>) => void;
    oncopy: () => void;
    onsave: () => void;
    onclose: () => void;
  }

  let {
    hasUndo,
    tool,
    style,
    styleEnabled,
    busy,
    onundo,
    ontool,
    onstyle,
    oncopy,
    onsave,
    onclose,
  }: Props = $props();

  const TOOLS: { kind: ToolKind; label: string; title: string }[] = [
    { kind: 'rect', label: '矩形', title: '矩形' },
    { kind: 'ellipse', label: '椭圆', title: '椭圆' },
    { kind: 'arrow', label: '箭头', title: '箭头' },
    { kind: 'pen', label: '画笔', title: '画笔' },
    { kind: 'mosaic', label: '马赛克', title: '马赛克' },
    { kind: 'text', label: '文本', title: '文本框' },
    { kind: 'step', label: '标号', title: '数字标号' },
  ];

  let popover = $state<'color' | 'width' | null>(null);
  let customColor = $state('#E8424C');

  function togglePopover(which: 'color' | 'width') {
    popover = popover === which ? null : which;
  }

  function pickColor(c: string) {
    onstyle({ color: c });
    popover = null;
  }

  function pickWidth(w: number) {
    onstyle({ width: w });
  }

  function stop(e: PointerEvent | MouseEvent) {
    e.stopPropagation();
  }

  function onCustomColorInput(e: Event) {
    const v = (e.target as HTMLInputElement).value.toUpperCase();
    customColor = v;
    onstyle({ color: v });
  }
</script>

<div class="toolbar" role="toolbar" onpointerdown={stop} ondblclick={stop}>
  <!-- 撤销 -->
  <button class="tb-btn" class:disabled={!hasUndo} title="撤销 (标注)" disabled={!hasUndo} onclick={() => onundo()}>
    <svg viewBox="0 0 24 24" width="18" height="18">
      <path
        d="M9 14 4 9l5-5"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
      <path
        d="M4 9h10a6 6 0 0 1 0 12h-3"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
      />
    </svg>
  </button>

  <span class="sep"></span>

  <!-- 七工具 -->
  {#each TOOLS as t (t.kind)}
    <button
      class="tb-btn"
      class:active={tool === t.kind}
      title={t.title}
      onclick={() => ontool(tool === t.kind ? null : t.kind)}
    >
      {#if t.kind === 'rect'}
        <svg viewBox="0 0 24 24" width="18" height="18"><rect x="4" y="6" width="16" height="12" rx="1" fill="none" stroke="currentColor" stroke-width="2" /></svg>
      {:else if t.kind === 'ellipse'}
        <svg viewBox="0 0 24 24" width="18" height="18"><ellipse cx="12" cy="12" rx="8" ry="6" fill="none" stroke="currentColor" stroke-width="2" /></svg>
      {:else if t.kind === 'arrow'}
        <svg viewBox="0 0 24 24" width="18" height="18"><path d="M4 20 20 4M20 4h-8M20 4v8" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg>
      {:else if t.kind === 'pen'}
        <svg viewBox="0 0 24 24" width="18" height="18"><path d="M4 20c1-4 2-6 5-9l8-8 4 4-8 8c-3 3-5 4-9 5z" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round" /></svg>
      {:else if t.kind === 'mosaic'}
        <svg viewBox="0 0 24 24" width="18" height="18">
          <rect x="4" y="4" width="5" height="5" fill="currentColor" />
          <rect x="10" y="4" width="5" height="5" fill="none" stroke="currentColor" />
          <rect x="16" y="4" width="4" height="5" fill="currentColor" />
          <rect x="4" y="10" width="5" height="5" fill="none" stroke="currentColor" />
          <rect x="10" y="10" width="5" height="5" fill="currentColor" />
          <rect x="16" y="10" width="4" height="5" fill="none" stroke="currentColor" />
          <rect x="4" y="16" width="5" height="4" fill="currentColor" />
          <rect x="10" y="16" width="5" height="4" fill="none" stroke="currentColor" />
          <rect x="16" y="16" width="4" height="4" fill="currentColor" />
        </svg>
      {:else if t.kind === 'text'}
        <svg viewBox="0 0 24 24" width="18" height="18"><path d="M5 5h14M12 5v14M9 19h6" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" /></svg>
      {:else if t.kind === 'step'}
        <svg viewBox="0 0 24 24" width="18" height="18"><circle cx="12" cy="12" r="8" fill="currentColor" /><text x="12" y="16" text-anchor="middle" font-size="11" font-weight="700" fill="var(--tb-bg, #2b2f36)">1</text></svg>
      {/if}
    </button>
  {/each}

  <span class="sep"></span>

  <!-- 颜色 -->
  <div class="pop-wrap">
    <button
      class="tb-btn"
      class:disabled={!styleEnabled}
      title="颜色"
      disabled={!styleEnabled}
      onclick={() => styleEnabled && togglePopover('color')}
    >
      <span class="swatch" style:background={style.color}></span>
    </button>
    {#if popover === 'color'}
      <div class="popover color-pop" onpointerdown={stop}>
        <div class="grid16">
          {#each PRESET_COLORS as c (c)}
            <button class="cell" class:selected={style.color.toUpperCase() === c} style:background={c} title={c} onclick={() => pickColor(c)}></button>
          {/each}
        </div>
        <div class="custom-row">
          <input type="color" value={customColor} oninput={onCustomColorInput} title="自定义颜色" />
          <input
            class="hex"
            type="text"
            spellcheck="false"
            maxlength="7"
            value={customColor}
            onchange={(e) => {
              const v = (e.target as HTMLInputElement).value;
              if (/^#[0-9a-fA-F]{6}$/.test(v)) pickColor(v.toUpperCase());
            }}
          />
        </div>
      </div>
    {/if}
  </div>

  <!-- 线宽 -->
  <div class="pop-wrap">
    <button
      class="tb-btn"
      class:disabled={!styleEnabled}
      title="线宽"
      disabled={!styleEnabled}
      onclick={() => styleEnabled && togglePopover('width')}
    >
      <span class="dot" style:width={`${style.width * 1.1 + 2}px`} style:height={`${style.width * 1.1 + 2}px`}></span>
    </button>
    {#if popover === 'width'}
      <div class="popover width-pop" onpointerdown={stop}>
        <div class="widths">
          {#each PRESET_WIDTHS as w (w)}
            <button class="wcell" class:selected={style.width === w} onclick={() => pickWidth(w)}>
              <span class="dot" style:width={`${w * 1.1 + 2}px`} style:height={`${w * 1.1 + 2}px`}></span>
              <span class="wlabel">{w}</span>
            </button>
          {/each}
        </div>
        <div class="slider-row">
          <span class="slabel">自定义</span>
          <input
            type="range"
            min={CUSTOM_WIDTH_MIN}
            max={CUSTOM_WIDTH_MAX}
            step="1"
            value={style.width}
            oninput={(e) => pickWidth(Number((e.target as HTMLInputElement).value))}
          />
          <span class="sval">{style.width}px</span>
        </div>
      </div>
    {/if}
  </div>

  <span class="sep"></span>

  <!-- 输出区 -->
  <button class="tb-btn" title="保存到文件" disabled={busy} onclick={() => onsave()}>
    <svg viewBox="0 0 24 24" width="18" height="18"><path d="M5 3h11l3 3v15H5zM8 3v5h8V3M8 21v-7h8v7" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round" /></svg>
  </button>
  <button class="tb-btn primary" title="复制到剪贴板 (Enter)" disabled={busy} onclick={() => oncopy()}>
    <svg viewBox="0 0 24 24" width="18" height="18"><rect x="8" y="8" width="12" height="13" rx="2" fill="none" stroke="currentColor" stroke-width="2" /><path d="M16 8V5a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2h3" fill="none" stroke="currentColor" stroke-width="2" /></svg>
  </button>
  <button class="tb-btn" title="退出截图 (Esc)" disabled={busy} onclick={() => onclose()}>
    <svg viewBox="0 0 24 24" width="18" height="18"><path d="M6 6l12 12M18 6 6 18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" /></svg>
  </button>
</div>

<style>
  .toolbar {
    --tb-bg: #2b2f36;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 5px 8px;
    background: var(--tb-bg);
    border-radius: 8px;
    box-shadow:
      0 4px 16px rgba(0, 0, 0, 0.35),
      0 0 0 1px rgba(255, 255, 255, 0.08);
    user-select: none;
  }

  .tb-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: #d6d9e0;
    cursor: pointer;
    transition: background 0.12s ease;
  }

  .tb-btn:hover:not(.disabled):not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
  }

  .tb-btn.active {
    background: #3d82f6;
    color: #fff;
  }

  .tb-btn.primary {
    color: #7ab7ff;
  }

  .tb-btn:disabled {
    opacity: 0.35;
    cursor: default;
  }

  .sep {
    width: 1px;
    height: 20px;
    margin: 0 4px;
    background: rgba(255, 255, 255, 0.14);
  }

  .swatch {
    width: 16px;
    height: 16px;
    border-radius: 4px;
    box-shadow:
      inset 0 0 0 1px rgba(255, 255, 255, 0.35),
      0 0 0 1px rgba(0, 0, 0, 0.4);
  }

  .dot {
    border-radius: 50%;
    background: #e6e9ef;
  }

  .pop-wrap {
    position: relative;
    display: inline-flex;
  }

  .popover {
    position: absolute;
    top: calc(100% + 8px);
    left: 50%;
    transform: translateX(-50%);
    background: var(--tb-bg);
    border-radius: 8px;
    padding: 10px;
    box-shadow:
      0 8px 24px rgba(0, 0, 0, 0.45),
      0 0 0 1px rgba(255, 255, 255, 0.08);
    z-index: 30;
  }

  .grid16 {
    display: grid;
    grid-template-columns: repeat(8, 22px);
    gap: 4px;
  }

  .cell {
    width: 22px;
    height: 22px;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.18);
  }

  .cell:hover {
    transform: scale(1.12);
  }

  .cell.selected {
    box-shadow:
      0 0 0 2px #fff,
      inset 0 0 0 1px rgba(0, 0, 0, 0.3);
  }

  .custom-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px solid rgba(255, 255, 255, 0.12);
  }

  .custom-row input[type='color'] {
    width: 30px;
    height: 24px;
    padding: 0;
    border: none;
    background: none;
    cursor: pointer;
  }

  .hex {
    width: 84px;
    height: 24px;
    border: 1px solid rgba(255, 255, 255, 0.2);
    border-radius: 4px;
    background: rgba(0, 0, 0, 0.3);
    color: #e6e9ef;
    font-size: 12px;
    text-align: center;
    font-family: Consolas, monospace;
  }

  .widths {
    display: flex;
    gap: 2px;
  }

  .wcell {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 3px;
    width: 34px;
    height: 38px;
    border: none;
    border-radius: 6px;
    background: transparent;
    cursor: pointer;
  }

  .wcell:hover {
    background: rgba(255, 255, 255, 0.12);
  }

  .wcell.selected {
    background: rgba(61, 130, 246, 0.35);
  }

  .wcell .dot {
    background: #e6e9ef;
  }

  .wlabel {
    font-size: 10px;
    color: #aab0ba;
  }

  .slider-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px solid rgba(255, 255, 255, 0.12);
  }

  .slabel {
    font-size: 12px;
    color: #aab0ba;
    white-space: nowrap;
  }

  .slider-row input[type='range'] {
    width: 110px;
    accent-color: #3d82f6;
  }

  .sval {
    font-size: 12px;
    color: #e6e9ef;
    width: 34px;
    font-family: Consolas, monospace;
  }
</style>
