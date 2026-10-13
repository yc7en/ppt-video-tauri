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

pub struct TtsService;

impl TtsService {
    /// 解析文案中的角色标签并完成 [break=100ms] 的转义
    pub fn parse_script(
        text: &str,
        role_configs: &[RoleVoiceConfig],
    ) -> (Vec<TtsChunk>, Option<String>) {
        if text.trim().is_empty() {
            return (Vec::new(), None);
        }

        // 1. 清洗可能导致 CosyVoice 引擎卡顿或生成 10 秒死寂空白的非标/外来 speak 标签（如 <speak effect="eq"...> 与 </speak>）
        let speak_re = Regex::new(r"(?i)</?speak[^>]*>").unwrap();
        let cleaned = speak_re.replace_all(text, "").to_string();

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
        
        let fallback_role = role_configs
            .iter()
            .find(|r| r.role_id == 1)
            .cloned()
            .unwrap_or(RoleVoiceConfig {
                role_id: 1,
                role_name: "".to_string(),
                voice: "longyingjing_v3".to_string(),
                speed: 1.0,
            });

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
            let p_str = seg.canonicalize()?.to_string_lossy().replace('\\', "/");
            list_content.push_str(&format!("file '{}'\n", p_str));
            if i < segment_paths.len() - 1 {
                let s_str = silence_path.canonicalize()?.to_string_lossy().replace('\\', "/");
                list_content.push_str(&format!("file '{}'\n", s_str));
            }
        }
        fs::write(&concat_list_path, list_content)?;

        // FFmpeg concat
        let status = Self::new_command(&ffmpeg_bin)
            .args([
                "-f", "concat",
                "-safe", "0",
                "-i", concat_list_path.to_str().unwrap(),
                "-c", "copy",
                "-y",
                output_path.to_str().unwrap(),
            ])
            .status()
            .context("Failed to concat audio segments via ffmpeg")?;

        let _ = fs::remove_dir_all(&temp_dir);

        if !status.success() {
            bail!("多角色音频拼接失败");
        }

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
            let body = serde_json::json!({
                "model": model_name,
                "input": {
                    "text": text,
                    "voice": voice,
                    "format": "mp3",
                    "sample_rate": 22050,
                    "rate": speed
                }
            });

            let mut req = client.post(&url).json(&body);
            if !cfg.api_key.trim().is_empty() {
                req = req.header("Authorization", format!("Bearer {}", cfg.api_key.trim()));
            }

            let resp = req.send().await.context("Failed to request SpeechSynthesizer TTS")?;
            if !resp.status().is_success() {
                let status = resp.status();
                let err_text = resp.text().await.unwrap_or_default();
                bail!("Local TTS Error ({}): {}", status, err_text);
            }
            let bytes = resp.bytes().await?;
            fs::write(output_path, bytes)?;
            return Ok(output_path.to_path_buf());
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
            fs::write(output_path, bytes)?;
            return Ok(output_path.to_path_buf());
        }

        // 3. 通用 DashScope / 阿里云 HTTP 模式
        let body = serde_json::json!({
            "model": model_name,
            "input": {
                "text": text,
                "voice": voice,
                "format": "mp3",
                "sample_rate": 22050,
                "rate": speed
            },
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
        // 若返回包装了 audio URL 的 JSON 结果，则尝试下载音频
        if bytes.starts_with(b"{") {
            if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                if let Some(audio_url) = val.pointer("/output/audio").and_then(|v| v.as_str()) {
                    let audio_resp = client.get(audio_url).send().await.context("Failed to download audio from url")?;
                    let audio_bytes = audio_resp.bytes().await?;
                    fs::write(output_path, audio_bytes)?;
                    return Ok(output_path.to_path_buf());
                }
            }
        }
        fs::write(output_path, bytes)?;
        Ok(output_path.to_path_buf())
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
