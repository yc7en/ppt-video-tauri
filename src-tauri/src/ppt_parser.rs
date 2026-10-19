use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{bail, Context, Result};
use crate::models::SlideInfo;

#[async_trait::async_trait]
pub trait PptParser: Send + Sync {
    /// 导出幻灯片为 PNG 文件列表
    async fn export_slides(&self, ppt_path: &Path, output_dir: &Path) -> Result<Vec<SlideInfo>>;
}

pub struct WindowsOfficeComParser;

#[async_trait::async_trait]
impl PptParser for WindowsOfficeComParser {
    async fn export_slides(&self, ppt_path: &Path, output_dir: &Path) -> Result<Vec<SlideInfo>> {
        fs::create_dir_all(output_dir)?;

        let ppt_abs = ppt_path.canonicalize().context("Invalid PPT path")?;
        let out_abs = output_dir.canonicalize().unwrap_or_else(|_| output_dir.to_path_buf());

        // 移除 Windows canonicalize 产生的 \\?\ 前缀，PowerPoint COM 接口不支持此格式
        let ppt_clean = ppt_abs.to_string_lossy().trim_start_matches(r"\\?\").to_string();
        let out_clean = out_abs.to_string_lossy().trim_start_matches(r"\\?\").to_string();

        // 使用 Windows COM (PowerPoint.Application) 执行导出
        // 兼容中文版和英文版 Office
        let script = format!(
            r#"
$ErrorActionPreference = 'Stop'
$pptApp = New-Object -ComObject PowerPoint.Application
try {{
    $pres = $pptApp.Presentations.Open('{ppt}', -1, 0, 0)
    try {{
        $count = $pres.Slides.Count
        $pres.Export('{out}', 'PNG')
        Write-Output "COUNT:$count"
    }} finally {{
        $pres.Close()
    }}
}} finally {{
    $pptApp.Quit()
    [System.GC]::Collect()
}}
"#,
            ppt = ppt_clean.replace('\'', "''"),
            out = out_clean.replace('\'', "''"),
        );

        let mut cmd = Command::new("powershell");
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let output = cmd
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
            .context("Failed to invoke PowerShell for PowerPoint COM export")?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            bail!("PowerPoint 解析失败: {err_msg}");
        }

        // 收集并排序导出的 PNG 图片
        let mut png_files: Vec<PathBuf> = Vec::new();
        for entry in fs::read_dir(output_dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext.to_string_lossy().to_lowercase() == "png" {
                    png_files.push(path);
                }
            }
        }

        if png_files.is_empty() {
            bail!("PowerPoint 导出未产生任何 PNG 图片");
        }

        // 按文件名自然排序 (Slide1, Slide2...)
        png_files.sort_by_key(|p| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| {
                    let num_part: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
                    num_part.parse::<usize>().unwrap_or(0)
                })
                .unwrap_or(0)
        });

        let mut slides = Vec::new();
        for (idx, path) in png_files.into_iter().enumerate() {
            let b64_data = load_image_base64(&path);
            slides.push(SlideInfo {
                index: idx + 1,
                image_path: path.to_string_lossy().to_string(),
                image_data: b64_data,
                script_text: "".to_string(),
                subtitle_text: None,
                audio_path: None,
                audio_duration: 0.0,
                audio_versions: Vec::new(),
            });
        }

        Ok(slides)
    }
}

pub fn load_image_base64(path: &Path) -> Option<String> {
    use base64::Engine;
    if let Ok(bytes) = fs::read(path) {
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        Some(format!("data:image/png;base64,{}", b64))
    } else {
        None
    }
}
