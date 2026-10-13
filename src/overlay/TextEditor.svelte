<script lang="ts">
  /**
   * 就地文本编辑浮层（FR-13）：
   * Enter 提交 / Shift+Enter 换行 / Esc 取消 / 失焦提交。
   * 两层 Esc 语义：此组件 stopPropagation，覆盖层的「退出会话」不触发。
   */
  import { onMount } from 'svelte';

  interface Props {
    x: number;
    y: number;
    color: string;
    fontSize: number;
    maxWidth: number;
    /** 字体族（空 = 默认栈）与加粗，与提交后的标注渲染保持一致 */
    font?: string;
    bold?: boolean;
    oncommit: (text: string) => void;
    oncancel: () => void;
  }

  let { x, y, color, fontSize, maxWidth, font = '', bold = false, oncommit, oncancel }: Props =
    $props();

  let el = $state<HTMLDivElement | undefined>(undefined);
  let done = false;

  onMount(() => {
    el?.focus();
  });

  function commit() {
    if (done) return;
    done = true;
    const text = (el?.innerText ?? '').replace(/\u00a0/g, ' ').trim();
    if (text.length > 0) oncommit(text);
    else oncancel();
  }

  function onKeydown(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      commit();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      done = true;
      oncancel();
    }
  }
</script>

<div
  bind:this={el}
  class="editor"
  role="textbox"
  aria-multiline="true"
  aria-label="文本标注"
  tabindex="0"
  contenteditable="true"
  spellcheck="false"
  style:left="{x}px"
  style:top="{y}px"
  style:color={color}
  style:font-size="{fontSize}px"
  style:font-family={font ? `'${font}', 'Segoe UI', 'Microsoft YaHei', system-ui, sans-serif` : undefined}
  style:font-weight={bold ? '800' : '600'}
  style:max-width="{maxWidth}px"
  onkeydown={onKeydown}
  onblur={(e) => {
    // 焦点移入工具条（如调整字体设置）不提交，点回编辑器继续输入
    const rt = e.relatedTarget as HTMLElement | null;
    if (rt && rt.closest('.toolbar')) return;
    commit();
  }}
  onpointerdown={(e) => e.stopPropagation()}
  ondblclick={(e) => e.stopPropagation()}
></div>

<style>
  .editor {
    position: absolute;
    min-width: 60px;
    min-height: 1.3em;
    padding: 2px 4px;
    background: rgba(255, 255, 255, 0.92);
    outline: 1.5px dashed #3d82f6;
    white-space: pre-wrap;
    word-break: break-word;
    line-height: 1.25;
    font-family: 'Segoe UI', 'Microsoft YaHei', system-ui, sans-serif;
    font-weight: 600;
    z-index: 40;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.35);
    caret-color: currentColor;
  }
</style>
