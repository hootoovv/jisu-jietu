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
    measureTextSize,
  } from '../lib/render';
  import {
    annBBox,
    badgePosition,
    clampRect,
    dist,
    hitTestAnnotation,
    hitTestWindows,
    inRect,
    isScalableKind,
    moveAnnotation,
    moveRect,
    normalizeRect,
    placeToolbar,
    rectCorners,
    resizeAnnotation,
  } from '../lib/geometry';
  import {
    BADGE_SIZE,
    CLICK_SLOP_PX,
    CURSOR_CROSSHAIR,
    HANDLE_SIZE,
    MIN_SELECTION,
    TOOLBAR_SIZE,
    TOOL_CURSORS,
    TOOL_DEFAULT_STYLES,
  } from '../lib/constants';
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
  /** 选中的标注对象 id（对象化编辑：点击选中 → 拖动/缩放/改样式/删除） */
  let selectedAnn = $state<number | null>(null);
  const selectedAnnotation = $derived(annotations.find((a) => a.id === selectedAnn) ?? null);
  let drawing = $state<Annotation | null>(null);
  let drawAnchor = $state<Pt | null>(null);
  let tool = $state<ToolKind | null>(null);
  let textEdit = $state<Pt | null>(null); // 物理坐标
  let busy = $state(false);
  let cursor = $state(CURSOR_CROSSHAIR);
  let annSeq = 0;
  let loadStart = 0;

  interface DragInfo {
    mode: 'select' | 'move' | 'resize' | 'annMove' | 'annResize';
    startPt: Pt;
    startSel: Rect;
    corner?: number;
    /** annMove/annResize：目标标注 id 与拖拽起始快照（缩放基于快照计算，幂等） */
    annId?: number;
    annSnapshot?: Annotation;
    moved: boolean;
  }
  let drag = $state<DragInfo | null>(null);

  // ---------- DOM refs ----------
  let imgEl = $state<HTMLImageElement | null>(null);
  let maskCanvas = $state<HTMLCanvasElement | null>(null);
  let annotCanvas = $state<HTMLCanvasElement | null>(null);
  let textEditorRef = $state<{ commitNow: () => void } | null>(null);

  // 各工具默认颜色互异（TOOL_DEFAULT_STYLES）；patchStyle 只写当前工具，
  // 颜色/线宽按工具独立记忆，切换工具互不覆盖
  let styles = $state<Record<ToolKind, ToolStyle>>({ ...TOOL_DEFAULT_STYLES });

  // 工具条样式源：工具激活时为该工具默认样式；标注选中时为该对象当前样式
  // （文本上下文附带 font/size/bold，供字体设置弹层显示与修改）
  const currentStyle = $derived(
    tool
      ? styles[tool]
      : selectedAnnotation
        ? {
            color: selectedAnnotation.color,
            width: selectedAnnotation.width,
            font: selectedAnnotation.font,
            size: selectedAnnotation.size,
            bold: selectedAnnotation.bold,
          }
        : { color: '#E8424C', width: 4 },
  );

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

  // 文本编辑中也保留工具条：字体/字号/加粗/颜色可就地调整（TextEditor 对工具条
  // 的 blur 做了豁免，点回编辑器继续输入）
  const showToolbar = $derived(
    !!selection && phase !== 'loading' && phase !== 'dragSelect' && !busy,
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
        if (busy) return;
        // 三层 Esc：先取消标注选中，再退出会话
        if (selectedAnn !== null) selectedAnn = null;
        else void endSession('cancelled');
      } else if (e.key === 'Enter' && selection && !busy) {
        e.preventDefault();
        void confirmCopy();
      } else if ((e.key === 'Delete' || e.key === 'Backspace') && selectedAnn !== null && !busy) {
        e.preventDefault();
        annotations = annotations.filter((a) => a.id !== selectedAnn);
        selectedAnn = null;
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
    selectedAnn = null;
    drawing = null;
    drawAnchor = null;
    tool = null;
    textEdit = null;
    busy = false;
    cursor = CURSOR_CROSSHAIR;
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
      selectedId: selectedAnn,
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

  /** 顶层优先的标注命中（后画的覆盖先画的）。 */
  function hitAnnotationTopmost(p: Pt): Annotation | null {
    for (let i = annotations.length - 1; i >= 0; i--) {
      if (hitTestAnnotation(annotations[i], p, dpr)) return annotations[i];
    }
    return null;
  }

  /** 选中标注的包围盒四角手柄命中（与选区手柄同一手感）；
   *  标记笔/马赛克不允许改尺寸，无手柄。 */
  function annHandleHitIndex(p: Pt): number | null {
    if (!selectedAnnotation || !isScalableKind(selectedAnnotation.kind)) return null;
    const corners = rectCorners(annBBox(selectedAnnotation, dpr));
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
    } else if (tool) {
      if (tool === 'text') {
        // 必须 cancel pointerdown：否则同一交互序列的 mousedown 默认行为会把焦点
        // 抢回 body，onMount 里的 el.focus() 随即被 onblur 覆盖，空文本触发
        // oncancel，编辑器挂载当帧自毁（表现为点击无反应）
        e.preventDefault();
      }
      startDrawing(p);
    } else {
      // 无工具：标注对象编辑优先于选区操作
      const ahi = annHandleHitIndex(p);
      if (ahi !== null && selectedAnnotation) {
        cursor = CURSOR_BY_CORNER[ahi]; // 角手柄斜向双箭头，按下即反馈
        drag = {
          mode: 'annResize',
          startPt: p,
          startSel: selection ?? { x: 0, y: 0, w: 0, h: 0 },
          corner: ahi,
          annId: selectedAnnotation.id,
          annSnapshot: selectedAnnotation,
          moved: true,
        };
        phase = 'adjust';
      } else {
        const hit = hitAnnotationTopmost(p);
        if (hit) {
          selectedAnn = hit.id;
          cursor = 'move'; // 按下即进入移动语义
          drag = {
            mode: 'annMove',
            startPt: p,
            startSel: selection ?? { x: 0, y: 0, w: 0, h: 0 },
            annId: hit.id,
            annSnapshot: hit,
            moved: false,
          };
        } else {
          selectedAnn = null;
          if (selection && inRect(selection, p)) {
            drag = { mode: 'move', startPt: p, startSel: selection, moved: true };
            phase = 'adjust';
          } else {
            drag = {
              mode: 'select',
              startPt: p,
              startSel: selection ?? { x: p.x, y: p.y, w: 0, h: 0 },
              moved: false,
            };
            phase = 'dragSelect';
            hover = null;
          }
        }
      }
    }
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onPointerMove(e: PointerEvent) {
    if (!virtual) return;
    const p = evPt(e);

    if (drag) {
      drag.moved = drag.moved || dist(drag.startPt, p) > CLICK_SLOP_PX;
      // 拖拽期间的光标即时反馈（悬停分支不会执行）
      if (drag.mode === 'annMove') cursor = 'move';
      else if (drag.mode === 'annResize' && drag.corner !== undefined) {
        cursor = CURSOR_BY_CORNER[drag.corner];
      }
      if (drag.mode === 'select' && !drawing) {
        selection = clampRect(normalizeRect(drag.startPt, p), virtual, MIN_SELECTION);
      } else if (drag.mode === 'move') {
        selection = moveRect(drag.startSel, p.x - drag.startPt.x, p.y - drag.startPt.y, virtual, MIN_SELECTION);
      } else if (drag.mode === 'resize' && drag.corner !== undefined) {
        const anchor = rectCorners(drag.startSel)[(drag.corner + 2) % 4];
        selection = clampRect(normalizeRect(anchor, p), virtual, MIN_SELECTION);
      } else if (drag.mode === 'annMove' && drag.annSnapshot) {
        const moved = moveAnnotation(drag.annSnapshot, p.x - drag.startPt.x, p.y - drag.startPt.y);
        annotations = annotations.map((a) => (a.id === drag.annId ? moved : a));
      } else if (drag.mode === 'annResize' && drag.annSnapshot && drag.corner !== undefined) {
        const anchor = rectCorners(annBBox(drag.annSnapshot, dpr))[(drag.corner + 2) % 4];
        let scaled = resizeAnnotation(drag.annSnapshot, anchor, drag.startPt, p);
        if (scaled.kind === 'text' && scaled.rect) {
          // 字号随比例变化，包围盒须按新字体设置重测
          const m = measureTextSize(scaled.text ?? '', scaled, dpr);
          scaled = { ...scaled, rect: { ...scaled.rect, w: m.w, h: m.h } };
        }
        annotations = annotations.map((a) => (a.id === drag.annId ? scaled : a));
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
      cursor = CURSOR_CROSSHAIR;
    } else {
      hover = null;
      const ahi = annHandleHitIndex(p);
      const hi = handleHitIndex(p);
      if (ahi !== null) cursor = CURSOR_BY_CORNER[ahi];
      else if (hi !== null) cursor = CURSOR_BY_CORNER[hi];
      else if (!tool && selectedAnnotation && inRect(annBBox(selectedAnnotation, dpr), p)) {
        cursor = 'move'; // 已选中对象整体可拖
      } else if (!tool && hitAnnotationTopmost(p)) cursor = 'pointer';
      else if (selection && inRect(selection, p) && !tool) cursor = 'move';
      else cursor = tool ? TOOL_CURSORS[tool] : CURSOR_CROSSHAIR;
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
    if (hitAnnotationTopmost(p)) return; // 双击标注对象不触发复制退出
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
      case 'highlighter':
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
      case 'highlighter':
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
      case 'highlighter':
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
      // 包围盒按实际字体测量（对象化选中/缩放的几何依据）
      const m = measureTextSize(text, st, dpr);
      annotations = [
        ...annotations,
        {
          id: annSeq,
          kind: 'text',
          color: st.color,
          width: st.width,
          font: st.font,
          size: st.size ?? 18,
          bold: st.bold,
          text,
          rect: { x: textEdit.x, y: textEdit.y, w: m.w, h: m.h },
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
    const last = annotations[annotations.length - 1];
    if (last && last.id === selectedAnn) selectedAnn = null;
    annotations = annotations.slice(0, -1);
  }

  function pickTool(t: ToolKind | null) {
    if (phase === 'textEditing') {
      // 点击工具按钮 = 先收起文本编辑器（提交已输入内容）再切换，
      // 不能吞掉这次点击——否则编辑器失焦滞留时所有工具按钮都点不动
      textEditorRef?.commitNow();
      if (phase === 'textEditing') return;
    }
    tool = t;
    selectedAnn = null; // 进入绘制模式，退出对象编辑
    // 立即切换光标，不等首次移动
    cursor = t ? TOOL_CURSORS[t] : CURSOR_CROSSHAIR;
  }

  function patchStyle(patch: Partial<ToolStyle>) {
    // 有选中对象时改的是对象本身（颜色/线宽/字号档/马赛克块），而非工具默认值
    if (selectedAnnotation) {
      let next: Annotation = { ...selectedAnnotation, ...patch };
      if (next.kind === 'text' && next.rect) {
        const m = measureTextSize(next.text ?? '', next, dpr);
        next = { ...next, rect: { ...next.rect, w: m.w, h: m.h } };
      }
      annotations = annotations.map((a) => (a.id === next.id ? next : a));
      return;
    }
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
    if (!selection || busy || phase === 'textEditing') return;
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
    if (!busy && phase !== 'textEditing') void endSession('cancelled');
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

<!-- 禁用 WebView 自带右键菜单（Chromium 右键菜单是 contextmenu 的默认行为，
     preventDefault 即可抑制；截图覆盖层的右键属误触源） -->
<svelte:window oncontextmenu={(e) => e.preventDefault()} />

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
          styleEnabled={!!tool || selectedAnn !== null}
          textMode={tool === 'text' || selectedAnnotation?.kind === 'text'}
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
        bind:this={textEditorRef}
        x={toL(textEdit.x)}
        y={toL(textEdit.y)}
        color={styles.text.color}
        fontSize={styles.text.size ?? 18}
        font={styles.text.font}
        bold={styles.text.bold}
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
