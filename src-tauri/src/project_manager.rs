use std::fs;
use std::path::Path;
use anyhow::{Context, Result};
use chrono::Local;
use crate::config::PathManager;
use crate::models::{ProjectState, RoleVoiceConfig};
use crate::ppt_parser::{PptParser, UniversalPresentationParser};

pub struct ProjectManager;

impl ProjectManager {
    /// 创建新项目并解析演示文稿（PPT/PPTX/PDF）
    pub async fn create_project(ppt_path: &Path, default_roles: Vec<RoleVoiceConfig>) -> Result<ProjectState> {
        let ppt_stem = ppt_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("project")
            .to_string();

        let projects_root = PathManager::projects_dir();
        let proj_dir = projects_root.join(&ppt_stem);
        let slides_dir = proj_dir.join("slides");
        let audio_dir = proj_dir.join("audio");
        let output_dir = proj_dir.join("output");

        fs::create_dir_all(&slides_dir)?;
        fs::create_dir_all(&audio_dir)?;
        fs::create_dir_all(&output_dir)?;

        // 解析演示文稿导出 PNG（支持 PPT/PPTX 及原生 PDF）
        let parser = UniversalPresentationParser;
        let slides = parser.export_slides(ppt_path, &slides_dir).await?;

        let now_str = Local::now().to_rfc3339();
        let project = ProjectState {
            ppt_path: ppt_path.to_string_lossy().to_string(),
            ppt_stem,
            project_dir: proj_dir.to_string_lossy().to_string(),
            slide_count: slides.len(),
            slides,
            role_configs: default_roles,
            created_at: now_str.clone(),
            updated_at: now_str,
        };

        Self::save_project(&project)?;
        Ok(project)
    }

    /// 保存项目到 project.json
    pub fn save_project(project: &ProjectState) -> Result<()> {
        let proj_dir = Path::new(&project.project_dir);
        let file_path = proj_dir.join("project.json");
        let json_str = serde_json::to_string_pretty(project)
            .context("Failed to serialize project state")?;
        fs::write(file_path, json_str)?;
        Ok(())
    }

    /// 从 project.json 加载项目
    pub fn load_project(project_json: &Path) -> Result<ProjectState> {
        let content = fs::read_to_string(project_json)
            .context("Failed to read project.json")?;
        let mut state: ProjectState = serde_json::from_str(&content)
            .context("Failed to parse project.json")?;
        let proj_dir = Path::new(&state.project_dir);
        let audio_dir = proj_dir.join("audio");

        for slide in &mut state.slides {
            if slide.image_data.is_none() {
                slide.image_data = crate::ppt_parser::load_image_base64(Path::new(&slide.image_path));
            }
            // 扫描并同步该页的所有已有音频版本
            let slide_audio_dir = audio_dir.join(format!("slide_{:04}", slide.index));
            if slide_audio_dir.exists() {
                let mut disk_versions = Vec::new();
                if let Ok(entries) = fs::read_dir(&slide_audio_dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.extension().map(|e| e.to_string_lossy().to_lowercase()) == Some("mp3".to_string()) {
                            disk_versions.push(p.to_string_lossy().to_string());
                        }
                    }
                }
                if !disk_versions.is_empty() {
                    disk_versions.sort(); // 按文件名时间戳升序排序
                    slide.audio_versions = disk_versions;
                    // 如果当前未设置 audio_path 或当前路径不存在，默认使用最新版本（最后一个）
                    let need_default = slide.audio_path.as_ref().map(|p| !Path::new(p).exists()).unwrap_or(true);
                    if need_default {
                        if let Some(latest) = slide.audio_versions.last() {
                            slide.audio_path = Some(latest.clone());
                            slide.audio_duration = crate::tts_service::TtsService::probe_duration(Path::new(latest));
                        }
                    }
                }
            }
        }
        Ok(state)
    }
}
