use serde::{Deserialize, Serialize};

/// 全局应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub tts: TtsConfig,
    pub video: VideoConfig,
    pub role_configs: Vec<RoleVoiceConfig>,
    pub output_dir: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            tts: TtsConfig::default(),
            video: VideoConfig::default(),
            role_configs: vec![
                RoleVoiceConfig {
                    role_id: 1,
                    role_name: "".to_string(),
                    voice: "longyingjing_v3".to_string(),
                    speed: 1.0,
                },
                RoleVoiceConfig {
                    role_id: 2,
                    role_name: "".to_string(),
                    voice: "longanyang".to_string(),
                    speed: 1.0,
                },
                RoleVoiceConfig {
                    role_id: 3,
                    role_name: "".to_string(),
                    voice: "longxiaochun_v2".to_string(),
                    speed: 1.0,
                },
            ],
            output_dir: "output".to_string(),
        }
    }
}

/// 语音合成 TTS 配置 (支持云端与本地接口)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsConfig {
    /// 接口调用地址，默认为 DashScope，亦可配置本地例如 http://127.0.0.1:8000/v1
    pub api_url: String,
    /// API 密钥（本地接口可留空）
    pub api_key: String,
    /// 模型名称
    pub model: String,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            api_url: "https://dashscope.aliyuncs.com/api/v1/services/aigc/text-generation/generation".to_string(),
            api_key: "".to_string(),
            model: "cosyvoice-v3-flash".to_string(),
        }
    }
}

/// 视频生成配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoConfig {
    pub fps: u32,
    pub resolution: (u32, u32),
    pub default_slide_duration: f32,
    pub slide_pause: f32,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            fps: 5,
            resolution: (1920, 1080),
            default_slide_duration: 3.0,
            slide_pause: 0.5,
        }
    }
}

/// 角色音色配置（固定角色1/2/3）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleVoiceConfig {
    /// 角色槽位编号：1, 2, 3
    pub role_id: u8,
    /// 脚本中对应的角色名称，例如 "李磊"、"韩梅梅"
    pub role_name: String,
    /// 音色 ID
    pub voice: String,
    /// 该角色专属语速 (0.5 ~ 2.0)
    pub speed: f32,
}

/// 单页幻灯片信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideInfo {
    pub index: usize,
    pub image_path: String,
    pub image_data: Option<String>,
    pub script_text: String,
    pub audio_path: Option<String>,
    pub audio_duration: f32,
    pub audio_versions: Vec<String>,
}

/// 项目状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectState {
    pub ppt_path: String,
    pub ppt_stem: String,
    pub project_dir: String,
    pub slide_count: usize,
    pub slides: Vec<SlideInfo>,
    pub role_configs: Vec<RoleVoiceConfig>,
    pub created_at: String,
    pub updated_at: String,
}
