use std::path::PathBuf;
use tauri::command;
use chrono::Local;

use crate::config::{load_config, save_config};
use crate::models::{AppConfig, ProjectState, RoleVoiceConfig, SlideInfo, SubtitleConfig, VideoConfig};
use crate::project_manager::ProjectManager;
use crate::tts_service::TtsService;
use crate::video_composer::VideoComposer;

#[command]
pub fn get_app_config() -> Result<AppConfig, String> {
    Ok(load_config())
}

#[command]
pub fn update_app_config(cfg: AppConfig) -> Result<(), String> {
    save_config(&cfg).map_err(|e| e.to_string())
}

#[command]
pub async fn create_project_cmd(ppt_path: String) -> Result<ProjectState, String> {
    let cfg = load_config();
    let path = PathBuf::from(&ppt_path);
    ProjectManager::create_project(&path, cfg.role_configs)
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub fn save_project_cmd(project: ProjectState) -> Result<(), String> {
    ProjectManager::save_project(&project).map_err(|e| e.to_string())
}

#[command]
pub fn load_project_cmd(project_json: String) -> Result<ProjectState, String> {
    let p = PathBuf::from(project_json);
    ProjectManager::load_project(&p).map_err(|e| e.to_string())
}

#[command]
pub fn detect_first_role_cmd(text: String) -> Result<Option<String>, String> {
    let cfg = load_config();
    let (_, first_role) = TtsService::parse_script(&text, &cfg.role_configs);
    Ok(first_role)
}

#[command]
pub fn detect_roles_cmd(text: String) -> Result<Vec<String>, String> {
    let re = regex::Regex::new(r"\{role:(.*?)\}").unwrap();
    let mut roles = Vec::new();
    for cap in re.captures_iter(&text) {
        let r = cap.get(1).unwrap().as_str().trim().to_string();
        if !r.is_empty() && !roles.contains(&r) {
            roles.push(r);
        }
    }
    Ok(roles)
}

#[command]
pub async fn synthesize_slide_cmd(
    slide_index: usize,
    text: String,
    project_dir: String,
    role_configs: Vec<RoleVoiceConfig>,
) -> Result<SlideInfo, String> {
    let cfg = load_config();
    let proj_dir = PathBuf::from(project_dir);
    let slide_audio_dir = proj_dir.join("audio").join(format!("slide_{:04}", slide_index));
    let _ = std::fs::create_dir_all(&slide_audio_dir);

    let ts = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let target_audio = slide_audio_dir.join(format!("{}.mp3", ts));

    let saved = TtsService::synthesize_slide_audio_with_timing(&text, &target_audio, &cfg.tts, &role_configs)
        .await
        .map_err(|e| e.to_string())?
        .0;

    let dur = TtsService::probe_duration(&saved);
    let abs_str = saved.to_string_lossy().to_string();

    // 扫描该幻灯片目录下的全部历史配音版本
    let mut versions = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&slide_audio_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().map(|e| e.to_string_lossy().to_lowercase()) == Some("mp3".to_string()) {
                versions.push(p.to_string_lossy().to_string());
            }
        }
    }
    versions.sort(); // 默认按时间戳升序排序，最后一个即为最新

    Ok(SlideInfo {
        index: slide_index,
        image_path: "".to_string(), // 前端保留原有图片
        image_data: None,
        script_text: text,
        subtitle_text: None,
        audio_path: Some(abs_str),
        audio_duration: dur,
        audio_versions: versions,
    })
}

#[command]
pub fn probe_audio_duration_cmd(path: String) -> Result<f32, String> {
    let p = PathBuf::from(path);
    Ok(TtsService::probe_duration(&p))
}

/// 探测某配音版本的计时模式：segment（精准）| estimate（估算）
#[command]
pub fn probe_timing_mode_cmd(audio_path: String) -> Result<String, String> {
    let p = PathBuf::from(audio_path);
    match crate::subtitle_service::load_timing(&p) {
        Some(t) => Ok(t.mode),
        None => Ok("estimate".to_string()),
    }
}

#[command]
pub async fn compose_video_cmd(
    project: ProjectState,
    video_config: VideoConfig,
    subtitle_config: SubtitleConfig,
) -> Result<String, String> {
    let proj_dir = PathBuf::from(&project.project_dir);
    let out_dir = proj_dir.join("output");
    let _ = std::fs::create_dir_all(&out_dir);

    let ts = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let file_name = format!("{}_{}.mp4", project.ppt_stem, ts);
    let out_mp4 = out_dir.join(&file_name);

    let res = VideoComposer::compose(
        &project.slides,
        &out_mp4,
        &video_config,
        &subtitle_config,
        |curr, total, msg| {
            println!("[VideoCompose] ({}/{}) {}", curr, total, msg);
        },
    ).map_err(|e| e.to_string())?;

    // 复制一份到便携根目录 output/，双重保障
    let global_out = crate::config::PathManager::output_dir();
    let global_file = global_out.join(&file_name);
    let _ = std::fs::copy(&res, &global_file);

    Ok(res.to_string_lossy().to_string())
}

#[command]
pub fn open_path_cmd(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let clean = path.replace('/', "\\");
        let p = PathBuf::from(&clean);

        let mut cmd = std::process::Command::new("explorer");
        if p.is_file() {
            cmd.arg(format!("/select,{}", clean));
        } else {
            let _ = std::fs::create_dir_all(&p);
            cmd.arg(&clean);
        }
        cmd.spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(serde::Serialize)]
pub struct ProjectSummary {
    pub name: String,
    pub path: String,
    pub slide_count: usize,
    pub updated_at: String,
}

#[command]
pub fn list_recent_projects_cmd() -> Result<Vec<ProjectSummary>, String> {
    let projects_dir = crate::config::PathManager::projects_dir();
    let mut list = Vec::new();
    if let Ok(entries) = std::fs::read_dir(projects_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let p_json = p.join("project.json");
                if p_json.exists() {
                    if let Ok(state) = ProjectManager::load_project(&p_json) {
                        list.push(ProjectSummary {
                            name: state.ppt_stem,
                            path: p_json.to_string_lossy().to_string(),
                            slide_count: state.slide_count,
                            updated_at: state.updated_at,
                        });
                    }
                }
            }
        }
    }
    list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(list)
}

#[command]
pub fn select_ppt_file_cmd() -> Result<Option<String>, String> {
    let file = rfd::FileDialog::new()
        .add_filter("演示文档 (*.pptx; *.ppt; *.pdf)", &["pptx", "ppt", "pdf"])
        .add_filter("PowerPoint 演示文稿 (*.pptx; *.ppt)", &["pptx", "ppt"])
        .add_filter("PDF 演示文档 (*.pdf)", &["pdf"])
        .set_title("选择演示文档 (PPTX / PPT / PDF)")
        .pick_file();
    Ok(file.map(|p| p.to_string_lossy().to_string()))
}

#[command]
pub fn select_project_file_cmd() -> Result<Option<String>, String> {
    let file = rfd::FileDialog::new()
        .add_filter("项目文件 (project.json)", &["json"])
        .set_title("选择项目文件 (project.json)")
        .pick_file();
    Ok(file.map(|p| p.to_string_lossy().to_string()))
}
