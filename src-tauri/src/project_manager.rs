use std::fs;
use std::path::Path;
use anyhow::{Context, Result};
use chrono::Local;
use crate::config::PathManager;
use crate::models::{ProjectState, RoleVoiceConfig};
use crate::ppt_parser::{PptParser, WindowsOfficeComParser};

pub struct ProjectManager;

impl ProjectManager {
    /// 创建新项目并解析 PPT
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

        // 解析 PPT 导出 PNG
        let parser = WindowsOfficeComParser;
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
        for slide in &mut state.slides {
            if slide.image_data.is_none() {
                slide.image_data = crate::ppt_parser::load_image_base64(Path::new(&slide.image_path));
            }
        }
        Ok(state)
    }
}
