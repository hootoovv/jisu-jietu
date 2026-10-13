<script lang="ts">
  /**
   * 截图覆盖层主组件（开发计划 4.4 选区状态机的宿主）。
   *
   * 分层：img(冻结画面) < canvas#mask(蒙版/选区) < canvas#annot(标注)
   *       < DOM(手柄/读数/工具条/文本编辑器)
   * 坐标：绘制一律物理像素；DOM 一律逻辑像素（÷dpr），换算唯此一处。
   */
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { save as saveDialog } from '@tauri-apps/plugin-dialog';
  import {
    copyImage,
    endSession,
    fetchSession,
    getSettings,
    logMetric,
    revealOverlay,
    saveImage,
  } from '../lib/ipc';
  import {
    composeAnnotationsPng,
    drawAnnotations,
    drawMask,
    invalidateMosaicCache,
  } from '../lib/render';
  import {
    badgePosition,
    clampRect,
    dist,
    fontSizeForWidth,
    hitTestWindows,
    inRect,
    moveRect,
    normalizeRect,
    placeToolbar,
    rectCorners,
  } from '../lib/geometry';
  import { BADGE_SIZE, CLICK_SLOP_PX, HANDLE_SIZE, MIN_SELECTION, TOOLBAR_SIZE } from '../lib/constants';
  import type {
    Annotation,
    FrozenPayload,
    Phase,
    Pt,
    Rect,
    ToolKind,
    ToolStyle,
  } from '../lib/types';
  import Toolbar from './Toolbar.svelte';
  import TextEditor from './TextEditor.svelte';

  // ---------- 会话 ----------
  let session = $state<FrozenPayload | null>(null);
  const dpr = window.devicePixelRatio || 1;

  // Windows/Android 的 WebView2 将自定义协议映射为 http://<scheme>.localhost/<path>；
  // 该映射只作用于 wry 接管的页面导航 URL，页面自拼的子资源 URL 必须直接用映射形态
  function snapshotSrc(id: number): string {
    return /Windows|Android/i.test(navigator.userAgent)
      ? `http://snapshot.localhost/${id}`
      : `snapshot://localhost/${id}`;
  }

  const snapshotUrl = $derived(session ? snapshotSrc(session.snapshotId) : '');
  const virtual = $derived<Rect | null>(session?.virtualRect ?? null);

  // ---------- 交互状态 ----------
  let phase = $state<Phase>('loading');
  let selection = $state<Rect | null>(null);
  let hover = $state<Rect | null>(null);
  let annotations = $state<Annotation[]>([]);
  let drawing = $state<Annotation | null>(null);
  let drawAnchor = $state<Pt | null>(null);
  let tool = $state<ToolKind | null>(null);
  let textEdit = $state<Pt | null>(null); // 物理坐标
  let busy = $state(false);
  let cursor = $state('crosshair');
  let annSeq = 0;
  let loadStart = 0;

  interface DragInfo {
    mode: 'select' | 'move' | 'resize';
    startPt: Pt;
    startSel: Rect;
    corner?: number;
    moved: boolean;
  }
  let drag = $state<DragInfo | null>(null);

  // ---------- DOM refs ----------
  let imgEl = $state<HTMLImageElement | null>(null);
  let maskCanvas = $state<HTMLCanvasElement | null>(null);
  let annotCanvas = $state<HTMLCanvasElement | null>(null);

  function defaultStyles(): Record<ToolKind, ToolStyle> {
    const d = {} as Record<ToolKind, ToolStyle>;
    const kinds: ToolKind[] = ['rect', 'ellipse', 'arrow', 'pen', 'mosaic', 'text', 'step'];
    for (const k of kinds) d[k] = { color: '#E8424C', width: 4 };
    return d;
  }
  let styles = $state<Record<ToolKind, ToolStyle>>(defaultStyles());

  const currentStyle = $derived(tool ? styles[tool] : { color: '#E8424C', width: 4 });

  // ---------- 坐标换算（唯一转换点） ----------
  const evPt = (e: PointerEvent | MouseEvent): Pt => ({
    x: Math.round(e.clientX * dpr),
    y: Math.round(e.clientY * dpr),
  });
  const toL = (v: number): number => v / dpr;

  // ---------- DOM 派生（逻辑像素） ----------
  const selL = $derived.by(() => {
    if (!selection) return null;
    return { x: toL(selection.x), y: toL(selection.y), w: toL(selection.w), h: toL(selection.h) };
  });

  const virtL = $derived.by(() => {
    if (!virtual) return null;
    return { x: toL(virtual.x), y: toL(virtual.y), w: toL(virtual.w), h: toL(virtual.h) };
  });

  const toolbarPos = $derived.by(() => {
    if (!selL || !virtL) return null;
    return placeToolbar(selL, virtL, TOOLBAR_SIZE);
  });

  const badgePos = $derived.by(() => {
    if (!selL || !virtL) return null;
    return badgePosition(selL, virtL, BADGE_SIZE);
  });

  const handles = $derived.by(() => {
    if (!selL) return [];
    return [
      { x: selL.x, y: selL.y },
      { x: selL.x + selL.w, y: selL.y },
      { x: selL.x + selL.w, y: selL.y + selL.h },
      { x: selL.x, y: selL.y + selL.h },
    ];
  });

  const CURSOR_BY_CORNER = ['nwse-resize', 'nesw-resize', 'nwse-resize', 'nesw-resize'];

  const showToolbar = $derived(
    !!selection && phase !== 'loading' && phase !== 'dragSelect' && phase !== 'textEditing' && !busy,
  );
  const showBadge = $derived(phase === 'dragSelect' || phase === 'adjust');

  // ---------- 生命周期 ----------
  onMount(() => {
    // 常驻窗口复用：会话经 session-frozen 事件反复到达，每次全量复位
    void listen<FrozenPayload>('session-frozen', (e) => applySession(e.payload));
    void boot();

    const onKey = (e: KeyboardEvent) => {
      if (phase === 'textEditing') return; // TextEditor 已 stopPropagation
      if (e.key === 'Escape') {
        e.preventDefault();
        if (!busy) void endSession('cancelled');
      } else if (e.key === 'Enter' && selection && !busy) {
        e.preventDefault();
        void confirmCopy();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  // 兜底拉取：覆盖窗口为预建常驻，挂载时通常尚无会话；事件先于监听注册
  // 到达的竞态由 fetch 兜住
  async function boot() {
    const payload = await fetchSession();
    if (payload) applySession(payload);
  }

  /** 应用一次新会话：页面不重载，交互状态必须在此全量复位。 */
  function applySession(payload: FrozenPayload) {
    if (session?.snapshotId === payload.snapshotId) return;
    selection = null;
    hover = null;
    annotations = [];
    drawing = null;
    drawAnchor = null;
    tool = null;
    textEdit = null;
    busy = false;
    cursor = 'crosshair';
    annSeq = 0;
    loadStart = performance.now();
    invalidateMosaicCache(); // 冻结图已更换，马赛克纹理缓存必须失效
    session = payload;
    phase = 'frozen';
  }

  function onImgLoad() {
    if (!session) return;
    requestAnimationFrame(() => {
      void revealOverlay();
      const total = session!.hotkeyElapsedMs + (performance.now() - loadStart);
      void logMetric('N1_hotkey_to_mask', total);
    });
  }

  // ---------- 画布尺寸与渲染 ----------
  $effect(() => {
    if (session && maskCanvas) {
      maskCanvas.width = session.virtualRect.w;
      maskCanvas.height = session.virtualRect.h;
    }
    if (session && annotCanvas) {
      annotCanvas.width = session.virtualRect.w;
      annotCanvas.height = session.virtualRect.h;
    }
  });

  $effect(() => {
    if (!maskCanvas || !session) return;
    const ctx = maskCanvas.getContext('2d');
    if (!ctx) return;
    drawMask(ctx, { selection, hover, dpr });
  });

  $effect(() => {
    if (!annotCanvas || !session || !virtual) return;
    const ctx = annotCanvas.getContext('2d');
    if (!ctx) return;
    drawAnnotations(ctx, {
      annotations,
      drawing,
      selection,
      offset: { x: 0, y: 0 },
      img: imgEl,
      virtual,
      dpr,
      clip: true,
    });
  });

  // ---------- 指针交互（状态机） ----------
  function handleHitIndex(p: Pt): number | null {
    if (!selection) return null;
    const corners = rectCorners(selection);
    const slack = (HANDLE_SIZE / 2 + 4) * dpr;
    for (let i = 0; i < 4; i++) {
      if (Math.abs(p.x - corners[i].x) <= slack && Math.abs(p.y - corners[i].y) <= slack) {
        return i;
      }
    }
    return null;
  }

  function onPointerDown(e: PointerEvent) {
    if (phase === 'textEditing' || busy || e.button !== 0) return;
    if (!virtual) return;
    const p = evPt(e);

    const hi = handleHitIndex(p);
    if (hi !== null && selection) {
      drag = { mode: 'resize', startPt: p, startSel: selection, corner: hi, moved: true };
      phase = 'adjust';
    } else if (selection && inRect(selection, p) && !tool) {
      drag = { mode: 'move', startPt: p, startSel: selection, moved: true };
      phase = 'adjust';
    } else if (tool) {
      if (tool === 'text') {
        // 必须 cancel pointerdown：否则同一交互序列的 mousedown 默认行为会把焦点
        // 抢回 body，onMount 里的 el.focus() 随即被 onblur 覆盖，空文本触发
        // oncancel，编辑器挂载当帧自毁（表现为点击无反应）
        e.preventDefault();
      }
      startDrawing(p);
    } else {
      drag = { mode: 'select', startPt: p, startSel: selection ?? { x: p.x, y: p.y, w: 0, h: 0 }, moved: false };
      phase = 'dragSelect';
      hover = null;
    }
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onPointerMove(e: PointerEvent) {
    if (!virtual) return;
    const p = evPt(e);

    if (drag) {
      drag.moved = drag.moved || dist(drag.startPt, p) > CLICK_SLOP_PX;
      if (drag.mode === 'select' && !drawing) {
        selection = clampRect(normalizeRect(drag.startPt, p), virtual, MIN_SELECTION);
      } else if (drag.mode === 'move') {
        selection = moveRect(drag.startSel, p.x - drag.startPt.x, p.y - drag.startPt.y, virtual, MIN_SELECTION);
      } else if (drag.mode === 'resize' && drag.corner !== undefined) {
        const anchor = rectCorners(drag.startSel)[(drag.corner + 2) % 4];
        selection = clampRect(normalizeRect(anchor, p), virtual, MIN_SELECTION);
      }
      return;
    }

    if (drawing) {
      updateDrawing(p);
      return;
    }

    // 悬停预高亮（无选区、无工具）
    if (!selection && !tool && session) {
      hover = hitTestWindows(session.windows, p);
      cursor = 'crosshair';
    } else {
      hover = null;
      const hi = handleHitIndex(p);
      if (hi !== null) cursor = CURSOR_BY_CORNER[hi];
      else if (selection && inRect(selection, p) && !tool) cursor = 'move';
      else cursor = 'crosshair';
    }
  }

  function onPointerUp(e: PointerEvent) {
    const p = evPt(e);
    if (drawing) {
      commitDrawing();
      return;
    }
    if (!drag) return;
    const d = drag;
    drag = null;

    if (d.mode === 'select') {
      if (!d.moved) {
        // 单击（≤4px）→ 智能窗口识别（FR-04）
        const w = session ? hitTestWindows(session.windows, p) : null;
        if (w) {
          selection = clampRect(w, virtual!, MIN_SELECTION);
          phase = 'selected';
        } else {
          selection = null;
          phase = 'frozen';
        }
        hover = null;
      } else {
        phase = 'selected';
      }
    } else {
      phase = 'selected';
    }
  }

  function onDoubleClick(e: MouseEvent) {
    // 工具激活时双击属于绘制语义（如文本工具连续放置），不得触发「双击复制」
    if (busy || phase === 'textEditing' || tool) return;
    const p = evPt(e);
    if (selection && inRect(selection, p)) void confirmCopy();
  }

  // ---------- 标注引擎 ----------
  function startDrawing(p: Pt) {
    if (!tool) return;
    const st = styles[tool];
    annSeq += 1;
    const base = { id: annSeq, kind: tool, color: st.color, width: st.width };

    switch (tool) {
      case 'rect':
      case 'ellipse':
        drawing = { ...base, rect: { x: p.x, y: p.y, w: 0, h: 0 } };
        drawAnchor = p;
        break;
      case 'arrow':
        drawing = { ...base, line: { x1: p.x, y1: p.y, x2: p.x, y2: p.y } };
        break;
      case 'pen':
      case 'mosaic':
        drawing = { ...base, points: [p] };
        break;
      case 'text':
        textEdit = p;
        phase = 'textEditing';
        break;
      case 'step':
        annotations = [...annotations, { ...base, rect: { x: p.x, y: p.y, w: 0, h: 0 } }];
        break;
    }
    if (tool !== 'text' && tool !== 'step') phase = 'annotate';
  }

  function updateDrawing(p: Pt) {
    if (!drawing) return;
    switch (drawing.kind) {
      case 'rect':
      case 'ellipse':
        if (drawAnchor && drawing.rect) drawing.rect = normalizeRect(drawAnchor, p);
        break;
      case 'arrow':
        if (drawing.line) drawing.line = { ...drawing.line, x2: p.x, y2: p.y };
        break;
      case 'pen':
      case 'mosaic':
        if (drawing.points) {
          const last = drawing.points[drawing.points.length - 1];
          if (!last || dist(last, p) > 2 * dpr) {
            drawing.points = [...drawing.points, p];
          }
        }
        break;
    }
  }

  function commitDrawing() {
    const d = drawing;
    drawing = null;
    drawAnchor = null;
    if (!d) {
      phase = 'selected';
      return;
    }
    let ok = false;
    switch (d.kind) {
      case 'rect':
      case 'ellipse':
        ok = !!d.rect && d.rect.w >= 2 * dpr && d.rect.h >= 2 * dpr;
        break;
      case 'arrow':
        ok =
          !!d.line &&
          dist({ x: d.line.x1, y: d.line.y1 }, { x: d.line.x2, y: d.line.y2 }) > 4 * dpr;
        break;
      case 'pen':
        ok = !!d.points && d.points.length >= 2;
        break;
      case 'mosaic':
        ok = !!d.points && d.points.length >= 1;
        break;
    }
    if (ok) annotations = [...annotations, d];
    phase = 'selected';
  }

  function onTextCommit(text: string) {
    if (textEdit) {
      const st = styles.text;
      annSeq += 1;
      annotations = [
        ...annotations,
        {
          id: annSeq,
          kind: 'text',
          color: st.color,
          width: st.width,
          text,
          rect: { x: textEdit.x, y: textEdit.y, w: 0, h: 0 },
        },
      ];
    }
    textEdit = null;
    phase = 'selected';
  }

  function onTextCancel() {
    textEdit = null;
    phase = 'selected';
  }

  // ---------- 工具条回调 ----------
  function undo() {
    annotations = annotations.slice(0, -1);
  }

  function pickTool(t: ToolKind | null) {
    tool = t;
  }

  function patchStyle(patch: Partial<ToolStyle>) {
    if (!tool) return;
    styles[tool] = { ...styles[tool], ...patch };
  }

  // ---------- 导出链路（复制/保存，开发计划 5.1.5 / S7 失败语义） ----------

  function timestamp(): string {
    const d = new Date();
    const p = (n: number) => String(n).padStart(2, '0');
    return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}_${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}`;
  }

  function joinPath(dir: string, name: string): string {
    const d = dir.replace(/[\\/]+$/, '');
    return d.length > 0 ? `${d}\\${name}` : name;
  }

  async function composePng(): Promise<Uint8Array> {
    if (!selection || !virtual) throw new Error('无有效选区');
    return composeAnnotationsPng(annotations, selection, virtual, imgEl, dpr);
  }

  async function confirmCopy() {
    if (!selection || busy) return;
    busy = true;
    const t0 = performance.now();
    try {
      const png = await composePng();
      await copyImage(selection, png);
      await logMetric('N5_copy', performance.now() - t0);
      await endSession('confirmed');
    } catch (err) {
      // S7 失败语义：保留会话，用户可重试或换出口
      console.error('[jisu-jietu] 复制失败:', err);
      busy = false;
    }
  }

  async function confirmSave() {
    if (!selection || busy) return;
    busy = true;
    try {
      const png = await composePng();
      const s = await getSettings();
      const fmt = s.lastFormat === 'jpeg' ? 'jpeg' : 'png';
      const defaultPath = joinPath(
        s.defaultSaveDir ?? '',
        `截图_${timestamp()}.${fmt === 'jpeg' ? 'jpg' : 'png'}`,
      );
      const path = await saveDialog({
        title: '保存截图',
        defaultPath,
        filters: [
          { name: 'PNG 图片', extensions: ['png'] },
          { name: 'JPEG 图片', extensions: ['jpg', 'jpeg'] },
        ],
      });
      if (!path) {
        busy = false; // 用户取消对话框，会话继续
        return;
      }
      const format = /\.(jpe?g)$/i.test(path) ? 'jpeg' : 'png';
      const t0 = performance.now();
      await saveImage(selection, png, path, format);
      await logMetric('N6_save', performance.now() - t0);
      await endSession('confirmed');
    } catch (err) {
      console.error('[jisu-jietu] 保存失败:', err);
      busy = false;
    }
  }

  function closeSession() {
    if (!busy) void endSession('cancelled');
  }
</script>

<svelte:head>
  <style>
    html,
    body {
      margin: 0;
      overflow: hidden;
      background: #000;
    }
  </style>
</svelte:head>

{#if session}
  <div
    class="layers"
    role="application"
    style:cursor={cursor}
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    ondblclick={onDoubleClick}
  >
    <img
      bind:this={imgEl}
      src={snapshotUrl}
      alt=""
      class="frozen"
      draggable="false"
      crossorigin="anonymous"
      onload={onImgLoad}
      onerror={() => {
        console.error('[jisu-jietu] snapshot 加载失败');
        // 冻结图加载失败则无法 reveal，必须结束会话，否则 active 防重入会吞掉后续热键
        void endSession('cancelled');
      }}
    />
    <canvas bind:this={maskCanvas} class="layer"></canvas>
    <canvas bind:this={annotCanvas} class="layer"></canvas>

    <!-- 四角手柄 -->
    {#if showToolbar}
      {#each handles as h, i (i)}
        <div
          class="handle"
          style:left="{h.x - HANDLE_SIZE / 2}px"
          style:top="{h.y - HANDLE_SIZE / 2}px"
          style:cursor={CURSOR_BY_CORNER[i]}
        ></div>
      {/each}
    {/if}

    <!-- 尺寸读数 -->
    {#if showBadge && selL}
      <div
        class="badge"
        style:left="{badgePos?.x ?? selL.x + selL.w + 8}px"
        style:top="{badgePos?.y ?? selL.y + selL.h + 8}px"
      >
        {selection!.w}×{selection!.h}
      </div>
    {/if}

    <!-- 工具条 -->
    {#if toolbarPos && showToolbar}
      <div
        class="toolbar-wrap"
        style:left="{toolbarPos.x}px"
        style:top="{toolbarPos.y}px"
        style:transition-property="left, top"
      >
        <Toolbar
          hasUndo={annotations.length > 0}
          {tool}
          style={currentStyle}
          styleEnabled={!!tool}
          {busy}
          onundo={undo}
          ontool={pickTool}
          onstyle={patchStyle}
          oncopy={() => void confirmCopy()}
          onsave={() => void confirmSave()}
          onclose={closeSession}
        />
      </div>
    {/if}

    <!-- 文本编辑器 -->
    {#if textEdit && tool === 'text' && selL}
      {@const maxW = Math.max(60, selL.x + selL.w - toL(textEdit.x))}
      <TextEditor
        x={toL(textEdit.x)}
        y={toL(textEdit.y)}
        color={styles.text.color}
        fontSize={fontSizeForWidth(styles.text.width)}
        maxWidth={maxW}
        oncommit={onTextCommit}
        oncancel={onTextCancel}
      />
    {/if}
  </div>
{:else}
  <div class="waiting"></div>
{/if}

<style>
  .layers {
    position: fixed;
    inset: 0;
    overflow: hidden;
    cursor: crosshair;
  }

  .frozen {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    user-select: none;
    -webkit-user-drag: none;
    pointer-events: none;
  }

  .layer {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
  }

  .handle {
    position: absolute;
    width: 12px;
    height: 12px;
    background: #fff;
    border: 1.5px solid #3d82f6;
    border-radius: 2px;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.5);
    pointer-events: none;
    z-index: 20;
  }

  .badge {
    position: absolute;
    min-width: 78px;
    height: 24px;
    padding: 0 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(20, 22, 27, 0.85);
    color: #e6e9ef;
    font: 12px Consolas, monospace;
    border-radius: 4px;
    pointer-events: none;
    z-index: 20;
  }

  .toolbar-wrap {
    position: absolute;
    z-index: 25;
    transition:
      left 0.1s ease,
      top 0.1s ease;
  }

  .waiting {
    position: fixed;
    inset: 0;
    background: #000;
  }
</style>
