use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{bail, Context, Result};
use crate::config::PathManager;
use crate::models::{SlideInfo, VideoConfig};

pub struct VideoComposer;

impl VideoComposer {
    fn new_command(bin: &Path) -> Command {
        let mut cmd = Command::new(bin);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        cmd
    }

    /// 合成完整 PPT 视频
    pub fn compose<F>(
        slides: &[SlideInfo],
        output_path: &Path,
        cfg: &VideoConfig,
        on_progress: F,
    ) -> Result<PathBuf>
    where
        F: Fn(usize, usize, &str),
    {
        if slides.is_empty() {
            bail!("幻灯片列表不能为空");
        }

        let ffmpeg_bin = PathManager::ffmpeg_bin();
        let total = slides.len();

        let temp_dir = output_path
            .parent()
            .unwrap_or(Path::new("."))
            .join(format!("vid_tmp_{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir_all(&temp_dir)?;

        let mut segment_paths = Vec::new();
        let (w, h) = cfg.resolution;

        // 1. 逐页生成视频切片
        for (i, slide) in slides.iter().enumerate() {
            let idx = i + 1;
            let msg = format!("正在生成第 {}/{} 页视频切片...", idx, total);
            on_progress(idx, total, &msg);

            let seg_path = temp_dir.join(format!("seg_{:04}.mp4", idx));
            let has_audio = slide.audio_path.as_ref().map(|p| Path::new(p).exists()).unwrap_or(false);

            let (audio_src, is_silence) = if has_audio {
                (PathBuf::from(slide.audio_path.as_ref().unwrap()), false)
            } else {
                let sil_path = temp_dir.join(format!("sil_{:04}.mp3", idx));
                Self::generate_silence(&sil_path, cfg.default_slide_duration, &ffmpeg_bin)?;
                (sil_path, true)
            };

            let base_dur = if is_silence {
                cfg.default_slide_duration
            } else if slide.audio_duration > 0.0 {
                slide.audio_duration
            } else {
                cfg.default_slide_duration
            };

            let duration = base_dur + cfg.slide_pause;

            // 调用 FFmpeg 合成单页切片
            let mut cmd = Self::new_command(&ffmpeg_bin);
            cmd.args([
                "-loop", "1",
                "-framerate", &cfg.fps.to_string(),
                "-i", &slide.image_path,
                "-i", audio_src.to_str().unwrap(),
            ]);

            if !is_silence && cfg.slide_pause > 0.0 {
                cmd.args([
                    "-filter_complex", "[1:a]apad[a]",
                    "-map", "0:v",
                    "-map", "[a]",
                ]);
            } else {
                cmd.args(["-map", "0:v", "-map", "1:a"]);
            }

            let vf = format!(
                "scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:black,format=yuv420p"
            );

            cmd.args([
                "-t", &duration.to_string(),
                "-c:v", "libx264",
                "-tune", "stillimage",
                "-c:a", "aac",
                "-b:a", "128k",
                "-ar", "22050",
                "-ac", "1",
                "-vf", &vf,
                "-movflags", "+faststart",
                "-y",
                seg_path.to_str().unwrap(),
            ]);

            let status = cmd.status().context("Failed to run ffmpeg for slide segment")?;
            if !status.success() {
                bail!("生成第 {} 页视频片段失败", idx);
            }

            segment_paths.push(seg_path);
        }

        // 2. 拼接全部切片为最终 MP4
        on_progress(total, total, "正在拼接全部视频切片...");
        let concat_list = temp_dir.join("concat_list.txt");
        let mut list_content = String::new();
        for seg in &segment_paths {
            let p_str = seg.canonicalize()?.to_string_lossy().replace('\\', "/");
            list_content.push_str(&format!("file '{}'\n", p_str));
        }
        fs::write(&concat_list, list_content)?;

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let concat_status = Self::new_command(&ffmpeg_bin)
            .args([
                "-f", "concat",
                "-safe", "0",
                "-i", concat_list.to_str().unwrap(),
                "-c:v", "copy",
                "-c:a", "aac",
                "-b:a", "128k",
                "-ar", "22050",
                "-ac", "1",
                "-movflags", "+faststart",
                "-y",
                output_path.to_str().unwrap(),
            ])
            .status()
            .context("Failed to concat final video via ffmpeg")?;

        let _ = fs::remove_dir_all(&temp_dir);

        if !concat_status.success() {
            bail!("合并最终 MP4 视频失败");
        }

        on_progress(total, total, "视频合成完成");
        Ok(output_path.to_path_buf())
    }

    /// 生成纯静音频
    fn generate_silence(output_path: &Path, duration: f32, ffmpeg_bin: &Path) -> Result<()> {
        let status = Self::new_command(ffmpeg_bin)
            .args([
                "-f", "lavfi",
                "-i", "anullsrc=r=22050:cl=mono",
                "-t", &duration.to_string(),
                "-q:a", "9",
                "-acodec", "libmp3lame",
                "-y",
                output_path.to_str().unwrap(),
            ])
            .status()
            .context("Failed to generate silence via ffmpeg")?;

        if !status.success() {
            bail!("生成静音音频失败");
        }
        Ok(())
    }
}
