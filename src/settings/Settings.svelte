<script lang="ts">
  /**
   * 设置窗口（FR-01 / 开发计划 5.1.6）：
   * 热键录制（即时生效/冲突提示）、开机自启、默认保存目录、默认格式、测试截图。
   */
  import { onMount } from 'svelte';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import {
    getSettings,
    setAutostart,
    setHotkey,
    triggerCapture,
    updatePreferences,
  } from '../lib/ipc';
  import type { Settings } from '../lib/types';

  let settings = $state<Settings | null>(null);
  let recording = $state(false);
  let message = $state<{ kind: 'ok' | 'err'; text: string } | null>(null);

  onMount(() => {
    void load();
    window.addEventListener('keydown', onKeydown);
    return () => window.removeEventListener('keydown', onKeydown);
  });

  async function load() {
    settings = await getSettings();
  }

  function flash(kind: 'ok' | 'err', text: string) {
    message = { kind, text };
    setTimeout(() => (message = null), 3000);
  }

  /** e.code → 组合键主键名（与 Rust parse_combo 的接受格式对齐）。 */
  function codeToKeyName(code: string): string | null {
    if (code.startsWith('Key')) return code.slice(3);
    if (code.startsWith('Digit')) return code.slice(5);
    if (/^F([1-9]|1[0-2])$/.test(code)) return code;
    if (code === 'PrintScreen' || code === 'Space') return code;
    return null;
  }

  async function onKeydown(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();

    if (e.key === 'Escape') {
      recording = false;
      return;
    }
    const key = codeToKeyName(e.code);
    if (!key) return;

    const mods: string[] = [];
    if (e.ctrlKey) mods.push('Ctrl');
    if (e.altKey) mods.push('Alt');
    if (e.shiftKey) mods.push('Shift');
    if (e.metaKey) mods.push('Win');

    // PrintScreen 允许无修饰键单用；其余主键至少一个修饰键
    if (mods.length === 0 && key !== 'PrintScreen') {
      flash('err', '请至少配合一个修饰键（Ctrl/Alt/Shift/Win）');
      return;
    }
    const combo = [...mods, key].join('+');
    recording = false;
    try {
      const applied = await setHotkey(combo);
      if (settings) settings.hotkey = applied;
      flash('ok', `快捷键已更新为 ${applied}`);
    } catch (err) {
      flash('err', String(err));
    }
  }

  async function toggleAutostart(e: Event) {
    if (!settings) return;
    const enabled = (e.target as HTMLInputElement).checked;
    try {
      await setAutostart(enabled);
      settings.autostart = enabled;
      flash('ok', enabled ? '已开启开机自启' : '已关闭开机自启');
    } catch (err) {
      flash('err', String(err));
      // 失败回读，避免 UI 与实际状态不一致
      await load();
    }
  }

  async function pickSaveDir() {
    if (!settings) return;
    const dir = await openDialog({ directory: true, title: '选择默认保存目录' });
    if (typeof dir === 'string' && dir.length > 0) {
      settings.defaultSaveDir = dir;
      await persistPreferences();
    }
  }

  async function setFormat(fmt: 'png' | 'jpeg') {
    if (!settings) return;
    settings.lastFormat = fmt;
    await persistPreferences();
  }

  async function persistPreferences() {
    if (!settings) return;
    try {
      await updatePreferences(settings.defaultSaveDir, settings.lastFormat);
      flash('ok', '偏好已保存');
    } catch (err) {
      flash('err', String(err));
    }
  }

  async function testCapture() {
    await triggerCapture();
  }
</script>

