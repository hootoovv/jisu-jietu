# jisu-jietu 极速截图

模仿 QQ 桌面版（Windows）截图体验的轻量截图工具。

**按热键（默认 `Ctrl+PrintScreen`，可自定义）→ 屏幕冻结并覆盖黑色半透明蒙版 →
拖拽 / 单击智能选窗圈定选区 → 四角手柄精修 → 就地标注 → 复制或保存。**

技术栈：**Rust + Tauri 2 + Svelte 5 (runes) + Vite**（选型论证见开发计划文档）。

## 功能一览

| 功能 | 说明 |
| --- | --- |
| 自定义快捷键 | 默认 `Ctrl+PrintScreen`，设置窗口即时重绑，冲突有人话提示 |
| 冻结 + 蒙版 | BitBlt 全虚拟屏捕获，选区外 50% 压暗、选区内 100% 原亮度 |
| 拖拽选区 | 反向拖拽自动归一化，最小 8px，实时显示 `宽×高` 读数 |
| 智能窗口识别 | 单击（≤4px）自动选中鼠标下窗口；DWM 边界修正去阴影；任务栏/桌面可选 |
| 悬停预高亮 | 鼠标移动即预告将选中的窗口（预览即所得） |
| 四角手柄 / 移动 | 对角锚点缩放；选区内拖拽整体平移 |
| 工具条避让 | 默认选区下方 → 上方 → 全屏内置底部内侧 → 虚拟屏兜底 |
| 八种标注 | 矩形 / 椭圆 / 箭头 / 画笔 / 标记笔（宽笔 40% 高亮）/ 马赛克 / 文本框 / 数字标号（自动重排）；各工具默认颜色互异且独立记忆 |
| 文本排版 | 文本工具/选中文本时可选字体（微软雅黑/宋体/黑体/楷体/仿宋等）、字号（12~72）、加粗；编辑中即可实时调整 |
| Undo | 撤销最近标注，可连续回退 |
| 对象化编辑 | 点击已绘标注选中（虚线框+四角手柄）：拖动移动、角点**任意宽高比**缩放（文本字号/标号大小/标记笔厚度/马赛克块径随缩放同步）、颜色/线宽即时修改，`Delete` 删除；Esc 分层（先取消选中→再退出会话） |
| 8×2 色板 | 16 预设色 + 自定义颜色（取色器 / HEX 输入） |
| 线宽 | 5 档预设（2/4/6/8/12px）+ 1~20px 自定义滑杆 |
| 复制 / 保存 | 标注层与原始无损 BGRA 合成；PNG/JPEG；目录与格式记忆 |
| 取走即走 | Enter / 双击 = 复制退出；Esc = 放弃退出（文本编辑中仅取消输入） |
| 托盘常驻 | 菜单：立即截图 / 设置 / 退出；左键单击直接截图；开机自启可选 |

## 开发环境（Windows）

前置要求：

- Windows 10 21H2+ / Windows 11
- Rust（`stable-x86_64-pc-windows-msvc`）
- Node.js ≥ 20
- WebView2 Runtime（Win10/11 自带）

```powershell
npm install
npm run tauri dev
```

## 测试

```powershell
npm run test                 # 前端纯逻辑单测（vitest，27+ 项）
cargo test --workspace       # Rust 单测（热键解析/位图合成/几何等，30+ 项）
```

Windows 专属代码（GDI 捕获 / 窗口枚举 / 注册表 / 协议）由 CI 在
`windows-latest` 上编译验证；本仓库开发期也可在任意平台执行：

```powershell
rustup target add x86_64-pc-windows-msvc
cargo check -p jetsnap-win --target x86_64-pc-windows-msvc
```

## 打包发布

```powershell
npm run tauri build          # 产出 NSIS 安装包（含 icon.ico）
```

## 项目结构

```
├── index.html / settings.html     # Vite MPA 双入口（覆盖层 / 设置）
├── src/lib/                       # 纯逻辑层（全部可单测）
│   ├── geometry.ts                #   选区/避让/命中/箭头几何（27 项单测）
│   ├── render.ts                  #   Canvas 渲染 + 导出合成（马赛克 source-in）
│   ├── ipc.ts / types.ts          #   Tauri 命令封装 / 类型镜像
│   └── constants.ts               #   色板 / 线宽 / 工具条尺寸
├── src/overlay/                   # 覆盖层 UI（状态机宿主 / 工具条 / 文本编辑）
├── src/settings/                  # 设置窗口（热键录制 / 偏好）
├── crates/jetsnap-win/            # Windows 能力层（跨平台可编译骨架）
│   ├── capture.rs                 #   BitBlt 全虚拟屏捕获 + 显示器 DPI
│   ├── window_enum.rs             #   EnumWindows + DWM 边界修正 + 进程名
│   ├── compose.rs                 #   裁剪/合成/编码 + arboard 剪贴板
│   ├── autostart.rs               #   HKCU Run 键
│   └── hotkey_combo.rs            #   组合键解析（8 项单测）
├── src-tauri/                     # Tauri 应用壳
│   └── src/{lib,session,overlay,protocol,commands,hotkey,tray,state,settings}.rs
└── .github/workflows/ci.yml       # windows-latest：vitest + cargo test + release build
```

## 架构要点

- **显示 / 导出双通道**：显示走 snapshot 自定义协议直出 **32bpp BMP**——BGRA 内存
  布局与该格式逐字节一致，仅拼 122 字节文件头、零像素转换零编码（原 JPEG 编码
  曾占「热键→蒙版」时延的大头），且显示无损；Windows/Android 的 WebView2 将
  自定义协议映射为 `http://snapshot.localhost/<id>`（页面自拼 URL 必须用映射
  形态），协议应答带 `Access-Control-Allow-Origin` 且 `<img crossorigin>`
  以避免马赛克导出的画布跨域污染；导出用 Rust 侧持有的原始无损 BGRA 裁剪合成
  ——速度与画质各取所需。
- **覆盖窗口常驻复用**：WebView2 建进程 + 页面加载是首次出蒙版的大头
  （冷启动数百毫秒起步），窗口改为应用启动时预建、会话结束隐藏而非销毁，
  下次热键直接走内存通道；代价是一份常驻内存（约 60~100MB）。
  实测「热键→蒙版」（N1）≈ 230ms。
- **覆盖窗口不透明**：冻结位图即窗口内容，蒙版是 Canvas 绘制内容，
  绕开透明窗口 + WebView2 的性能坑。
- **坐标纪律**：IPC/绘制一律物理像素，DOM 一律逻辑像素，
  `÷dpr` 换算只存在于 Overlay 一处。
- **会话 RAII**：冻结缓冲 / 覆盖窗口 / 标注状态全部由会话管理器托管，
  确认、取消、窗口被销毁均走同一清理路径。

## 已知限制（V1）

见 [LIMITATIONS.md](./LIMITATIONS.md)。混合 DPI 多显示器当前采用
「单覆盖窗口 + 主屏 DPI」脚手架方案（方案 c），同 DPI 多屏完全正常；
混合 DPI 副屏存在画质/坐标近似，V1.x 切换每屏一窗方案。

## 许可

MIT（示例项目，可自由使用与修改）。
