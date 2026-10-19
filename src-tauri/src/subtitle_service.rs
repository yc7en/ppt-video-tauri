use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::models::SubtitleConfig;

/// 计时单元：一段文本及其在整页音频中的起止时间（毫秒）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingUnit {
    pub text: String,
    pub begin_ms: u64,
    pub end_ms: u64,
}

/// 计时旁车文件（与音频版本同名，扩展名为 timing.json）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingFile {
    /// "segment" 精准模式 | "estimate" 估算模式
    pub mode: String,
    pub units: Vec<TimingUnit>,
}

/// 单条字幕（可含 \n 多行）与其时间
#[derive(Debug, Clone)]
pub struct SubtitleCue {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

/// 规整空白：去掉连续空白，保留单个空格（中文字幕通常无空格）
pub fn normalize_whitespace(text: &str) -> String {
    let ws_re = Regex::new(r"[\s\u{3000}]+").unwrap();
    ws_re.replace_all(text, " ").trim().to_string()
}

/// 清洗显示/对齐用文本：去除角色标签、停顿标记与 speak 标签
pub fn clean_text(text: &str) -> String {
    let speak_re = Regex::new(r"(?i)</?speak[^>]*>").unwrap();
    let cleaned = speak_re.replace_all(text, "");
    let role_re = Regex::new(r"\{role:.*?\}").unwrap();
    let cleaned = role_re.replace_all(&cleaned, "");
    let break_re = Regex::new(r"\[break=.*?\]").unwrap();
    let cleaned = break_re.replace_all(&cleaned, "");
    normalize_whitespace(&cleaned)
}

/// 音频文件对应的计时旁车路径：xxx.mp3 -> xxx.timing.json
pub fn timing_path_for_audio(audio_path: &Path) -> PathBuf {
    audio_path.with_extension("timing.json")
}

/// 读取计时旁车（不存在或损坏时返回 None）
pub fn load_timing(audio_path: &Path) -> Option<TimingFile> {
    let path = timing_path_for_audio(audio_path);
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str::<TimingFile>(&content).ok()
}

fn is_sentence_end(c: char) -> bool {
    matches!(c, '。' | '！' | '？' | '!' | '?' | '；' | ';' | '\n')
}

fn is_break_punct(c: char) -> bool {
    matches!(
        c,
        '，' | '、' | ',' | '：' | ':' | '。' | '！' | '？' | '；' | ' ' | '!'
            | '?' | ';' | '）' | ')'
    )
}

/// 将文本按句末标点切分为句子
fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in text.chars() {
        cur.push(c);
        if is_sentence_end(c) {
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

/// 将一句文本按最大字数硬折行（优先在标点处断开）
fn hard_wrap(sentence: &str, width: usize) -> Vec<String> {
    let chars: Vec<char> = sentence.chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let mut end = (i + width).min(chars.len());
        if end < chars.len() {
            let low = i + width / 2;
            if let Some(p) = (low..end).rev().find(|&k| is_break_punct(chars[k])) {
                end = p + 1;
            }
        }
        let line: String = chars[i..end].iter().collect();
        let trimmed = line.trim().to_string();
        if !trimmed.is_empty() {
            lines.push(trimmed);
        }
        i = end;
    }
    lines
}

/// 生成字幕文本序列（每条最多 max_lines 行，同一句不会被拆到不同条目）
fn build_cue_texts(display: &str, max_chars: usize, max_lines: usize) -> Vec<String> {
    let max_lines = max_lines.max(1);
    let sentences = split_sentences(display);
    let mut cues = Vec::new();
    for sent in sentences {
        let lines = hard_wrap(&sent, max_chars);
        if lines.is_empty() {
            continue;
        }
        for chunk in lines.chunks(max_lines) {
            cues.push(chunk.join("\n"));
        }
    }
    cues
}

/// 由计时单元构建逐字时间映射
fn build_char_map(units: &[TimingUnit]) -> Vec<(char, u64, u64)> {
    let mut map = Vec::new();
    for u in units {
        let chars: Vec<char> = u.text.chars().filter(|c| !c.is_whitespace()).collect();
        let n = chars.len();
        if n == 0 {
            continue;
        }
        let span = u.end_ms.saturating_sub(u.begin_ms);
        for (k, ch) in chars.iter().enumerate() {
            let b = u.begin_ms + span * k as u64 / n as u64;
            let e = u.begin_ms + span * (k as u64 + 1) / n as u64;
            map.push((*ch, b, e));
        }
    }
    map
}

/// 按顺序将每行字幕的字与时间映射对齐；失败返回 None（交由比例兜底）
fn align_cues(cues: &[String], map: &[(char, u64, u64)]) -> Option<Vec<(u64, u64)>> {
    let mut mi = 0usize;
    let mut out = Vec::new();
    for cue in cues {
        let chars: Vec<char> = cue.chars().filter(|c| !c.is_whitespace()).collect();
        if chars.is_empty() {
            continue;
        }
        let mut first: Option<usize> = None;
        let mut last: usize = 0;
        for ch in chars {
            while mi < map.len() && map[mi].0 != ch {
                mi += 1;
            }
            if mi >= map.len() {
                return None;
            }
            if first.is_none() {
                first = Some(mi);
            }
            last = mi;
            mi += 1;
        }
        out.push((map[first.unwrap()].1, map[last].2));
    }
    Some(out)
}

/// 按字数比例分配整页时长
fn proportional(cues: &[String], duration_ms: u64) -> Vec<SubtitleCue> {
    let counts: Vec<u64> = cues
        .iter()
        .map(|c| c.chars().filter(|x| !x.is_whitespace()).count().max(1) as u64)
        .collect();
    let total: u64 = counts.iter().sum::<u64>().max(1);
    let mut acc = 0u64;
    let mut out = Vec::new();
    for (i, cue) in cues.iter().enumerate() {
        let start = duration_ms * acc / total;
        acc += counts[i];
        let end = if i == cues.len() - 1 {
            duration_ms
        } else {
            duration_ms * acc / total
        };
        out.push(SubtitleCue {
            start_ms: start,
            end_ms: end,
            text: cue.clone(),
        });
    }
    out
}

/// 构建字幕条目
pub fn build_cues(
    display_text: &str,
    timing: Option<&TimingFile>,
    duration_ms: u64,
    cfg: &SubtitleConfig,
) -> Vec<SubtitleCue> {
    let cues = build_cue_texts(
        display_text,
        cfg.max_chars_per_line as usize,
        cfg.max_lines as usize,
    );
    if cues.is_empty() {
        return Vec::new();
    }

    if let Some(t) = timing {
        if t.mode == "segment" && !t.units.is_empty() {
            let map = build_char_map(&t.units);
            if !map.is_empty() {
                if let Some(times) = align_cues(&cues, &map) {
                    if times.len() == cues.len() {
                        return cues
                            .into_iter()
                            .zip(times)
                            .map(|(text, (s, e))| SubtitleCue {
                                start_ms: s,
                                end_ms: e.max(s + 1),
                                text,
                            })
                            .collect();
                    }
                }
            }
        }
    }

    proportional(&cues, duration_ms)
}

fn ass_color(hex: &str, alpha: &str) -> String {
    let h = hex.trim().trim_start_matches('#');
    let (r, g, b) = if h.len() >= 6 {
        (&h[0..2], &h[2..4], &h[4..6])
    } else {
        ("FF", "FF", "FF")
    };
    format!("&H{}{}{}{}", alpha, b, g, r).to_uppercase()
}

fn ass_time(ms: u64) -> String {
    let cs = ms / 10;
    let s = cs / 100;
    let m = s / 60;
    let h = m / 60;
    format!("{}:{:02}:{:02}.{:02}", h, m % 60, s % 60, cs % 100)
}

fn escape_ass_text(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('{', "\\{")
        .replace('}', "\\}")
        .replace('\n', "\\N")
}

/// 生成 ASS 字幕文件内容
pub fn build_ass(cues: &[SubtitleCue], cfg: &SubtitleConfig, width: u32, height: u32) -> String {
    let mut s = String::new();
    s.push_str("[Script Info]\n");
    s.push_str("ScriptType: v4.00+\n");
    s.push_str(&format!("PlayResX: {}\n", width));
    s.push_str(&format!("PlayResY: {}\n", height));
    s.push_str("WrapStyle: 2\n");
    s.push_str("ScaledBorderAndShadow: yes\n\n");

    s.push_str("[V4+ Styles]\n");
    s.push_str("Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n");
    let primary = ass_color(&cfg.primary_color, "00");
    let outline_color = ass_color(&cfg.outline_color, "00");
    s.push_str(&format!(
        "Style: Default,{},{},{},{},{},&H80000000,0,0,0,0,100,100,0,0,1,{},0.6,2,20,20,{},1\n\n",
        cfg.font_name,
        cfg.font_size,
        primary,
        primary,
        outline_color,
        cfg.outline,
        cfg.margin_v
    ));

    s.push_str("[Events]\n");
    s.push_str("Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n");
    for cue in cues {
        if cue.end_ms <= cue.start_ms {
            continue;
        }
        s.push_str(&format!(
            "Dialogue: 0,{},{},Default,,0,0,0,,{}\n",
            ass_time(cue.start_ms),
            ass_time(cue.end_ms),
            escape_ass_text(&cue.text)
        ));
    }
    s
}
