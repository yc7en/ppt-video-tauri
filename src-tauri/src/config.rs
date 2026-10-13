use std::fs;
use std::path::PathBuf;
use crate::models::AppConfig;
use anyhow::{Context, Result};

pub struct PathManager;

impl PathManager {
    /// 获取应用根目录（优先便携模式，回退 AppData）
    pub fn get_app_root() -> PathBuf {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                // 检测是否存在便携模式标记
                let portable_config = exe_dir.join("config.json");
                let portable_tools = exe_dir.join("tools");
                if portable_config.exists() || portable_tools.exists() {
                    return exe_dir.to_path_buf();
                }
            }
        }

        // 回退到系统标准应用目录
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("PPTVideoGenerator")
    }

    /// 配置文件路径
    pub fn config_file() -> PathBuf {
        Self::get_app_root().join("config.json")
    }

    /// 外置工具目录 (tools/)
    pub fn tools_dir() -> PathBuf {
        Self::get_app_root().join("tools")
    }

    /// 获取 ffmpeg 可执行程序路径
    pub fn ffmpeg_bin() -> PathBuf {
        let local = Self::tools_dir().join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" });
        if local.exists() {
            return local;
        }
        // 尝试从 PATH 中寻找
        PathBuf::from("ffmpeg")
    }

    /// 获取 ffprobe 可执行程序路径
    pub fn ffprobe_bin() -> PathBuf {
        let local = Self::tools_dir().join(if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" });
        if local.exists() {
            return local;
        }
        PathBuf::from("ffprobe")
    }

    /// 项目工程根目录 (projects/)
    pub fn projects_dir() -> PathBuf {
        let p = Self::get_app_root().join("projects");
        let _ = fs::create_dir_all(&p);
        p
    }

    /// 视频输出目录 (output/)
    pub fn output_dir() -> PathBuf {
        let p = Self::get_app_root().join("output");
        let _ = fs::create_dir_all(&p);
        p
    }
}

/// 读取应用配置
pub fn load_config() -> AppConfig {
    let path = PathManager::config_file();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<AppConfig>(&content) {
                return cfg;
            }
        }
    }
    // 默认配置并持久化
    let default_cfg = AppConfig::default();
    let _ = save_config(&default_cfg);
    default_cfg
}

/// 保存应用配置
pub fn save_config(cfg: &AppConfig) -> Result<()> {
    let path = PathManager::config_file();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json_str = serde_json::to_string_pretty(cfg)
        .context("Failed to serialize AppConfig to JSON")?;
    fs::write(path, json_str)?;
    Ok(())
}
