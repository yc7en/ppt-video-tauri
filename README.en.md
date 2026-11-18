# PPT Video Generator (PPTVideoGenerator)

![Version](https://img.shields.io/badge/version-0.1.0-blue)
![Platform](https://img.shields.io/badge/platform-Windows-lightgrey)
![Tauri](https://img.shields.io/badge/Tauri-2.x-24C8DB)
![Rust](https://img.shields.io/badge/Rust-edition%202021-orange)
![Vue](https://img.shields.io/badge/Vue-3.5-42b883)
![License](https://img.shields.io/badge/license-GPLv3-blue)

> Turn a slide deck into a distributable video: per-slide script → multi-voice narration → burned-in subtitles → one-click export.

🇨🇳 [中文版本](README.md)

## Overview

PPTVideoGenerator is a Windows desktop application built with **Rust + Tauri v2 + Vue 3 + TypeScript** that converts presentation decks into MP4 videos with narration and hard-burned subtitles.

It targets the "one slide, one paragraph of voice-over" format common to courses, briefings and product intros. After importing a deck you write the script slide by slide; the app splits it into per-sentence TTS calls per character, measures each segment, and hands the result to FFmpeg for composition. Everything is compiled into a single executable that ships as a portable folder — no installer, no registry writes.

Compared with recording and editing slide by slide, almost all of the remaining wall-clock time is TTS latency; composition and subtitle alignment run locally.

## Features

### Presentation parsing

- **`.pptx` / `.ppt`** — exported to PNG through Windows COM using the installed PowerPoint, works with both Chinese and English Office builds.
- **`.pdf`** — rendered by the native WinRT `Windows.Data.Pdf` API at 1920px width, no extra PDF tooling required.
- A single entry point dispatches by extension; slides are sorted naturally by page number and a project directory is created automatically.

### Multi-character narration

- **Three character slots**, each with its own voice (picked from the voice library), speaking rate (0.5x–2.0x) and a "character name used in the script" binding.
- `{role:name}` switches the speaker mid-script; the first tag found is auto-assigned to slot 1.
- Unlabelled paragraphs, and names that match no slot, fall back to slot 1's voice.
- A **300 ms** silence is inserted at every speaker change; same-speaker sentences are separated by `sentence_gap_ms` (120 ms by default).
- A **Scan roles** button walks the whole deck in order, fills the slots and saves immediately.

### Voice library and fallback

- Voices are a `name + voice ID` mapping; built in: `longyingjing_v3`, `longanyang`, `longxiaochun_v2`.
- Any CosyVoice voice can be added (e.g. 龙安欢 / `longanhuan_v3`), with quick-fill presets for common ones.
- Exactly one voice may be the default fallback; the flag is mutually exclusive, and the fallback voice cannot be deleted.

### Timing and subtitles

- Precise pauses: `[break=100ms]`, `[break=1.5s]`; a bare number is read as milliseconds.
- Line breaks in the script become full-width commas, and runs of commas collapse, giving TTS natural pauses.
- **Precise timing**: multi-segment synthesis measures each sentence and writes a `.timing.json` sidecar next to the audio; subtitles are aligned by character-level time mapping.
- **Estimate fallback**: without a timing file, page duration is distributed by character count.
- Hard subtitles are burned via the FFmpeg `ass` filter, configurable font, size, primary/outline colour, outline width, bottom margin, characters per line and line count; the editor shows an overlay preview.
- Each slide may carry a separate subtitle text; when empty the narration text is reused with `{role:}`, `[break=]` and `<speak>` tags stripped.

### Audio versions and batching

- Every synthesis is archived as a timestamped version under `audio/slide_0001/`, previewable and switchable from a dropdown, newest selected by default.
- **Synthesize all** queues every non-empty slide, saves after each page and can be stopped safely; a failing page is reported and the queue continues.

### Video composition

- Per-slide segments: still image loop + `scale/pad` to a fixed canvas + `apad` to cover the page pause, then concatenated into the final MP4.
- 1920×1080, 1280×720 or 3840×2160; adjustable frame rate; slides without audio get a silence track at the default duration.
- Output is H.264 + AAC (22.05 kHz mono) with `+faststart` for web and mobile playback.

### Projects and distribution

- Home screen offers a native file picker, recent-project cards and direct `project.json` loading, so re-editing needs no re-parsing.
- Scripts can be imported from and exported to Markdown through native dialogs, avoiding WebView2 download restrictions.
- Portable mode: if `config.json` or `tools/` sits next to the executable, that directory becomes the root; otherwise `%LOCALAPPDATA%\PPTVideoGenerator`.

## Workflow

The UI has four fixed steps, reachable from the top step bar:

| Step | Name | What happens |
|---|---|---|
| ① | Upload PPT / PDF | Pick a deck to parse, or open a previous project |
| ② | Edit script | Write narration and subtitle text per slide, synthesize one or all, import/export Markdown |
| ③ | Generate video | Configure character voices, compose the video, open the output folder |
| ④ | Settings | TTS endpoint, voice library, video parameters, subtitle parameters |

## Script syntax

Scripts combine speaker switching, exact pauses and natural pauses.

```text
{role:Narrator}Welcome to this product introduction.

{role:Host}Hello everyone, I'm your host.[break=300ms] Glad to meet you.
{role:Guest}Hi, I'm the guest speaker.[break=500ms] Delighted to be here.
```

| Syntax | Effect | Notes |
|---|---|---|
| `{role:name}` | Switch voice and rate from this point | Name must match the slot binding; surrounding spaces ignored |
| `[break=100ms]` / `[break=1.5s]` / `[break=500]` | Exact pause | Unit-less values are milliseconds |
| Line break | Natural pause | Converted to a full-width comma |

The Chinese guide in [脚本撰写说明](docs/脚本撰写说明.md) covers the same rules in more detail.

## Requirements

- **Windows 10 / 11**
- **Microsoft Office with PowerPoint** — only needed for `.ppt` / `.pptx` parsing
- **Node.js 20.19+ or 22.12+** (required by Vite 8; developed on Node 22.23)
- **Rust toolchain** — `rustup` plus the Visual Studio C++ build tools (MSVC linker)
- **FFmpeg / FFprobe** — put them in `tools/` or on `PATH`; binaries are not bundled in the repo

## Quick start

### Development

```bash
git clone <repo-url>
cd ppt-video-tauri
npm install
npm run tauri dev
```

Vite pins port 1420 with `strictPort`, so a busy port fails the start instead of silently moving.

### Build

```bash
npm run tauri build
```

`bundle.active = false` in `tauri.conf.json`, so **no installer is produced** — only the standalone `src-tauri/target/release/PPTVideoGenerator.exe`.

### Portable deployment

```text
PPTVideoGenerator/
├── PPTVideoGenerator.exe     # release binary, renamed
├── config.json               # generated on first run; may be pre-configured
├── tools/
│   ├── ffmpeg.exe
│   └── ffprobe.exe
├── projects/                 # created on demand, project data
└── output/                   # created on demand, final MP4 files
```

Either `config.json` or `tools/` marks the folder as portable.

## Configuration

Settings live in `config.json` at the application root; missing fields fall back to defaults. The file is ignored by `.gitignore`, and `api_key` is stored in plain text in the local file only.

| Block | Field | Default | Meaning |
|---|---|---|---|
| `tts` | `api_url` | DashScope TTS endpoint | Cloud Bailian, local CosyVoice or OpenAI-compatible |
| | `api_key` | empty | May be empty for local unauthenticated endpoints |
| | `model` | `cosyvoice-v3-flash` | Voice model name |
| | `sentence_gap_ms` | `120` | Gap between sentences of the same speaker |
| `video` | `fps` | `5` | Composition frame rate |
| | `resolution` | `[1920, 1080]` | Output canvas |
| | `default_slide_duration` | `3.0` | Duration for slides without script |
| | `slide_pause` | `0.5` | Pause after each slide |
| `role_configs` | `role_id` / `role_name` / `voice` / `speed` | slots 1–3 | A slot without `role_name` can never be matched |
| `subtitle` | `enabled` | `false` | Master switch; off means zero composition cost |
| | `font_name` / `font_size` | Microsoft YaHei / 48 | Sized against PlayRes height |
| | `primary_color` / `outline_color` / `outline` | `#FFFFFF` / `#000000` / 2 | ASS style |
| | `margin_v` / `max_chars_per_line` / `max_lines` | 60 / 20 / 2 | Layout |
| | `timing_mode` | `segment` | `segment` precise / `estimate` proportional |
| `custom_voices` | `name` / `voice` / `is_default` | 3 built-in | `is_default` is globally unique |

### TTS adapters

The protocol is chosen from the URL shape:

| Shape | Request body | Target |
|---|---|---|
| Contains `SpeechSynthesizer` on `dashscope.aliyuncs.com` | `input` holds only `text`; voice, format, sample rate and rate go in `parameters` | Alibaba Cloud Bailian |
| Contains `SpeechSynthesizer` elsewhere | `input` carries `voice/format/sample_rate/rate` plus an `instruction` tone hint | Local CosyVoice-compatible servers |
| Contains `/audio/speech` | OpenAI-compatible payload | Any compatible endpoint |
| Anything else | Generic DashScope generation payload | Fallback |

Responses are normalised from three shapes: raw audio bytes, JSON containing an OSS download URL, and Base64-embedded audio.

## Backend commands

| Command | Purpose |
|---|---|
| `get_app_config` / `update_app_config` | Read / persist global configuration |
| `create_project_cmd` | Parse a deck and create a project |
| `save_project_cmd` / `load_project_cmd` | Save a project / load it (images and audio versions restored) |
| `synthesize_slide_cmd` | Synthesize one slide into a new timestamped version |
| `compose_video_cmd` | Compose the video and copy it to the global `output/` |
| `probe_audio_duration_cmd` | Duration via ffprobe |
| `probe_timing_mode_cmd` | Whether this version is precise or estimated |
| `detect_first_role_cmd` / `detect_roles_cmd` | First role / all roles in a script |
| `list_recent_projects_cmd` | Scan `projects/` for recent projects |
| `select_ppt_file_cmd` / `select_project_file_cmd` | Native file dialogs |
| `export_markdown_cmd` / `import_markdown_cmd` | Script export / import |
| `open_path_cmd` | Reveal a file or folder in Explorer |

## Architecture

- **Layered**: `commands` only converts arguments and maps errors; parsing, synthesis, concatenation and subtitle logic live in their own modules.
- **Swappable parser**: `PptParser::export_slides` is implemented by `UniversalPresentationParser`, which dispatches to the COM and WinRT implementations by extension — porting to another OS means adding one implementation.
- **Traceable synthesis**: each sentence lands on disk before concatenation, and a failed `-c copy` concat falls back to `libmp3lame` re-encoding instead of failing the whole page.
- **Decoupled subtitles**: TTS only produces audio plus the timing sidecar; subtitles are read at composition time, so disabling them costs nothing earlier in the pipeline.
- **Silent subprocesses**: FFmpeg and PowerShell children start with `CREATE_NO_WINDOW` on Windows, so no console flashes.

## FAQ

| Symptom | Cause and fix |
|---|---|
| Synthesis fails or is silent | Check the API key and network; for local endpoints confirm the service runs and the voice ID is registered |
| Deck will not import | Office is not installed, or PowerPoint holds the file — close PowerPoint and retry |
| `0x800C000E` error | The legacy `\\?\` verbatim path prefix; already stripped in `ppt_parser` |
| Cannot find the video | It is under `output/` in the application root, named with deck and timestamp; a copy also stays in the project |
| No subtitles | The master switch is off in Settings, or the system lacks the chosen font (Microsoft YaHei by default) |
| A character keeps the wrong voice | The slot's script name differs from `{role:name}`, or its voice is empty — press **Scan roles** |
| Build fails on MSVC | Install the Visual Studio C++ build tools, or run `install_cpp_tools.bat` |
| Build fails on a locked file | Exit the running `PPTVideoGenerator.exe` first |

## Contributing

Personal project; external pull requests are not accepted for now. Issues with bugs and usage questions are welcome.

## License

GNU General Public License v3.0 (GPL-3.0) — see [LICENSE](LICENSE)

Copyright (C) 2026 Leo

## Author

Leo
