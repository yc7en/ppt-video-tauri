use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{bail, Context, Result};
use regex::Regex;
use crate::config::PathManager;
use crate::models::{RoleVoiceConfig, TtsConfig};

#[derive(Debug, Clone)]
pub struct TtsChunk {
    pub role_name: String,
    pub voice_id: String,
    pub speed: f32,
    pub text: String,
}

/// 精准字幕模式下的脚本片段
#[derive(Debug, Clone)]
pub enum ScriptSegment {
    /// 需要合成的语音片段
    Speech {
        role_name: String,
        voice_id: String,
        speed: f32,
        text: String,
    },
    /// 显式停顿（毫秒）
    Gap { ms: u64 },
}

pub struct TtsService;

impl TtsService {
    /// 解析文案中的角色标签并完成 [break=100ms] 的转义
    pub fn parse_script(
        text: &str,
        role_configs: &[RoleVoiceConfig],
    ) -> (Vec<TtsChunk>, Option<String>) {
        let cleaned_bom = text.trim_start_matches('\u{feff}');
        if cleaned_bom.trim().is_empty() {
            return (Vec::new(), None);
        }

        // 1. 清洗可能导致 CosyVoice 引擎卡顿或生成 10 秒死寂空白的非标/外来 speak 标签（如 <speak effect="eq"...> 与 </speak>）
        let speak_re = Regex::new(r"(?i)</?speak[^>]*>").unwrap();
        let cleaned = speak_re.replace_all(cleaned_bom, "").to_string();

        // 2. 将换行转为全角逗号提供自然停顿，并消除连续多重逗号
        let newline_re = Regex::new(r"[\r\n]+").unwrap();
        let normalized = newline_re.replace_all(&cleaned, "，").to_string();
        let comma_re = Regex::new(r"[，,]{2,}").unwrap();
        let normalized = comma_re.replace_all(&normalized, "，").to_string();

        // 3. 将 [break=100ms] 转为 SSML <break time="100ms"/>
        let break_re = Regex::new(r"\[break=(.*?)\]").unwrap();
        let ssml_text = break_re.replace_all(&normalized, r#"<break time="$1"/>"#).to_string();

        // 3. 解析 {role:角色名}
        let role_re = Regex::new(r"\{role:(.*?)\}").unwrap();
        
        let default_voice = crate::config::load_config().get_default_voice();
        let mut fallback_role = role_configs
            .iter()
            .find(|r| r.role_id == 1)
            .cloned()
            .unwrap_or(RoleVoiceConfig {
                role_id: 1,
                role_name: "".to_string(),
                voice: default_voice.clone(),
                speed: 1.0,
            });
        if fallback_role.voice.trim().is_empty() {
            fallback_role.voice = default_voice.clone();
        }

        let mut first_detected_role = None;
        let mut chunks = Vec::new();

        // 查找所有匹配
        let mut last_end = 0;
        let mut current_role_name = "".to_string();

        for cap in role_re.captures_iter(&ssml_text) {
            let m = cap.get(0).unwrap();
            let role_name = cap.get(1).unwrap().as_str().trim().to_string();

            if first_detected_role.is_none() && !role_name.is_empty() {
                first_detected_role = Some(role_name.clone());
            }

            let segment = ssml_text[last_end..m.start()].trim();
            if !segment.is_empty() {
                let (voice_id, speed) = Self::resolve_role(&current_role_name, role_configs, &fallback_role);
                chunks.push(TtsChunk {
                    role_name: current_role_name.clone(),
                    voice_id,
                    speed,
                    text: segment.to_string(),
                });
            }

            current_role_name = role_name;
            last_end = m.end();
        }

        let tail = ssml_text[last_end..].trim();
        if !tail.is_empty() {
            let (voice_id, speed) = Self::resolve_role(&current_role_name, role_configs, &fallback_role);
            chunks.push(TtsChunk {
                role_name: current_role_name,
                voice_id,
                speed,
                text: tail.to_string(),
            });
        }

        (chunks, first_detected_role)
    }

    /// 根据角色名获取对应音色与语速，未匹配时统一兜底到角色 1
    fn resolve_role(
        name: &str,
        role_configs: &[RoleVoiceConfig],
        fallback: &RoleVoiceConfig,
    ) -> (String, f32) {
        if name.is_empty() {
            return (fallback.voice.clone(), fallback.speed);
        }

        for cfg in role_configs {
            if !cfg.role_name.is_empty() && cfg.role_name.trim() == name.trim() {
                return (cfg.voice.clone(), cfg.speed);
            }
        }

        // 未匹配兜底
        (fallback.voice.clone(), fallback.speed)
    }

    /// 解析 [break=100ms] / [break=1.5s] 的时长（毫秒）
    fn parse_break_ms(raw: &str) -> u64 {
        let t = raw.trim().to_lowercase();
        if let Some(v) = t.strip_suffix("ms") {
            v.trim().parse::<f64>().ok().map(|x| x.round() as u64).unwrap_or(0)
        } else if let Some(v) = t.strip_suffix('s') {
            v.trim().parse::<f64>().ok().map(|x| (x * 1000.0).round() as u64).unwrap_or(0)
        } else {
            t.parse::<f64>().ok().map(|x| x.round() as u64).unwrap_or(0)
        }
    }

    /// 将文本按句末标点切分为句子（供 TTS 逐句合成使用）
    fn split_sentences(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = String::new();
        for c in text.chars() {
            cur.push(c);
            if matches!(c, '。' | '！' | '？' | '!' | '?' | '；' | ';' | '\n') {
                let s = cur.trim().to_string();
                if !s.is_empty() {
                    out.push(s);
                }
                cur.clear();
            }
        }
        let s = cur.trim().to_string();
        if !s.is_empty() {
            out.push(s);
        }
        out
    }

    /// 将整页文案切分为「语音 / 停顿」片段序列（保留角色切换与 break 语义）
    pub fn segment_script(text: &str, role_configs: &[RoleVoiceConfig]) -> Vec<ScriptSegment> {
        let cleaned_bom = text.trim_start_matches('\u{feff}');
        if cleaned_bom.trim().is_empty() {
            return Vec::new();
        }

        let speak_re = Regex::new(r"(?i)</?speak[^>]*>").unwrap();
        let cleaned = speak_re.replace_all(cleaned_bom, "").to_string();

        let default_voice = crate::config::load_config().get_default_voice();
        let mut fallback_role = role_configs
            .iter()
            .find(|r| r.role_id == 1)
            .cloned()
            .unwrap_or(RoleVoiceConfig {
                role_id: 1,
                role_name: "".to_string(),
                voice: default_voice.clone(),
                speed: 1.0,
            });
        if fallback_role.voice.trim().is_empty() {
            fallback_role.voice = default_voice.clone();
        }

        let role_re = Regex::new(r"\{role:(.*?)\}").unwrap();
        let break_re = Regex::new(r"\[break=(.*?)\]").unwrap();

        let mut segments = Vec::new();
        let mut last_end = 0usize;
        let mut current_role = String::new();

        // 内部闭包：把一段纯文本按 break / 句子切分并加入片段
        let push_text = |txt: &str,
                         role_name: &str,
                         segments: &mut Vec<ScriptSegment>| {
            let (voice, speed) = Self::resolve_role(role_name, role_configs, &fallback_role);
            let mut last = 0usize;
            let handle_part = |part: &str, segments: &mut Vec<ScriptSegment>| {
                for sent in Self::split_sentences(part) {
                    let s = sent.trim().trim_matches('\u{feff}').to_string();
                    if s.is_empty() {
                        continue;
                    }
                    segments.push(ScriptSegment::Speech {
                        role_name: role_name.to_string(),
                        voice_id: voice.clone(),
                        speed,
                        text: s,
                    });
                }
            };
            for cap in break_re.captures_iter(txt) {
                let m = cap.get(0).unwrap();
                handle_part(&txt[last..m.start()], segments);
                let ms = Self::parse_break_ms(cap.get(1).unwrap().as_str());
                if ms > 0 {
                    segments.push(ScriptSegment::Gap { ms });
                }
                last = m.end();
            }
            handle_part(&txt[last..], segments);
        };

        for cap in role_re.captures_iter(&cleaned) {
            let m = cap.get(0).unwrap();
            let role_name = cap.get(1).unwrap().as_str().trim().to_string();
            push_text(&cleaned[last_end..m.start()], &current_role, &mut segments);
            current_role = role_name;
            last_end = m.end();
        }
        push_text(&cleaned[last_end..], &current_role, &mut segments);

        segments
    }

    /// 生成静音 mp3 片段（时长毫秒）
    fn generate_silence_mp3(output_path: &Path, duration_ms: u64) -> Result<()> {
        let ffmpeg_bin = PathManager::ffmpeg_bin();
        let secs = duration_ms as f64 / 1000.0;
        let status = Self::new_command(&ffmpeg_bin)
            .args([
                "-f", "lavfi",
                "-i", "anullsrc=r=22050:cl=mono",
                "-t", &format!("{:.3}", secs),
                "-q:a", "9",
                "-acodec", "libmp3lame",
                "-y",
                output_path.to_str().unwrap(),
            ])
            .status()
            .context("生成静音片段失败")?;
        if !status.success() {
            bail!("生成静音片段失败");
        }
        Ok(())
    }

    /// 写入计时旁车文件
    fn write_timing(output_path: &Path, timing: &crate::subtitle_service::TimingFile) -> Result<()> {
        let p = crate::subtitle_service::timing_path_for_audio(output_path);
        let json = serde_json::to_string_pretty(timing).context("序列化计时文件失败")?;
        fs::write(p, json)?;
        Ok(())
    }

    /// 精准字幕模式：逐段合成 + 实测时长，产出音频与计时旁车（T2）
    pub async fn synthesize_slide_audio_with_timing(
        text: &str,
        output_path: &Path,
        tts_config: &TtsConfig,
        role_configs: &[RoleVoiceConfig],
    ) -> Result<(PathBuf, crate::subtitle_service::TimingFile)> {
        use crate::subtitle_service::{TimingFile, TimingUnit};

        let segments = Self::segment_script(text, role_configs);
        let speech_count = segments
            .iter()
            .filter(|s| matches!(s, ScriptSegment::Speech { .. }))
            .count();
        if speech_count == 0 {
            bail!("待合成文本为空");
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // 单段且无空隙：直接合成快路径
        if segments.len() == 1 {
            if let ScriptSegment::Speech { voice_id, speed, text: t, .. } = &segments[0] {
                Self::call_tts_api(t, voice_id, *speed, output_path, tts_config).await?;
                let dur = Self::probe_duration(output_path);
                let dur_ms = (dur * 1000.0).round() as u64;
                let timing = TimingFile {
                    mode: "segment".to_string(),
                    units: vec![TimingUnit {
                        text: t.clone(),
                        begin_ms: 0,
                        end_ms: dur_ms,
                    }],
                };
                Self::write_timing(output_path, &timing)?;
                return Ok((output_path.to_path_buf(), timing));
            }
        }

        // 多段：逐段合成 + 插入空隙 + concat
        let temp_dir = output_path
            .parent()
            .unwrap_or(Path::new("."))
            .join(format!("ttmp_{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir_all(&temp_dir)?;

        let ffmpeg_bin = PathManager::ffmpeg_bin();
        let mut concat_items: Vec<PathBuf> = Vec::new();
        let mut units: Vec<TimingUnit> = Vec::new();
        let mut offset_ms: u64 = 0;
        let mut prev_role: Option<String> = None;
        let mut seg_counter = 0usize;
        let mut silence_counter = 0usize;

        for seg in &segments {
            match seg {
                ScriptSegment::Speech { role_name, voice_id, speed, text: t } => {
                    if let Some(prev) = &prev_role {
                        let gap = if prev == role_name {
                            tts_config.sentence_gap_ms as u64
                        } else {
                            300
                        };
                        if gap > 0 {
                            let sil = temp_dir.join(format!("gap_{:04}.mp3", silence_counter));
                            silence_counter += 1;
                            Self::generate_silence_mp3(&sil, gap)?;
                            concat_items.push(sil);
                            offset_ms += gap;
                        }
                    }
                    let chunk = temp_dir.join(format!("spk_{:04}.mp3", seg_counter));
                    seg_counter += 1;
                    Self::call_tts_api(t, voice_id, *speed, &chunk, tts_config).await?;
                    let dur = Self::probe_duration(&chunk);
                    let dur_ms = (dur * 1000.0).round() as u64;
                    units.push(TimingUnit {
                        text: t.clone(),
                        begin_ms: offset_ms,
                        end_ms: offset_ms + dur_ms,
                    });
                    offset_ms += dur_ms;
                    concat_items.push(chunk);
                    prev_role = Some(role_name.clone());
                }
                ScriptSegment::Gap { ms } => {
                    if *ms > 0 {
                        let sil = temp_dir.join(format!("gap_{:04}.mp3", silence_counter));
                        silence_counter += 1;
                        Self::generate_silence_mp3(&sil, *ms)?;
                        concat_items.push(sil);
                        offset_ms += *ms;
                    }
                    prev_role = None;
                }
            }
        }

        let list_path = temp_dir.join("concat.txt");
        let mut content = String::new();
        for p in &concat_items {
            let p_str = p.to_string_lossy().replace('\\', "/");
            let clean = p_str.strip_prefix("//?/").unwrap_or(&p_str);
            content.push_str(&format!("file '{}'\n", clean));
        }
        fs::write(&list_path, content)?;

        let mut cmd = Self::new_command(&ffmpeg_bin);
        cmd.args([
            "-f", "concat",
            "-safe", "0",
            "-i", list_path.to_str().unwrap(),
            "-c", "copy",
            "-y",
            output_path.to_str().unwrap(),
        ]);
        let output = cmd.output().context("启动 FFmpeg 拼接多段音频失败")?;

        if !output.status.success() {
            // 回退策略：若 -c copy 失败（如音频采样率、通道数不一致），使用 libmp3lame 统一重编码合并
            let mut fb_cmd = Self::new_command(&ffmpeg_bin);
            fb_cmd.args([
                "-f", "concat",
                "-safe", "0",
                "-i", list_path.to_str().unwrap(),
                "-c:a", "libmp3lame",
                "-q:a", "2",
                "-y",
                output_path.to_str().unwrap(),
            ]);
            let fb_output = fb_cmd.output().context("启动 FFmpeg 重编码拼接失败")?;
            if !fb_output.status.success() {
                let err = String::from_utf8_lossy(&fb_output.stderr);
                let _ = fs::remove_dir_all(&temp_dir);
                bail!("多段音频拼接失败: {}", err.trim());
            }
        }

        let _ = fs::remove_dir_all(&temp_dir);

        let timing = TimingFile {
            mode: "segment".to_string(),
            units,
        };
        Self::write_timing(output_path, &timing)?;
        Ok((output_path.to_path_buf(), timing))
    }

    /// 合成整页音频（多角色自动切片并插入 300ms 角色切换停顿）
    pub async fn synthesize_slide_audio(
        text: &str,
        output_path: &Path,
        tts_config: &TtsConfig,
        role_configs: &[RoleVoiceConfig],
    ) -> Result<PathBuf> {
        let (chunks, _) = Self::parse_script(text, role_configs);
        if chunks.is_empty() {
            bail!("待合成文本为空");
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // 单片段直接合成
        if chunks.len() == 1 {
            let c = &chunks[0];
            return Self::call_tts_api(&c.text, &c.voice_id, c.speed, output_path, tts_config).await;
        }

        // 多片段：合成各分段 + 生成 300ms 静音 + FFmpeg concat 合并
        let temp_dir = output_path
            .parent()
            .unwrap_or(Path::new("."))
            .join(format!("tmp_{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir_all(&temp_dir)?;

        let mut segment_paths = Vec::new();
        for (i, c) in chunks.iter().enumerate() {
            let seg_path = temp_dir.join(format!("chunk_{:03}.mp3", i));
            Self::call_tts_api(&c.text, &c.voice_id, c.speed, &seg_path, tts_config).await?;
            segment_paths.push(seg_path);
        }

        // 生成 300ms 静音
        let ffmpeg_bin = PathManager::ffmpeg_bin();
        let silence_path = temp_dir.join("silence_300ms.mp3");
        let silence_status = Self::new_command(&ffmpeg_bin)
            .args([
                "-f", "lavfi",
                "-i", "anullsrc=r=22050:cl=mono",
                "-t", "0.3",
                "-q:a", "9",
                "-acodec", "libmp3lame",
                "-y",
                silence_path.to_str().unwrap(),
            ])
            .status()
            .context("Failed to generate 300ms silence segment via ffmpeg")?;

        if !silence_status.success() {
            bail!("生成 300ms 静音切片失败");
        }

        // 拼接列表
        let concat_list_path = temp_dir.join("concat_list.txt");
        let mut list_content = String::new();
        for (i, seg) in segment_paths.iter().enumerate() {
            let p_str = seg.to_string_lossy().replace('\\', "/");
            let clean = p_str.strip_prefix("//?/").unwrap_or(&p_str);
            list_content.push_str(&format!("file '{}'\n", clean));
            if i < segment_paths.len() - 1 {
                let s_str = silence_path.to_string_lossy().replace('\\', "/");
                let s_clean = s_str.strip_prefix("//?/").unwrap_or(&s_str);
                list_content.push_str(&format!("file '{}'\n", s_clean));
            }
        }
        fs::write(&concat_list_path, list_content)?;

        // FFmpeg concat
        let mut cmd = Self::new_command(&ffmpeg_bin);
        cmd.args([
            "-f", "concat",
            "-safe", "0",
            "-i", concat_list_path.to_str().unwrap(),
            "-c", "copy",
            "-y",
            output_path.to_str().unwrap(),
        ]);
        let output = cmd.output().context("Failed to concat audio segments via ffmpeg")?;

        if !output.status.success() {
            let mut fb_cmd = Self::new_command(&ffmpeg_bin);
            fb_cmd.args([
                "-f", "concat",
                "-safe", "0",
                "-i", concat_list_path.to_str().unwrap(),
                "-c:a", "libmp3lame",
                "-q:a", "2",
                "-y",
                output_path.to_str().unwrap(),
            ]);
            let fb_output = fb_cmd.output().context("启动 FFmpeg 重编码拼接失败")?;
            if !fb_output.status.success() {
                let err = String::from_utf8_lossy(&fb_output.stderr);
                let _ = fs::remove_dir_all(&temp_dir);
                bail!("多角色音频拼接失败: {}", err.trim());
            }
        }

        let _ = fs::remove_dir_all(&temp_dir);

        Ok(output_path.to_path_buf())
    }

    fn new_command(bin: &Path) -> Command {
        let mut cmd = Command::new(bin);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        cmd
    }

    /// 从 TTS 响应数据中提取音频（支持二进制流、阿里云/DashScope JSON 中的 OSS URL 或 Base64，自动捕获 API 报错）
    async fn save_audio_from_bytes(
        client: &reqwest::Client,
        bytes: &[u8],
        output_path: &Path,
    ) -> Result<PathBuf> {
        if bytes.starts_with(b"{") {
            if let Ok(val) = serde_json::from_slice::<serde_json::Value>(bytes) {
                // 1. 检查是否存在明确的 API 错误信息
                if let Some(err_msg) = val.pointer("/message")
                    .or_else(|| val.pointer("/error/message"))
                    .or_else(|| val.pointer("/error"))
                    .and_then(|v| v.as_str())
                {
                    let code = val.pointer("/code")
                        .or_else(|| val.pointer("/error/code"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("API_ERROR");
                    bail!("TTS 接口返回错误 [{}]: {}", code, err_msg);
                }

                // 2. 提取音频 URL（支持 DashScope {"output": {"audio": {"url": "..."}}} 等结构）
                let audio_url = val.pointer("/output/audio/url")
                    .or_else(|| val.pointer("/output/audio_url"))
                    .or_else(|| val.pointer("/output/audio"))
                    .or_else(|| val.pointer("/audio_url"))
                    .or_else(|| val.pointer("/url"))
                    .and_then(|v| v.as_str());

                if let Some(url) = audio_url {
                    if url.starts_with("http://") || url.starts_with("https://") {
                        let audio_resp = client.get(url).send().await.context("从 OSS/URL 下载音频文件失败")?;
                        if !audio_resp.status().is_success() {
                            bail!("下载音频失败 (HTTP {}): {}", audio_resp.status(), url);
                        }
                        let audio_bytes = audio_resp.bytes().await?;
                        fs::write(output_path, audio_bytes)?;
                        return Ok(output_path.to_path_buf());
                    }
                }

                // 3. 提取 Base64 编码的音频流
                let b64_str = val.pointer("/output/audio/data")
                    .or_else(|| val.pointer("/output/data"))
                    .or_else(|| val.pointer("/audio_data"))
                    .or_else(|| val.pointer("/data"))
                    .and_then(|v| v.as_str());

                if let Some(b64) = b64_str {
                    if !b64.trim().is_empty() {
                        use base64::Engine;
                        let decoded = base64::engine::general_purpose::STANDARD
                            .decode(b64.trim())
                            .context("Base64 音频解码失败")?;
                        fs::write(output_path, decoded)?;
                        return Ok(output_path.to_path_buf());
                    }
                }

                bail!("TTS 接口返回了非预期的 JSON 响应，未能提取有效音频: {}", String::from_utf8_lossy(bytes));
            }
        }

        // 直接写入二进制音频流（如 MP3/WAV）
        fs::write(output_path, bytes)?;
        Ok(output_path.to_path_buf())
    }

    /// 发起单段 TTS 请求（支持云端 DashScope、本地 SpeechSynthesizer 和 OpenAI 兼容服务）
    async fn call_tts_api(
        text: &str,
        voice: &str,
        speed: f32,
        output_path: &Path,
        cfg: &TtsConfig,
    ) -> Result<PathBuf> {
        let client = reqwest::Client::new();

        // 规范化 URL
        let mut url = cfg.api_url.trim().to_string();
        if url.is_empty() {
            url = "https://dashscope.aliyuncs.com/api/v1/services/audio/tts/generation".to_string();
        } else {
            let trimmed = url.trim_end_matches('/');
            if trimmed == "http://127.0.0.1:8890" || trimmed == "http://localhost:8890" {
                url = format!("{}/api/v1/services/audio/tts/SpeechSynthesizer", trimmed);
            }
        }

        let model_name = if cfg.model.trim().is_empty() {
            "cosyvoice-v3-flash"
        } else {
            cfg.model.trim()
        };

        // 1. 本地/阿里 SpeechSynthesizer 接口 (如 http://127.0.0.1:8890/api/v1/services/audio/tts/SpeechSynthesizer)
        if url.contains("SpeechSynthesizer") {
            let instruction = match voice {
                "longanyang" => Some("用阳光年轻男声说话"),
                "longshuo" => Some("用沉稳成熟中年男声说话"),
                "longcheng" => Some("用富有磁性稳重的男声说话"),
                "longyingjing_v3" => Some("用知性温和女声说话"),
                "longxiaochun_v2" => Some("用温柔柔和女声说话"),
                "longanhuan_v3" => Some("用欢快活泼亲切女声说话"),
                _ if voice.contains("男") || voice.contains("yang") || voice.contains("male") => Some("用阳光年轻男声说话"),
                _ if voice.contains("女") || voice.contains("female") || voice.contains("huan") || voice.contains("chun") => Some("用知性温和女声说话"),
                _ => None,
            };

            let mut input_obj = serde_json::json!({
                "text": text,
                "voice": voice,
                "format": "mp3",
                "sample_rate": 22050,
                "rate": speed
            });
            if let Some(inst) = instruction {
                input_obj["instruction"] = serde_json::Value::String(inst.to_string());
            }

            let body = serde_json::json!({
                "model": model_name,
                "input": input_obj
            });

            let mut req = client.post(&url).json(&body);
            if !cfg.api_key.trim().is_empty() {
                req = req.header("Authorization", format!("Bearer {}", cfg.api_key.trim()));
            }

            let resp = req.send().await.context("Failed to request SpeechSynthesizer TTS")?;
            if !resp.status().is_success() {
                let status = resp.status();
                let err_text = resp.text().await.unwrap_or_default();
                bail!("Local/Ali TTS Error ({}): {}", status, err_text);
            }
            let bytes = resp.bytes().await?;
            return Self::save_audio_from_bytes(&client, &bytes, output_path).await;
        }

        // 2. OpenAI 兼容接口 (/v1/audio/speech)
        if url.contains("/audio/speech") {
            let body = serde_json::json!({
                "model": if cfg.model.trim().is_empty() { "tts-1" } else { cfg.model.trim() },
                "input": text,
                "voice": voice,
                "speed": speed
            });

            let mut req = client.post(&url).json(&body);
            if !cfg.api_key.trim().is_empty() {
                req = req.header("Authorization", format!("Bearer {}", cfg.api_key.trim()));
            }

            let resp = req.send().await.context("Failed to request OpenAI-compatible TTS")?;
            if !resp.status().is_success() {
                let status = resp.status();
                let err_text = resp.text().await.unwrap_or_default();
                bail!("OpenAI TTS Error ({}): {}", status, err_text);
            }
            let bytes = resp.bytes().await?;
            return Self::save_audio_from_bytes(&client, &bytes, output_path).await;
        }

        // 3. 通用 DashScope / 阿里云 HTTP 模式
        let instruction = match voice {
            "longanyang" => Some("用阳光年轻男声说话"),
            "longshuo" => Some("用沉稳成熟中年男声说话"),
            "longcheng" => Some("用富有磁性稳重的男声说话"),
            "longyingjing_v3" => Some("用知性温和女声说话"),
            "longxiaochun_v2" => Some("用温柔柔和女声说话"),
            "longanhuan_v3" => Some("用欢快活泼亲切女声说话"),
            _ if voice.contains("男") || voice.contains("yang") || voice.contains("male") => Some("用阳光年轻男声说话"),
            _ if voice.contains("女") || voice.contains("female") || voice.contains("huan") || voice.contains("chun") => Some("用知性温和女声说话"),
            _ => None,
        };

        let mut input_obj = serde_json::json!({
            "text": text,
            "voice": voice,
            "format": "mp3",
            "sample_rate": 22050,
            "rate": speed
        });
        if let Some(inst) = instruction {
            input_obj["instruction"] = serde_json::Value::String(inst.to_string());
        }

        let body = serde_json::json!({
            "model": model_name,
            "input": input_obj,
            "parameters": {
                "voice": voice,
                "rate": speed,
                "format": "mp3",
                "sample_rate": 22050
            }
        });

        let mut req = client.post(&url).json(&body);
        if !cfg.api_key.trim().is_empty() {
            req = req.header("Authorization", format!("Bearer {}", cfg.api_key.trim()));
        }

        let resp = req.send().await.context("Failed to request DashScope TTS")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            bail!("TTS Request Error ({}): {}", status, err_text);
        }

        let bytes = resp.bytes().await?;
        Self::save_audio_from_bytes(&client, &bytes, output_path).await
    }

    /// 使用 ffprobe 读取音频时长（秒）
    pub fn probe_duration(audio_path: &Path) -> f32 {
        let ffprobe_bin = PathManager::ffprobe_bin();
        let output = Self::new_command(&ffprobe_bin)
            .args([
                "-v", "error",
                "-show_entries", "format=duration",
                "-of", "default=noprint_wrappers=1:nokey=1",
                audio_path.to_str().unwrap(),
            ])
            .output();

        if let Ok(out) = output {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if let Ok(val) = s.parse::<f32>() {
                return val;
            }
        }
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slide7_segmentation() {
        let text = "{role:老师}\n老师说话\n[break=500ms]\n{role:同学}\n同学说话\n";
        let role_configs = vec![
            RoleVoiceConfig {
                role_id: 1,
                role_name: "老师".to_string(),
                voice: "longyingjing_v3".to_string(),
                speed: 1.0,
            },
            RoleVoiceConfig {
                role_id: 2,
                role_name: "同学".to_string(),
                voice: "longanyang".to_string(),
                speed: 1.0,
            },
            RoleVoiceConfig {
                role_id: 3,
                role_name: "".to_string(),
                voice: "longxiaochun_v2".to_string(),
                speed: 1.0,
            },
        ];

        let segments = TtsService::segment_script(text, &role_configs);
        assert_eq!(segments.len(), 3);
        match &segments[0] {
            ScriptSegment::Speech { role_name, voice_id, text, .. } => {
                assert_eq!(role_name, "老师");
                assert_eq!(voice_id, "longyingjing_v3");
                assert_eq!(text, "老师说话");
            }
            _ => panic!("Expected speech"),
        }
        match &segments[1] {
            ScriptSegment::Gap { ms } => assert_eq!(*ms, 500),
            _ => panic!("Expected gap"),
        }
        match &segments[2] {
            ScriptSegment::Speech { role_name, voice_id, text, .. } => {
                assert_eq!(role_name, "同学");
                assert_eq!(voice_id, "longanyang");
                assert_eq!(text, "同学说话");
            }
            _ => panic!("Expected speech"),
        }
    }
}
