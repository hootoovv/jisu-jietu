<script lang="ts">
  /**
   * 覆盖层（M1 版：冻结画面 + 全屏蒙版 + Esc 退出）。
   * M2 将接入选区交互，M3/M4 接入标注与工具条。
   */
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { fetchSession, revealOverlay, endSession, logMetric } from '../lib/ipc';
  import type { FrozenPayload } from '../lib/types';

  let session = $state<FrozenPayload | null>(null);
  let loadStart = 0;

  /** WebView 的 devicePixelRatio（方案 c：单窗口主屏 DPI，见开发计划 3.3-⑥） */
  const dpr = window.devicePixelRatio || 1;

  const snapshotUrl = $derived(session ? `snapshot://${session.snapshotId}` : '');

  onMount(() => {
    void boot();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        void endSession('cancelled');
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  async function boot() {
    loadStart = performance.now();
    // 主通道：拉取；兜底：事件（页面加载早于冻结完成的场景）
    let payload = await fetchSession();
    if (!payload) {
      payload = await new Promise<FrozenPayload>((resolve) => {
        void listen<FrozenPayload>('session-frozen', (e) => {
          resolve(e.payload);
        });
      });
    }
    session = payload;
  }

  function onImgLoad() {
    if (!session) return;
    // 首帧渲染后再显示窗口（N1 截止点）
    requestAnimationFrame(() => {
      void revealOverlay();
      const total = session!.hotkeyElapsedMs + (performance.now() - loadStart);
      void logMetric('N1_hotkey_to_mask', total);
    });
  }
</script>

<svelte:head>
  <style>
    html,
    body {
      margin: 0;
      overflow: hidden;
      background: #000;
      cursor: crosshair;
    }
  </style>
</svelte:head>

{#if session}
  <div class="layers">
    <img
      src={snapshotUrl}
      alt=""
      class="frozen"
      draggable="false"
      onload={onImgLoad}
      onerror={() => console.error('snapshot 加载失败')}
    />
    <div class="mask"></div>
  </div>
{:else}
  <div class="waiting"></div>
{/if}

<style>
  .layers {
    position: fixed;
    inset: 0;
    overflow: hidden;
  }

  .frozen {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    user-select: none;
    -webkit-user-drag: none;
  }

  /* M1：全屏 50% 黑蒙版；M2 起改为 canvas 挖洞实现 */
  .mask {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
  }

  .waiting {
    position: fixed;
    inset: 0;
    background: #000;
  }
</style>
