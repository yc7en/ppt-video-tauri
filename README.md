# PPT 视频生成器（PPTVideoGenerator）

![Version](https://img.shields.io/badge/version-0.1.0-blue)
![Platform](https://img.shields.io/badge/platform-Windows-lightgrey)
![Tauri](https://img.shields.io/badge/Tauri-2.x-24C8DB)
![Rust](https://img.shields.io/badge/Rust-edition%202021-orange)
![Vue](https://img.shields.io/badge/Vue-3.5-42b883)
![License](https://img.shields.io/badge/license-GPLv3-blue)

> 把 PPT / PDF 课件变成可分发视频：逐页写脚本 → 多角色配音 → 烧录字幕 → 一键出片。

🇬🇧 [English Version](README.en.md)

## 项目简介

PPTVideoGenerator 是一款基于 **Rust + Tauri v2 + Vue 3 + TypeScript** 的 Windows 桌面应用，用于把演示文档批量转换成带配音与硬字幕的 MP4 视频。

典型场景是课程、宣讲、产品介绍这类“一页 PPT 一段旁白”的内容：把演示文稿导入后，逐页撰写朗读脚本，程序调用 TTS 引擎按角色合成分句音频，再交给 FFmpeg 把静态切片与音轨合成为视频。全部能力编译进单个可执行文件，以“便携版（exe + 文件夹）”形式分发，不写注册表、不依赖安装程序。

相比逐页手动录音 + 剪辑，整个流程的耗时主要来自 TTS 接口调用，视频合成与字幕对齐由本地 FFmpeg 完成。

## 功能特性

### 演示文档解析

- 支持 **`.pptx` / `.ppt`**：经 Windows COM 调用本机 PowerPoint 导出 PNG 切片，兼容中英文版 Office。
- 支持 **`.pdf`**：经 Windows 原生 WinRT `Windows.Data.Pdf` 以 1920px 宽度高保真渲染，无需额外安装 PDF 组件。
- 解析入口按扩展名自动分流，切片按页码自然排序；解析完成后自动建立工程目录并生成 `project.json`。

### 多角色配音

- 固定 **3 个角色槽位**，每个槽位独立配置音色（下拉读取音色库）与专属语速（0.5x ~ 2.0x）及“脚本中对应的角色名”。
- 脚本内用 `{role:角色名}` 切换发音人；首个出现的角色自动绑定角色 1。
- 未标注角色、或角色名匹配不到的段落，统一兜底到角色 1 的兜底音色。
- 角色切换处自动插入 **300ms 静音**；同一角色相邻句子之间插入可配置空隙 `sentence_gap_ms`（默认 120ms）。
- 「扫描角色」按钮可遍历全篇文案，按出现顺序把角色自动填入槽位并即时落盘。

### 音色库与兜底

- 音色库以 `名称 + Voice ID` 映射维护，内置知性女声 `longyingjing_v3`、阳光男声 `longanyang`、温柔女声 `longxiaochun_v2`。
- 支持新增任意 CosyVoice 音色（如龙安欢 `longanhuan_v3`），并提供常用音色快捷填入。
- 全局只允许一个「唯一默认兜底」音色，互斥切换；作为兜底的音色禁止删除，避免兜底丢失。

### 时间与字幕

- 支持精确停顿 `[break=100ms]`、`[break=1.5s]`（不写单位按毫秒处理）。
- 脚本换行自动转为全角逗号，连续逗号自动合并，为 TTS 提供自然停顿。
- **精准计时**：多段合成时逐句实测时长，写出与音频同名的 `.timing.json` 计时旁车；字幕按字符时间映射对齐。
- **估算兜底**：无计时文件时按字数比例分配整页时长。
- 硬字幕通过 FFmpeg `ass` 滤镜烧录，可配置字体、字号、主色、描边色/宽度、底部边距、每行字数、最大行数；编辑页提供字幕叠层预览。
- 单页可单独填写「字幕文案」，留空则复用朗读文案（自动剔除 `{role:}`、`[break=]`、`<speak>` 等标记）。

### 配音版本与批量处理

- 每页音频按时间戳归档为多版本（`audio/slide_0001/<时间戳>.mp3`），底部下拉可试听与切换，默认选中最新版本。
- 「批量合成全部」按顺序遍历所有非空文案页，逐页合成并即时保存，可中途优雅停止；单页失败记录错误后继续后续页。

### 视频合成

- 逐页生成切片：静态图循环 + `scale/pad` 统一画幅 + 音轨 `apad` 补齐换页停顿，再经 concat 拼接为最终 MP4。
- 分辨率 1920×1080 / 1280×720 / 3840×2160 可选，帧率可调；无音频页自动以默认时长生成静音段。
- 输出统一为 H.264 + AAC（22.05kHz 单声道），带 `+faststart`，便于网页与移动端播放。

### 工程与分发

- 首页提供本地文件选择器、历史项目卡片与 `project.json` 直接载入，二次编辑无需重新解析 PPT。
- 文案支持 Markdown 导入 / 导出（走原生对话框，规避 WebView2 下载限制）。
- 便携模式：程序同级存在 `config.json` 或 `tools/` 即以该目录为根，否则回退 `%LOCALAPPDATA%\PPTVideoGenerator`。

## 工作流

界面固定四步，顶部步骤条可直接跳转：

| 步骤 | 名称 | 主要操作 |
|---|---|---|
| ① | 上传 PPT / PDF | 选择演示文档解析，或载入历史工程 |
| ② | 编辑文案 | 逐页写朗读文案与字幕文案、单页合成、批量合成、导入导出 Markdown |
| ③ | 生成视频 | 多角色音色配置、合成视频、打开输出目录 |
| ④ | 设置 | TTS 接口、音色库、视频参数、字幕参数 |

## 脚本语法

脚本支持角色切换、精确停顿、自然停顿三种控制，可自由组合。

```text
{role:旁白}欢迎收看本期产品介绍。

{role:主持人}大家好，我是主持人小李，很高兴和大家见面。
{role:嘉宾}大家好，我是嘉宾小张。[break=500ms]非常荣幸来到节目现场。

{role:主持人}小张，先给观众做个自我介绍吧。
{role:嘉宾}好的，我是一名软件工程师，专注桌面应用开发。[break=1s]平时喜欢研究音视频技术。
```

| 语法 | 作用 | 说明 |
|---|---|---|
| `{role:角色名}` | 从此处起切换发音人与语速 | 角色名需与槽位中填写的一致，前后空格忽略 |
| `[break=100ms]` / `[break=1.5s]` / `[break=500]` | 精确停顿 | 无单位按毫秒 |
| 换行 | 自然停顿 | 转为全角逗号，连续逗号自动合并 |

完整说明见 [脚本撰写说明](docs/脚本撰写说明.md)。

## 环境要求

- **Windows 10 / 11**（核心能力依赖系统组件，见下）
- **Microsoft Office（含 PowerPoint）**：仅解析 `.ppt` / `.pptx` 时需要
- **Node.js 20.19+ 或 22.12+**（Vite 8 的引擎要求；当前开发环境 Node 22.23）
- **Rust 工具链**：`rustup` + Visual Studio C++ 生成工具（MSVC 链接器）
- **FFmpeg / FFprobe**：置于 `tools/` 目录或加入 `PATH`；仓库不内置二进制

## 快速开始

### 开发调试

```bash
git clone <repo-url>
cd ppt-video-tauri
npm install
npm run tauri dev
```

Vite 固定占用 1420 端口（`strictPort`），被占用时启动会直接失败。

### 打包

```bash
npm run tauri build
```

`tauri.conf.json` 中 `bundle.active = false`，因此**不产安装包**，只输出独立可执行文件：`src-tauri/target/release/PPTVideoGenerator.exe`。

### 部署便携版

```text
PPTVideoGenerator/
├── PPTVideoGenerator.exe     # 复制 release 产物，改名即可
├── config.json               # 首次运行自动生成，可预先配置后随包分发
├── tools/
│   ├── ffmpeg.exe
│   └── ffprobe.exe
├── projects/                 # 自动创建，工程数据
└── output/                   # 自动创建，最终 MP4
```

`config.json` 或 `tools/` 任一存在即判定为便携模式。

## 配置说明

配置持久化在应用根目录 `config.json`，运行时读写，缺失字段按默认值补齐。该文件已在 `.gitignore` 中忽略，`api_key` 以明文保存在本地文件内，不入版本库。

| 配置块 | 字段 | 默认值 | 说明 |
|---|---|---|---|
| `tts` | `api_url` | DashScope TTS 接口 | 支持云端百炼、本地 CosyVoice、OpenAI 兼容端点 |
| | `api_key` | 空 | 本地无鉴权接口可留空 |
| | `model` | `cosyvoice-v3-flash` | 音色模型名 |
| | `sentence_gap_ms` | `120` | 同角色相邻句子空隙 |
| `video` | `fps` | `5` | 合成帧率 |
| | `resolution` | `[1920, 1080]` | 输出画幅 |
| | `default_slide_duration` | `3.0` | 无文案页默认时长（秒） |
| | `slide_pause` | `0.5` | 换页停顿（秒） |
| `role_configs` | `role_id` / `role_name` / `voice` / `speed` | 角色 1~3 | 未填 `role_name` 的槽位不会被脚本命中 |
| `subtitle` | `enabled` | `false` | 硬字幕总开关，关闭时合成零开销 |
| | `font_name` / `font_size` | 微软雅黑 / 48 | 基于 PlayRes 高度 |
| | `primary_color` / `outline_color` / `outline` | `#FFFFFF` / `#000000` / 2 | ASS 样式 |
| | `margin_v` / `max_chars_per_line` / `max_lines` | 60 / 20 / 2 | 布局 |
| | `timing_mode` | `segment` | `segment` 精准 / `estimate` 估算 |
| `custom_voices` | `name` / `voice` / `is_default` | 内置 3 个 | `is_default` 全局唯一 |

### TTS 接口适配

后端按 URL 形态自动选择协议：

| 形态 | 请求体 | 适用 |
|---|---|---|
| 含 `SpeechSynthesizer` 且域名为 `dashscope.aliyuncs.com` | `input` 仅含 `text`，音色/格式/采样率/语速放在 `parameters` | 阿里云百炼云端 |
| 含 `SpeechSynthesizer` 的其他地址 | `input` 内联 `voice/format/sample_rate/rate`，并按音色追加 `instruction` 语气指令 | 本地 CosyVoice 兼容服务 |
| 含 `/audio/speech` | OpenAI 兼容格式 | 任意兼容端点 |
| 其他 | 通用 DashScope generation 结构 | 预留扩展 |

响应侧已兼容三种回传：裸音频字节流、含 OSS 下载地址的 JSON、Base64 内嵌音频。

## 后端命令

前端通过 `invoke` 调用以下 Tauri 命令（`src-tauri/src/lib.rs` 注册）：

| 命令 | 作用 |
|---|---|
| `get_app_config` / `update_app_config` | 读取 / 落盘全局配置 |
| `create_project_cmd` | 解析演示文档并建工程 |
| `save_project_cmd` / `load_project_cmd` | 保存工程 / 从 `project.json` 载入（自动补图片与音频版本） |
| `synthesize_slide_cmd` | 单页合成，写入带时间戳的新版本 |
| `compose_video_cmd` | 合成视频，并复制一份到全局 `output/` |
| `probe_audio_duration_cmd` | ffprobe 读取音频时长 |
| `probe_timing_mode_cmd` | 探测该版本是精准还是估算计时 |
| `detect_first_role_cmd` / `detect_roles_cmd` | 提取首个角色 / 全部角色 |
| `list_recent_projects_cmd` | 扫描 `projects/` 生成历史工程列表 |
| `select_ppt_file_cmd` / `select_project_file_cmd` | 原生文件对话框 |
| `export_markdown_cmd` / `import_markdown_cmd` | 文案导出 / 导入 |
| `open_path_cmd` | 在 Explorer 中定位文件或目录 |

## 架构说明

- **分层**：`commands` 只做参数转换与错误映射，解析、合成、拼接逻辑分别落在 `ppt_parser`、`tts_service`、`video_composer`、`subtitle_service`。
- **解析可替换**：`PptParser` trait 定义了 `export_slides`，当前由 `UniversalPresentationParser` 按扩展名分派到 COM 实现与 WinRT 实现；移植其他平台只需新增实现。
- **合成可回溯**：每段音频落盘后再拼接，拼接失败时 `-c copy` 改为 `libmp3lame` 重编码兜底，避免采样率不一致导致整段失败。
- **计时与字幕解耦**：TTS 只负责产出音频与计时旁车，字幕在合成阶段按需读取，因此关闭字幕不影响配音与计时文件。
- **进程静默**：所有 FFmpeg / PowerShell 子进程在 Windows 下以 `CREATE_NO_WINDOW` 启动，不弹黑框。

## 常见问题

| 现象 | 原因与处理 |
|---|---|
| 合成失败或无声音 | 检查 API Key 与网络；本地接口确认服务已启动且音色 ID 已注册 |
| PPT 无法导入 | 本机未安装 Office，或 PowerPoint 正被占用；关闭 PowerPoint 后重试 |
| 中途报 `0x800C000E` | 历史遗留的 `\\?\` 路径问题，已在 `ppt_parser` 中裁剪前缀 |
| 视频找不到 | 在应用根目录 `output/`，文件名含工程名与时间戳；工程内另存一份 |
| 字幕不显示 | 设置页未开启字幕开关，或系统缺少所选字体（默认微软雅黑） |
| 某角色没换声 | 槽位「脚本中对应的角色名」与 `{role:xxx}` 不一致，或该槽位音色为空；点「扫描角色」重填 |
| 编译报 MSVC 未找到 | 安装 Visual Studio C++ 生成工具，或执行仓库根目录的 `install_cpp_tools.bat` |
| 打包报文件被占用 | 先退出正在运行的 `PPTVideoGenerator.exe` 再重试 |

## 贡献

个人项目，暂不接受外部 Pull Request；欢迎通过 Issue 反馈缺陷与使用问题。

## 许可证

GNU General Public License v3.0（GPL-3.0）— 详见 [LICENSE](LICENSE)

Copyright (C) 2026 Leo

## 作者

Leo