<main class="panel">
  <header>
    <div class="logo">极速</div>
    <div>
      <h1>极速截图</h1>
      <p class="sub">v1.5.0 · AI赋能，模仿 QQ 截图功能的轻量截图工具。</p>
    </div>
  </header>

  {#if settings}
    <section>
      <h2>截图快捷键</h2>
      <div class="row">
        <button
          class="hotkey"
          class:recording
          onclick={() => (recording = !recording)}
        >
          {recording ? '按下新组合键…（Esc 取消）' : settings.hotkey}
        </button>
        <button class="ghost" onclick={() => (recording = !recording)}>
          {recording ? '取消' : '修改'}
        </button>
      </div>
      <p class="hint">支持 Ctrl / Alt / Shift / Win 修饰键 + 字母 / 数字 / F1-F12 / PrintScreen / Space</p>
    </section>

    <section>
      <h2>偏好</h2>
      <label class="row check">
        <input type="checkbox" checked={settings.autostart} onchange={toggleAutostart} />
        <span>开机自动启动</span>
      </label>

      <div class="row">
        <input
          class="dir"
          type="text"
          readonly
          placeholder="（未设置，保存时选择）"
          value={settings.defaultSaveDir ?? ''}
        />
        <button class="ghost" onclick={pickSaveDir}>浏览…</button>
      </div>

      <div class="row radios">
        <span class="lbl">保存格式</span>
        <label><input type="radio" name="fmt" value="png" checked={settings.lastFormat !== 'jpeg'} onchange={() => setFormat('png')} /> PNG（无损）</label>
        <label><input type="radio" name="fmt" value="jpeg" checked={settings.lastFormat === 'jpeg'} onchange={() => setFormat('jpeg')} /> JPEG（体积小）</label>
      </div>
    </section>

    <section>
      <h2>测试</h2>
      <div class="row">
        <button class="primary" onclick={testCapture}>立即截图</button>
        <span class="hint">等同于按下快捷键（本窗口会被冻结画面覆盖）</span>
      </div>
    </section>
  {/if}

  {#if message}
    <div class="toast {message.kind}">{message.text}</div>
  {/if}
</main>

<style>
  :global(html),
  :global(body) {
    background: #f5f6f8;
  }

  .panel {
    max-width: 520px;
    margin: 0 auto;
    padding: 22px 24px 28px;
    color: #23272f;
  }

  header {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 18px;
  }

  .logo {
    width: 44px;
    height: 44px;
    border-radius: 10px;
    background: linear-gradient(180deg, #2196f3, #0d47a1);
    color: #fff;
    font-weight: 800;
    font-size: 15px;
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 4px 12px rgba(13, 71, 161, 0.35);
  }

  h1 {
    margin: 0;
    font-size: 17px;
  }

  .sub {
    margin: 2px 0 0;
    font-size: 12px;
    color: #8a919e;
  }

  section {
    background: #fff;
    border-radius: 10px;
    padding: 14px 16px 16px;
    margin-bottom: 12px;
    box-shadow: 0 1px 4px rgba(20, 24, 33, 0.06);
  }

  h2 {
    margin: 0 0 10px;
    font-size: 13px;
    color: #5c6472;
    font-weight: 600;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 8px;
  }

  .row:last-child {
    margin-bottom: 0;
  }

  .hotkey {
    min-width: 180px;
    height: 34px;
    padding: 0 14px;
    border: 1px solid #d4d9e2;
    border-radius: 6px;
    background: #fafbfc;
    font: 600 13px Consolas, monospace;
    color: #23272f;
    cursor: pointer;
  }

  .hotkey.recording {
    border-color: #3d82f6;
    color: #3d82f6;
    animation: pulse 1s ease infinite;
  }

  @keyframes pulse {
    50% {
      box-shadow: 0 0 0 4px rgba(61, 130, 246, 0.15);
    }
  }

  .ghost {
    height: 34px;
    padding: 0 14px;
    border: 1px solid #d4d9e2;
    border-radius: 6px;
    background: #fff;
    color: #41495a;
    cursor: pointer;
  }

  .ghost:hover {
    border-color: #3d82f6;
    color: #3d82f6;
  }

  .check {
    cursor: pointer;
    user-select: none;
  }

  .dir {
    flex: 1;
    height: 34px;
    padding: 0 10px;
    border: 1px solid #d4d9e2;
    border-radius: 6px;
    background: #fafbfc;
    font-size: 12px;
    color: #41495a;
  }

  .radios {
    gap: 16px;
    font-size: 13px;
  }

  .radios label {
    display: flex;
    align-items: center;
    gap: 5px;
    cursor: pointer;
  }

  .lbl {
    font-size: 13px;
    color: #41495a;
  }

  .primary {
    height: 34px;
    padding: 0 18px;
    border: none;
    border-radius: 6px;
    background: #3d82f6;
    color: #fff;
    font-weight: 600;
    cursor: pointer;
  }

  .primary:hover {
    background: #2f6fe0;
  }

  .hint {
    margin: 6px 0 0;
    font-size: 12px;
    color: #8a919e;
  }

  .toast {
    position: fixed;
    left: 50%;
    bottom: 18px;
    transform: translateX(-50%);
    padding: 8px 16px;
    border-radius: 6px;
    color: #fff;
    font-size: 13px;
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.25);
    animation: rise 0.2s ease;
  }

  .toast.ok {
    background: #2e9e5b;
  }

  .toast.err {
    background: #d64545;
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translate(-50%, 6px);
    }
  }
</style>
