# PPT 视频生成器 (Rust + Tauri v2 重构版)

基于 **Rust + Tauri v2 + Vue 3 + TypeScript** 打造的高性能跨平台 PPT 转视频桌面应用。

## 🌟 核心特性与重构优化 (落实 2026-10-13 需求)

1. **便携式“单 exe + 文件夹”模式**：
   - Tauri 前端与资源直接编译进单个二进制文件 `PPTVideoGenerator.exe` 中。
   - 自动检测程序同级目录的便携配置 (`config.json`)、外部工具 (`tools/ffmpeg.exe`)、项目文件夹 (`projects/`) 和输出文件夹 (`output/`)。
2. **多角色音色设置优化**：
   - 界面固定显示 `角色1`、`角色2`、`角色3`。
   - 每个角色独立配置音色（知性女声/阳光男声/温柔女声）、专属语速（0.5~2.0x）及脚本角色名绑定。
   - 系统解析脚本后，**首个出现的 `{role:xxx}` 自动填入角色 1**。
   - 脚本中未指定角色或匹配不到的段落，**统一无条件使用角色 1 的音色与语速兜底**。
   - 角色切换时，后台自动注入 **300ms** 静音切片保证对话自然。
3. **支持 `[break=100ms]` 停顿语法**：
   - 脚本中输入 `[break=100ms]` 或 `[break=1.5s]` 时，自动转义为 SSML `<break time="..."/>`。
4. **精简全局设置并支持自定义 TTS 接口**：
   - 移除原全局设置中的默认语速与音色设置（已移至角色配置面板）。
   - 新增 **API 接口调用地址 (Base URL)**，支持切换为本地部署的 CosyVoice 或 OpenAI 兼容端点服务。
5. **跨平台架构抽象**：
   - 核心 PPT 解析模块采用 `PptParser` 接口抽象，Windows 环境调用 Office COM 接口，为未来移植 macOS 预留良好扩展性。

---

## 📂 便携目录结构

编译打包后的输出目录形态：

```text
PPTVideoGenerator/
├── PPTVideoGenerator.exe       # 唯一主程序（内置全部界面与逻辑）
├── config.json                 # 全局配置文件（便携模式优先在此读写）
├── tools/                      # 外置工具目录
│   ├── ffmpeg.exe              # 视频合成引擎
│   └── ffprobe.exe             # 音视频探测工具
├── projects/                   # 工程数据目录（包含各 PPT 切片、音频版本）
└── output/                     # 最终生成的 MP4 视频
```

---

## 🛠 编译与开发指南

### 前置环境需求
1. **Node.js**: v18+（当前测试环境 Node.js v24）
2. **Rust 工具链**: 访问 [rustup.rs](https://rustup.rs) 安装并确保勾选 Visual Studio C++ 生成工具。

### 开发启动
```bash
# 1. 进入新工程目录
cd ppt-video-tauri

# 2. 启动桌面端调试环境 (包含热重载)
npm run tauri dev
```

### 打包生成单 EXE
```bash
npm run tauri build
```
打包产物将位于：`src-tauri/target/release/PPTVideoGenerator.exe`。
将该 exe 与 `tools/` 文件夹放置在同一目录，即可作为独立便携版软件直接分发！
