use serde::{Deserialize, Serialize};

/// 自定义音色项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceItem {
    /// 音色名称，如 "知性女声" 或 "龙安欢"
    pub name: String,
    /// 音色标识，如 "longyingjing_v3" 或 "longanhuan_v3"
    pub voice: String,
    /// 是否为默认兜底音色（唯一）
    #[serde(default)]
    pub is_default: bool,
}

pub fn default_voices() -> Vec<VoiceItem> {
    vec![
        VoiceItem {
            name: "知性女声 (龙莹静)".to_string(),
            voice: "longyingjing_v3".to_string(),
            is_default: true,
        },
        VoiceItem {
            name: "阳光男声 (龙安阳)".to_string(),
            voice: "longanyang".to_string(),
            is_default: false,
        },
        VoiceItem {
            name: "温柔女声 (龙小春)".to_string(),
            voice: "longxiaochun_v2".to_string(),
            is_default: false,
        },
    ]
}

/// 全局应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub tts: TtsConfig,
    pub video: VideoConfig,
    pub role_configs: Vec<RoleVoiceConfig>,
    pub output_dir: String,
    /// 字幕配置（旧配置文件缺失时使用默认值）
    #[serde(default)]
    pub subtitle: SubtitleConfig,
    /// 自定义音色库配置
    #[serde(default = "default_voices")]
    pub custom_voices: Vec<VoiceItem>,
}

impl AppConfig {
    pub fn get_default_voice(&self) -> String {
        self.custom_voices
            .iter()
            .find(|v| v.is_default)
            .map(|v| v.voice.clone())
            .unwrap_or_else(|| "longyingjing_v3".to_string())
    }
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
            subtitle: SubtitleConfig::default(),
            custom_voices: default_voices(),
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
    /// 精准字幕模式下，同一角色相邻句子之间的插入空隙（毫秒）
    #[serde(default = "default_sentence_gap_ms")]
    pub sentence_gap_ms: u32,
}

/// 同一角色相邻句子之间的默认插入空隙（毫秒）
pub fn default_sentence_gap_ms() -> u32 {
    120
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            api_url: "https://dashscope.aliyuncs.com/api/v1/services/audio/tts/generation".to_string(),
            api_key: "".to_string(),
            model: "cosyvoice-v3-flash".to_string(),
            sentence_gap_ms: default_sentence_gap_ms(),
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
    /// 独立字幕文案（留空则复用朗读文案）
    #[serde(default)]
    pub subtitle_text: Option<String>,
    pub audio_path: Option<String>,
    pub audio_duration: f32,
    pub audio_versions: Vec<String>,
}

/// 字幕计时模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TimingMode {
    /// 自动：本地/自建端点使用 segment 精准模式
    Auto,
    /// 精准：逐句合成实测时长
    #[default]
    Segment,
    /// 估算：按字数比例
    Estimate,
}

/// 字幕配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleConfig {
    /// 总开关
    pub enabled: bool,
    /// 字体名称（系统字体，如 微软雅黑 / Microsoft YaHei）
    pub font_name: String,
    /// 字号（基于 PlayRes 高度）
    pub font_size: u32,
    /// 字体颜色 "#RRGGBB"
    pub primary_color: String,
    /// 描边颜色 "#RRGGBB"
    pub outline_color: String,
    /// 描边宽度
    pub outline: f32,
    /// 距底部边距（像素）
    pub margin_v: u32,
    /// 每行最大字数
    pub max_chars_per_line: u32,
    /// 同时显示的最大行数
    pub max_lines: u32,
    /// 计时模式
    pub timing_mode: TimingMode,
}

impl Default for SubtitleConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            font_name: "Microsoft YaHei".to_string(),
            font_size: 48,
            primary_color: "#FFFFFF".to_string(),
            outline_color: "#000000".to_string(),
            outline: 2.0,
            margin_v: 60,
            max_chars_per_line: 20,
            max_lines: 2,
            timing_mode: TimingMode::Segment,
        }
    }
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
