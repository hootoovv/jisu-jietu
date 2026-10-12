# JetSnap 极速截图

模仿 QQ 桌面版（Windows）截图体验的轻量截图工具：热键冻结屏幕 →
拖拽/点选窗口圈定选区 → 四角手柄精修 → 就地标注（矩形/椭圆/箭头/画笔/
马赛克/文本/序号）→ 复制或保存。

技术栈：**Rust + Tauri 2 + Svelte 5 + Vite**。

> 状态：开发中（按里程碑 M0~M6 推进，见 `../极速截图-开发计划.md`）。

## 开发

前置要求（Windows）：

- Rust（MSVC toolchain）
- Node.js ≥ 20
- WebView2 Runtime（Win10/11 默认自带）

```powershell
npm install
npm run tauri dev
```

## 测试

```powershell
npm run test                 # 前端纯逻辑单测（vitest）
cargo test --workspace       # Rust 单测（含 Windows 模块）
```

## 打包

```powershell
npm run tauri build          # 产出 NSIS 安装包
```

## 结构

```
├── index.html / settings.html   # Vite MPA 双入口（覆盖层 / 设置）
├── src/lib/                     # 纯逻辑（几何/渲染/合成）+ 单测
├── src/overlay/                 # 截图覆盖层 UI
├── src/settings/                # 设置窗口 UI
├── crates/jetsnap-win/          # Windows 能力层（捕获/枚举/合成/热键解析）
└── src-tauri/                   # Tauri 应用壳（会话/协议/命令/托盘）
```
