<template>
  <div class="app-container">
    <!-- 顶部步骤导航栏 -->
    <header class="navbar" data-tauri-drag-region>
      <div class="logo" data-tauri-drag-region>
        <span class="logo-icon">🎬</span>
        <span class="logo-title">PPT 视频生成器</span>
        <span class="badge">Tauri + Rust</span>
      </div>
      <nav class="step-nav">
        <button
          v-for="(step, idx) in steps"
          :key="idx"
          class="step-btn"
          :class="{ active: currentStep === idx, disabled: (idx === 1 || idx === 2) && !project }"
          @click="goToStep(idx)"
        >
          {{ step }}
        </button>
      </nav>
      <div class="nav-extra" data-tauri-drag-region>
        <span class="portable-tag">便携模式</span>
      </div>
    </header>

    <!-- 主体区域 -->
    <main class="main-content">
      <!-- 步骤 1：新建或打开项目 -->
      <section v-if="currentStep === 0" class="step-page page-upload">
        <div class="upload-container">
          <!-- 上方：新建项目（选择 PPT） -->
          <div class="upload-card">
            <div class="card-header">
              <span class="card-icon">🎬</span>
              <div class="header-text">
                <h2>新建项目（选择并解析 PPT）</h2>
                <p class="upload-tip">选择本地 .ppt / .pptx 文件，通过本机 Office 自动完成高保真切片</p>
              </div>
            </div>
            
            <div class="input-group">
              <input
                v-model="pptInputPath"
                type="text"
                placeholder="点击【选择文件】或粘贴绝对路径..."
                class="path-input"
              />
              <button class="secondary-btn" @click="handleSelectPptFile">
                📁 选择文件...
              </button>
              <button class="primary-btn" :disabled="loading || !pptInputPath" @click="handleParsePpt">
                {{ loading ? "正在解析..." : "开始解析" }}
              </button>
            </div>

            <div v-if="project" class="project-info-card">
              <div class="info-item">
                <span class="label">已解析项目：</span>
                <span class="val">{{ project.ppt_stem }}</span>
              </div>
              <div class="info-item">
                <span class="label">幻灯片总数：</span>
                <span class="val">{{ project.slide_count }} 页</span>
              </div>
              <button class="secondary-btn" @click="goToStep(1)">前往文案编辑 ▶</button>
            </div>
          </div>

          <!-- 下方：打开已有项目 -->
          <div class="recent-card">
            <div class="recent-header">
              <div class="recent-title">
                <h3>💾 打开已有项目</h3>
                <span class="recent-tip">载入历史项目，无需重复解析 PPT，直接进入文案与音频编辑</span>
              </div>
              <button class="tool-btn" @click="handleOpenProjectFile">
                📂 浏览 project.json...
              </button>
            </div>

            <div v-if="recentProjects.length > 0" class="projects-grid">
              <div
                v-for="p in recentProjects"
                :key="p.path"
                class="project-tile"
                @click="handleLoadProject(p.path)"
              >
                <div class="tile-header">
                  <span class="tile-icon">📊</span>
                  <span class="tile-name" :title="p.name">{{ p.name }}</span>
                </div>
                <div class="tile-footer">
                  <span class="tile-badge">{{ p.slide_count }} 页幻灯片</span>
                  <span class="tile-date">{{ formatDate(p.updated_at) }}</span>
                </div>
              </div>
            </div>
            <div v-else class="empty-projects">
              <span>暂无历史项目记录，新建并解析 PPT 后将自动归档于此处。</span>
            </div>
          </div>
        </div>
      </section>

      <!-- 步骤 2：编辑文案 -->
      <section v-if="currentStep === 1" class="step-page page-script">
        <div class="script-toolbar" data-tauri-drag-region>
          <div class="toolbar-left">
            <button class="tool-btn" @click="handleImportMd">📥 导入 Markdown</button>
            <button class="tool-btn" @click="handleExportMd">📤 导出 Markdown</button>
            <button class="tool-btn danger" @click="handleClearScripts">🗑 清空文案</button>
          </div>
          <div class="toolbar-right" data-tauri-drag-region>
            <span class="hint-text">提示：支持 <code>{role:角色名}</code> 标签与 <code>[break=100ms]</code> 停顿</span>
          </div>
        </div>

        <div class="script-workspace" :class="{ 'is-resizing': isResizingSidebar || isResizingPreview }">
          <!-- 左侧缩略图列表 (可拖拽调整宽度) -->
          <aside class="slide-sidebar" :style="{ width: sidebarWidth + 'px' }">
            <div
              v-for="(slide, sIdx) in project?.slides"
              :key="slide.index"
              class="slide-thumb-item"
              :class="{ selected: selectedSlideIdx === sIdx }"
              @click="selectedSlideIdx = sIdx"
            >
              <div class="thumb-header">
                <span>第 {{ slide.index }} 页</span>
                <span v-if="slide.audio_path" class="audio-badge">🎵 已配音</span>
              </div>
              <div class="thumb-img-wrapper">
                <img :src="slide.image_data || toFileUrl(slide.image_path)" alt="Slide" class="thumb-img" />
              </div>
            </div>
          </aside>

          <!-- 左右分割拖动条 -->
          <div
            class="resizer-horizontal"
            :class="{ active: isResizingSidebar }"
            @mousedown="startResizeSidebar"
            title="左右拖动调整列表宽度"
          ></div>

          <!-- 右侧当前页编辑区 (包含上下拖拽分割) -->
          <div class="slide-editor-panel" ref="editorPanelRef" v-if="currentSlide">
            <!-- 高清预览图：默认占 2/3 高度 (66.7%)，支持上下拖动调整 -->
            <div class="preview-box" :style="{ height: previewHeightPercent + '%' }">
              <img :src="currentSlide.image_data || toFileUrl(currentSlide.image_path)" alt="Preview" class="preview-img" />
            </div>

            <!-- 上下分割拖动条 -->
            <div
              class="resizer-vertical"
              :class="{ active: isResizingPreview }"
              @mousedown="startResizePreview"
              title="上下拖动调整图片与文案高度"
            ></div>

            <!-- 脚本输入框：占据剩余高度 -->
            <div class="editor-box">
              <div class="editor-label-bar">
                <span>朗读文案：</span>
              </div>
              <textarea
                v-model="currentSlide.script_text"
                class="script-textarea"
                placeholder="在此输入本页朗读文案...&#10;支持多角色语法：{role:李磊} 大家好&#10;支持停顿语法：[break=200ms]"
                @input="handleScriptChanged"
              ></textarea>
            </div>

            <!-- 底部控制栏：左(配音/试听) - 中(页码) - 右(上一页/下一页) -->
            <div class="tts-single-bar">
              <!-- 左侧：生成配音与音频播放 -->
              <div class="bar-left">
                <button class="primary-btn sm" :disabled="loadingTts || !currentSlide.script_text" @click="handleSynthesizeSingle">
                  {{ loadingTts ? "正在合成..." : "🎤 生成配音" }}
                </button>
                <button v-if="currentSlide.audio_path" class="secondary-btn sm" @click="handlePlayAudio(currentSlide.audio_path)">
                  ▶ 试听播放
                </button>
                <span v-if="currentSlide.audio_duration" class="audio-dur-text">
                  时长: {{ currentSlide.audio_duration.toFixed(1) }}s
                </span>
              </div>

              <!-- 中间：第 2/3 页 -->
              <div class="bar-center">
                <span class="slide-pager-badge">
                  第 <b>{{ currentSlide.index }}</b> / {{ project?.slide_count }} 页
                </span>
              </div>

              <!-- 右侧：上一页、下一页 -->
              <div class="bar-right">
                <button
                  class="nav-btn"
                  :disabled="selectedSlideIdx === 0"
                  @click="selectedSlideIdx--"
                  title="上一页"
                >
                  ◀ 上一页
                </button>
                <button
                  class="nav-btn"
                  :disabled="selectedSlideIdx >= (project?.slide_count || 1) - 1"
                  @click="selectedSlideIdx++"
                  title="下一页"
                >
                  下一页 ▶
                </button>
              </div>
            </div>
          </div>
        </div>
      </section>

      <!-- 步骤 3：生成视频 (含多角色音色映射新需求) -->
      <section v-if="currentStep === 2" class="step-page page-generate">
        <div class="generate-card">
          <!-- 多角色音色配置优化模块 -->
          <div class="role-config-section">
            <div class="section-title">
              <h3>🎭 多角色音色配置</h3>
              <span class="sub-tip">在脚本中通过 <code>{role:角色名}</code> 切换发音人，最多配置3个角色</span>
            </div>

            <div class="role-grid">
              <div
                v-for="role in roleConfigs"
                :key="role.role_id"
                class="role-row"
                :class="{ fallback: role.role_id === 1 }"
              >
                <!-- 固定标签不可修改 -->
                <div class="role-label-badge">
                  角色 {{ role.role_id }}
                  <span v-if="role.role_id === 1" class="fallback-tag">默认兜底</span>
                </div>

                <!-- 音色选择 -->
                <div class="form-item">
                  <label>音色：</label>
                  <select v-model="role.voice" class="select-input">
                    <option value="longyingjing_v3">longyingjing_v3 (知性女声)</option>
                    <option value="longanyang">longanyang (阳光男声)</option>
                    <option value="longxiaochun_v2">longxiaochun_v2 (温柔女声)</option>
                  </select>
                </div>

                <!-- 语速配置 (已从设置页迁移至此) -->
                <div class="form-item">
                  <label>语速: {{ role.speed.toFixed(1) }}x</label>
                  <input
                    type="range"
                    v-model.number="role.speed"
                    min="0.5"
                    max="2.0"
                    step="0.1"
                    class="speed-slider"
                  />
                </div>

                <!-- 对应脚本角色名称输入框 -->
                <div class="form-item grow">
                  <label>脚本中对应的角色名：</label>
                  <input
                    type="text"
                    v-model="role.role_name"
                    class="text-input"
                    :placeholder="role.role_id === 1 ? '如: 李磊 (首个角色自动填入)' : '如: 韩梅梅 / 旁白'"
                  />
                </div>
              </div>
            </div>

            <div class="role-notes">
              <p>💡 <b>规则说明：</b></p>
              <ul>
                <li>读取脚本后，首次检测到的角色（例如 <code>{role:李磊}</code>）会自动填入<b>角色1</b>。</li>
                <li>脚本中未指定角色、或角色名匹配不到声音的段落，<b>默认使用角色1的音色与语速作为兜底</b>。</li>
              </ul>
            </div>
          </div>

          <!-- 视频生成控制区 -->
          <div class="compose-control-section">
            <div class="action-row">
              <button class="primary-btn lg" :disabled="loadingCompose" @click="handleComposeVideo">
                {{ loadingCompose ? "正在合成视频..." : "🎬 开始合成视频" }}
              </button>
              <button class="secondary-btn lg" @click="handleOpenOutput">📂 打开输出目录</button>
            </div>

            <div v-if="composeStatus" class="compose-status-box">
              <div class="status-msg">{{ composeStatus }}</div>
              <div v-if="lastVideoPath" class="done-path">
                已生成文件: <code>{{ lastVideoPath }}</code>
              </div>
            </div>
          </div>
        </div>
      </section>

      <!-- 步骤 4：全局设置 -->
      <section v-if="currentStep === 3" class="step-page page-settings">
        <div class="settings-card">
          <h2>系统设置</h2>
          <p class="settings-desc">配置 TTS 接口调用地址与视频参数（便携模式配置自动存储在当前程序同级目录）</p>

          <!-- TTS 接口设置 -->
          <div class="settings-group">
            <h3>🎙 语音合成 TTS 接口</h3>
            
            <div class="setting-item">
              <label>API 接口调用地址 (Base URL)：</label>
              <input
                type="text"
                v-model="appConfig.tts.api_url"
                class="text-input"
                placeholder="默认阿里云: https://dashscope.aliyuncs.com/... 亦可填写本地服务如 http://127.0.0.1:8000/v1"
              />
              <span class="field-hint">支持切换为本地部署的 CosyVoice 或 OpenAI 兼容端点接口。</span>
            </div>

            <div class="setting-item">
              <label>API Key 密钥：</label>
              <input
                type="password"
                v-model="appConfig.tts.api_key"
                class="text-input"
                placeholder="sk-..."
              />
              <span class="field-hint">如使用本地不需要认证的接口可留空。</span>
            </div>

            <div class="setting-item">
              <label>TTS 模型名称：</label>
              <input
                type="text"
                v-model="appConfig.tts.model"
                class="text-input"
                placeholder="cosyvoice-v3-flash"
              />
            </div>
          </div>

          <!-- 视频参数设置 -->
          <div class="settings-group">
            <h3>🎬 视频合成参数</h3>
            <div class="setting-grid">
              <div class="setting-item">
                <label>输出分辨率：</label>
                <select v-model="resolutionModel" class="select-input">
                  <option value="1920x1080">1920 × 1080 (1080p 全高清)</option>
                  <option value="1280x720">1280 × 720 (720p 高清)</option>
                  <option value="3840x2160">3840 × 2160 (4K 超高清)</option>
                </select>
              </div>

              <div class="setting-item">
                <label>合成帧率 (FPS)：</label>
                <input
                  type="number"
                  v-model.number="appConfig.video.fps"
                  class="text-input"
                  min="1"
                  max="60"
                />
              </div>

              <div class="setting-item">
                <label>换页停顿时间 (秒)：</label>
                <input
                  type="number"
                  v-model.number="appConfig.video.slide_pause"
                  step="0.1"
                  class="text-input"
                />
              </div>

              <div class="setting-item">
                <label>无文案页默认时长 (秒)：</label>
                <input
                  type="number"
                  v-model.number="appConfig.video.default_slide_duration"
                  step="0.5"
                  class="text-input"
                />
              </div>
            </div>
          </div>

          <div class="save-bar">
            <button class="primary-btn" @click="handleSaveConfig">💾 保存全局设置</button>
            <span v-if="saveSuccess" class="success-text">✅ 设置已保存</span>
          </div>
        </div>
      </section>
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";

const steps = ["① 上传 PPT", "② 编辑文案", "③ 生成视频", "④ 设置"];
const currentStep = ref(0);

// 数据模型
interface RoleVoiceConfig {
  role_id: number;
  role_name: string;
  voice: string;
  speed: number;
}

interface SlideInfo {
  index: number;
  image_path: string;
  image_data?: string;
  script_text: string;
  audio_path?: string;
  audio_duration: number;
  audio_versions: string[];
}

interface ProjectState {
  ppt_path: string;
  ppt_stem: string;
  project_dir: string;
  slide_count: number;
  slides: SlideInfo[];
  role_configs: RoleVoiceConfig[];
}

interface AppConfig {
  tts: {
    api_url: string;
    api_key: string;
    model: string;
  };
  video: {
    fps: number;
    resolution: [number, number];
    default_slide_duration: number;
    slide_pause: number;
  };
  role_configs: RoleVoiceConfig[];
  output_dir: string;
}

interface ProjectSummary {
  name: string;
  path: string;
  slide_count: number;
  updated_at: string;
}

// 状态
const pptInputPath = ref("");
const loading = ref(false);
const loadingTts = ref(false);
const loadingCompose = ref(false);
const project = ref<ProjectState | null>(null);
const recentProjects = ref<ProjectSummary[]>([]);
const selectedSlideIdx = ref(0);
const composeStatus = ref("");
const lastVideoPath = ref("");
const saveSuccess = ref(false);

// 侧边栏与预览区大小拖拽
const sidebarWidth = ref(220);
const isResizingSidebar = ref(false);

function startResizeSidebar(e: MouseEvent) {
  isResizingSidebar.value = true;
  const startX = e.clientX;
  const startWidth = sidebarWidth.value;

  function onMouseMove(ev: MouseEvent) {
    const delta = ev.clientX - startX;
    const targetW = Math.max(160, Math.min(480, startWidth + delta));
    sidebarWidth.value = targetW;
  }

  function onMouseUp() {
    isResizingSidebar.value = false;
    window.removeEventListener("mousemove", onMouseMove);
    window.removeEventListener("mouseup", onMouseUp);
  }

  window.addEventListener("mousemove", onMouseMove);
  window.addEventListener("mouseup", onMouseUp);
}

// 预览图与文案高度比例拖拽 (默认 66.7% 即 2/3)
const previewHeightPercent = ref(66.7);
const isResizingPreview = ref(false);
const editorPanelRef = ref<HTMLElement | null>(null);

function startResizePreview(e: MouseEvent) {
  if (!editorPanelRef.value) return;
  isResizingPreview.value = true;
  const startY = e.clientY;
  const panelHeight = editorPanelRef.value.clientHeight - 60;
  const startPercent = previewHeightPercent.value;

  function onMouseMove(ev: MouseEvent) {
    const deltaY = ev.clientY - startY;
    const deltaPercent = (deltaY / panelHeight) * 100;
    const targetPercent = Math.max(25, Math.min(85, startPercent + deltaPercent));
    previewHeightPercent.value = targetPercent;
  }

  function onMouseUp() {
    isResizingPreview.value = false;
    window.removeEventListener("mousemove", onMouseMove);
    window.removeEventListener("mouseup", onMouseUp);
  }

  window.addEventListener("mousemove", onMouseMove);
  window.addEventListener("mouseup", onMouseUp);
}

const appConfig = ref<AppConfig>({
  tts: {
    api_url: "https://dashscope.aliyuncs.com/api/v1/services/audio/tts/generation",
    api_key: "",
    model: "cosyvoice-v3-flash",
  },
  video: {
    fps: 5,
    resolution: [1920, 1080],
    default_slide_duration: 3.0,
    slide_pause: 0.5,
  },
  role_configs: [
    { role_id: 1, role_name: "", voice: "longyingjing_v3", speed: 1.0 },
    { role_id: 2, role_name: "", voice: "longanyang", speed: 1.0 },
    { role_id: 3, role_name: "", voice: "longxiaochun_v2", speed: 1.0 },
  ],
  output_dir: "output",
});

const roleConfigs = computed(() => {
  return project.value ? project.value.role_configs : appConfig.value.role_configs;
});

const currentSlide = computed(() => {
  if (!project.value || project.value.slides.length === 0) return null;
  return project.value.slides[selectedSlideIdx.value] || null;
});

const resolutionModel = computed({
  get() {
    const [w, h] = appConfig.value.video.resolution;
    return `${w}x${h}`;
  },
  set(val: string) {
    const parts = val.split("x").map(Number);
    if (parts.length === 2) {
      appConfig.value.video.resolution = [parts[0], parts[1]];
    }
  },
});

onMounted(async () => {
  try {
    const cfg = await invoke<AppConfig>("get_app_config");
    appConfig.value = cfg;
  } catch (err) {
    console.error("Failed to load config:", err);
  }
  loadRecentProjects();
});

function goToStep(idx: number) {
  if ((idx === 1 || idx === 2) && !project.value) return;
  currentStep.value = idx;
}

function toFileUrl(path: string) {
  if (!path) return "";
  try {
    return convertFileSrc(path);
  } catch {
    return "";
  }
}

function formatDate(dtStr?: string) {
  if (!dtStr) return "";
  return dtStr.replace("T", " ").substring(0, 19);
}

async function loadRecentProjects() {
  try {
    const list = await invoke<ProjectSummary[]>("list_recent_projects_cmd");
    recentProjects.value = list;
  } catch (err) {
    console.error("Failed to load recent projects:", err);
  }
}

// 步骤 1：选择本地 PPT 文件
async function handleSelectPptFile() {
  try {
    const selected = await invoke<string | null>("select_ppt_file_cmd");
    if (selected) {
      pptInputPath.value = selected;
    }
  } catch (err) {
    console.error("Select PPT file error:", err);
  }
}

// 步骤 1：浏览并打开已有项目文件
async function handleOpenProjectFile() {
  try {
    const selected = await invoke<string | null>("select_project_file_cmd");
    if (selected) {
      await handleLoadProject(selected);
    }
  } catch (err) {
    console.error("Select project file error:", err);
  }
}

// 步骤 1：载入指定项目
async function handleLoadProject(projPath: string) {
  loading.value = true;
  try {
    const proj = await invoke<ProjectState>("load_project_cmd", {
      projectJson: projPath,
    });
    project.value = proj;
    selectedSlideIdx.value = 0;
    goToStep(1);
    loadRecentProjects();
  } catch (err: any) {
    alert("打开项目失败: " + err);
  } finally {
    loading.value = false;
  }
}

// 步骤 1：解析 PPT
async function handleParsePpt() {
  if (!pptInputPath.value.trim()) return;
  loading.value = true;
  try {
    const proj = await invoke<ProjectState>("create_project_cmd", {
      pptPath: pptInputPath.value.trim(),
    });
    project.value = proj;
    selectedSlideIdx.value = 0;
    goToStep(1);
    loadRecentProjects();
  } catch (err: any) {
    alert("解析 PPT 失败: " + err);
  } finally {
    loading.value = false;
  }
}

// 步骤 2：脚本修改自动检测首个角色
async function handleScriptChanged() {
  if (!currentSlide.value) return;
  try {
    const firstRole = await invoke<string | null>("detect_first_role_cmd", {
      text: currentSlide.value.script_text,
    });
    // 落实需求1：若角色1名称为空，自动填入首个检测到的角色
    if (firstRole && roleConfigs.value[0] && !roleConfigs.value[0].role_name.trim()) {
      roleConfigs.value[0].role_name = firstRole;
    }
  } catch (_) {}
}

// 单页 TTS 合成
async function handleSynthesizeSingle() {
  if (!currentSlide.value || !project.value) return;
  loadingTts.value = true;
  try {
    const updated = await invoke<SlideInfo>("synthesize_slide_cmd", {
      slideIndex: currentSlide.value.index,
      text: currentSlide.value.script_text,
      projectDir: project.value.project_dir,
      roleConfigs: roleConfigs.value,
    });
    currentSlide.value.audio_path = updated.audio_path;
    currentSlide.value.audio_duration = updated.audio_duration;
    // 自动保存项目
    await invoke("save_project_cmd", { project: project.value });
  } catch (err: any) {
    alert("TTS 生成失败: " + err);
  } finally {
    loadingTts.value = false;
  }
}

// 播放音频
function handlePlayAudio(path?: string) {
  if (!path) return;
  invoke("open_path_cmd", { path });
}

// 导入 Markdown
function handleImportMd() {
  const input = document.createElement("input");
  input.type = "file";
  input.accept = ".md,.txt";
  input.onchange = async (e: any) => {
    const file = e.target.files[0];
    if (!file || !project.value) return;
    const text = await file.text();
    const sections = text.split("\n---\n");
    sections.forEach((sec: string, idx: number) => {
      if (project.value && idx < project.value.slides.length) {
        project.value.slides[idx].script_text = sec.trim();
      }
    });
    handleScriptChanged();
  };
  input.click();
}

// 导出 Markdown
function handleExportMd() {
  if (!project.value) return;
  const content = project.value.slides.map((s) => s.script_text || "").join("\n\n---\n\n");
  const blob = new Blob([content], { type: "text/markdown;charset=utf-8" });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = `${project.value.ppt_stem}_scripts.md`;
  a.click();
}

function handleClearScripts() {
  if (!project.value || !confirm("确认清空所有幻灯片文案？")) return;
  project.value.slides.forEach((s) => (s.script_text = ""));
}

// 步骤 3：视频合成
async function handleComposeVideo() {
  if (!project.value) return;
  loadingCompose.value = true;
  composeStatus.value = "正在合成视频切片并调用 FFmpeg 拼接...";
  try {
    const outMp4 = await invoke<string>("compose_video_cmd", {
      project: project.value,
      videoConfig: appConfig.value.video,
    });
    lastVideoPath.value = outMp4;
    composeStatus.value = "✅ 视频合成成功！";
  } catch (err: any) {
    composeStatus.value = "❌ 合成失败: " + err;
  } finally {
    loadingCompose.value = false;
  }
}

function handleOpenOutput() {
  if (!project.value) return;
  invoke("open_path_cmd", { path: `${project.value.project_dir}/output` });
}

// 步骤 4：保存配置
async function handleSaveConfig() {
  try {
    await invoke("update_app_config", { cfg: appConfig.value });
    saveSuccess.value = true;
    setTimeout(() => (saveSuccess.value = false), 2000);
  } catch (err: any) {
    alert("保存失败: " + err);
  }
}
</script>

<style scoped>
/* 全局暗色调现代风格 */
.app-container {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: #1e1e2e;
  color: #e8e8f0;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  overflow: hidden;
}

.navbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 58px;
  background: #252538;
  border-bottom: 1px solid #3a3a55;
  padding: 0 24px;
}

.logo {
  display: flex;
  align-items: center;
  gap: 10px;
  font-weight: 700;
  font-size: 16px;
}

.badge {
  font-size: 11px;
  background: #7c6af7;
  color: #fff;
  padding: 2px 8px;
  border-radius: 12px;
}

.step-nav {
  display: flex;
  gap: 8px;
}

.step-btn {
  background: transparent;
  border: none;
  color: #9898b0;
  padding: 8px 16px;
  font-size: 14px;
  cursor: pointer;
  border-bottom: 2px solid transparent;
  transition: all 0.2s;
}

.step-btn.active {
  color: #7c6af7;
  font-weight: 700;
  border-bottom-color: #7c6af7;
}

.step-btn.disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.portable-tag {
  font-size: 12px;
  color: #3ddc84;
  background: rgba(61, 220, 132, 0.1);
  padding: 3px 8px;
  border-radius: 4px;
  border: 1px solid rgba(61, 220, 132, 0.3);
}

.main-content {
  flex: 1;
  overflow-y: auto;
  padding: 24px;
}

/* 按钮规范 */
.primary-btn {
  background: #7c6af7;
  color: #fff;
  border: none;
  padding: 10px 24px;
  border-radius: 8px;
  font-weight: 600;
  cursor: pointer;
  transition: opacity 0.2s;
}

.primary-btn:hover {
  opacity: 0.9;
}

.primary-btn.sm {
  padding: 6px 14px;
  font-size: 13px;
}

.primary-btn.lg {
  padding: 12px 32px;
  font-size: 16px;
}

.secondary-btn {
  background: #313147;
  color: #e8e8f0;
  border: 1px solid #3a3a55;
  padding: 10px 20px;
  border-radius: 8px;
  cursor: pointer;
}

.secondary-btn.sm {
  padding: 6px 14px;
  font-size: 13px;
}

/* 步骤 1 上传与项目管理 */
.page-upload {
  display: flex;
  justify-content: center;
  align-items: flex-start;
  min-height: 100%;
  padding: 20px 0;
}

.upload-container {
  display: flex;
  flex-direction: column;
  gap: 24px;
  width: 100%;
  max-width: 820px;
}

.upload-card {
  background: #2a2a3e;
  border: 1px solid #3a3a55;
  border-radius: 12px;
  padding: 32px;
  text-align: left;
}

.card-header {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-bottom: 20px;
}

.card-icon {
  font-size: 36px;
}

.header-text h2 {
  font-size: 18px;
  font-weight: 700;
  margin: 0 0 4px 0;
}

.upload-tip {
  color: #9898b0;
  font-size: 13px;
  margin: 0;
}

.input-group {
  display: flex;
  gap: 10px;
}

.path-input {
  flex: 1;
  background: #1e1e2e;
  border: 1px solid #3a3a55;
  color: #fff;
  padding: 10px 14px;
  border-radius: 8px;
}

.project-info-card {
  margin-top: 24px;
  padding-top: 20px;
  border-top: 1px solid #3a3a55;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

/* 最近项目卡片 */
.recent-card {
  background: #2a2a3e;
  border: 1px solid #3a3a55;
  border-radius: 12px;
  padding: 24px 32px;
}

.recent-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
}

.recent-title h3 {
  font-size: 16px;
  font-weight: 700;
  margin: 0 0 4px 0;
}

.recent-tip {
  color: #9898b0;
  font-size: 12px;
}

.projects-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 14px;
  max-height: 280px;
  overflow-y: auto;
  padding-right: 4px;
}

.project-tile {
  background: #1e1e2e;
  border: 1px solid #3a3a55;
  border-radius: 8px;
  padding: 14px;
  cursor: pointer;
  transition: all 0.2s ease;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 12px;
}

.project-tile:hover {
  border-color: #7c6af7;
  transform: translateY(-2px);
  background: #232338;
}

.tile-header {
  display: flex;
  align-items: center;
  gap: 8px;
}

.tile-icon {
  font-size: 18px;
}

.tile-name {
  font-weight: 600;
  font-size: 14px;
  color: #e8e8f0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 150px;
}

.tile-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 11px;
  color: #9898b0;
}

.tile-badge {
  background: rgba(124, 106, 247, 0.15);
  color: #a395f8;
  padding: 2px 6px;
  border-radius: 4px;
}

.empty-projects {
  padding: 28px 0;
  text-align: center;
  color: #686882;
  font-size: 13px;
}

/* 步骤 2 脚本编辑 */
.page-script {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.script-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 14px;
}

.tool-btn {
  background: #2a2a3e;
  border: 1px solid #3a3a55;
  color: #e8e8f0;
  padding: 6px 14px;
  border-radius: 6px;
  cursor: pointer;
  margin-right: 8px;
}

.tool-btn.danger {
  color: #ff5252;
}

.script-workspace {
  display: flex;
  flex: 1;
  overflow: hidden;
  position: relative;
  gap: 0;
}

.script-workspace.is-resizing {
  user-select: none;
}

.slide-sidebar {
  flex-shrink: 0;
  background: #252538;
  border-radius: 8px;
  border: 1px solid #3a3a55;
  overflow-y: auto;
  padding: 8px;
}

.slide-thumb-item {
  padding: 8px;
  border-radius: 6px;
  cursor: pointer;
  margin-bottom: 6px;
  border: 2px solid transparent;
}

.slide-thumb-item.selected {
  border-color: #7c6af7;
  background: #313147;
}

.thumb-header {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  margin-bottom: 4px;
}

.audio-badge {
  color: #3ddc84;
}

.thumb-img {
  width: 100%;
  aspect-ratio: 16/9;
  background: #1e1e2e;
  border-radius: 4px;
  object-fit: contain;
}

/* 左右分割拖拽条 */
.resizer-horizontal {
  width: 10px;
  cursor: col-resize;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.15s;
  flex-shrink: 0;
  margin: 0 4px;
  border-radius: 4px;
}

.resizer-horizontal:hover,
.resizer-horizontal.active {
  background: rgba(124, 106, 247, 0.35);
}

.resizer-horizontal::after {
  content: "";
  width: 2px;
  height: 40px;
  background: #4a4a68;
  border-radius: 1px;
}

.resizer-horizontal:hover::after,
.resizer-horizontal.active::after {
  background: #7c6af7;
}

/* 右侧编辑面板 */
.slide-editor-panel {
  flex: 1;
  min-width: 0;
  background: #252538;
  border-radius: 8px;
  border: 1px solid #3a3a55;
  display: flex;
  flex-direction: column;
  padding: 12px 16px;
  overflow: hidden;
}

/* 高清预览图默认占 2/3 高度 */
.preview-box {
  width: 100%;
  background: #181824;
  border-radius: 6px;
  border: 1px solid #2f2f45;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  padding: 10px;
  box-sizing: border-box;
  flex-shrink: 0;
}

.preview-img {
  max-height: 100%;
  max-width: 100%;
  object-fit: contain;
  border-radius: 4px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
}

/* 上下分割拖拽条 */
.resizer-vertical {
  height: 12px;
  cursor: row-resize;
  display: flex;
  align-items: center;
  justify-content: center;
  margin: 4px 0;
  transition: background 0.15s;
  border-radius: 4px;
  flex-shrink: 0;
}

.resizer-vertical:hover,
.resizer-vertical.active {
  background: rgba(124, 106, 247, 0.35);
}

.resizer-vertical::after {
  content: "";
  height: 2px;
  width: 50px;
  background: #4a4a68;
  border-radius: 1px;
}

.resizer-vertical:hover::after,
.resizer-vertical.active::after {
  background: #7c6af7;
}

/* 脚本编辑区 */
.editor-box {
  flex: 1;
  min-height: 70px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  overflow: hidden;
}

.editor-label-bar {
  font-size: 13px;
  font-weight: 600;
  color: #9898b0;
  flex-shrink: 0;
}

.script-textarea {
  flex: 1;
  width: 100%;
  box-sizing: border-box;
  background: #1e1e2e;
  border: 1px solid #3a3a55;
  color: #fff;
  padding: 10px 14px;
  border-radius: 6px;
  resize: none;
  font-size: 14px;
  line-height: 1.6;
  outline: none;
}

.script-textarea:focus {
  border-color: #7c6af7;
}

/* 底部操作与导航栏：左中右三段式 */
.tts-single-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-top: 10px;
  border-top: 1px solid #33334d;
  margin-top: 6px;
  flex-shrink: 0;
}

.bar-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.audio-dur-text {
  color: #9898b0;
  font-size: 13px;
}

.bar-center {
  display: flex;
  align-items: center;
  justify-content: center;
}

.slide-pager-badge {
  background: #1e1e2e;
  border: 1px solid #3a3a55;
  padding: 4px 16px;
  border-radius: 16px;
  font-size: 13px;
  color: #9898b0;
}

.slide-pager-badge b {
  color: #7c6af7;
  font-size: 14px;
}

.bar-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.nav-btn {
  background: #313147;
  border: 1px solid #3a3a55;
  color: #e8e8f0;
  padding: 6px 14px;
  border-radius: 6px;
  font-size: 13px;
  cursor: pointer;
  transition: all 0.2s;
}

.nav-btn:hover:not(:disabled) {
  background: #3d3d58;
  border-color: #7c6af7;
}

.nav-btn:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}

/* 步骤 3 多角色生成 */
.role-config-section {
  background: #252538;
  border: 1px solid #3a3a55;
  border-radius: 10px;
  padding: 20px;
  margin-bottom: 24px;
}

.section-title h3 {
  margin: 0;
  font-size: 16px;
}

.sub-tip {
  color: #9898b0;
  font-size: 12px;
}

.role-grid {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin: 16px 0;
}

.role-row {
  display: flex;
  align-items: center;
  gap: 16px;
  background: #1e1e2e;
  border: 1px solid #3a3a55;
  padding: 12px 16px;
  border-radius: 8px;
}

.role-label-badge {
  font-weight: 700;
  color: #7c6af7;
  width: 90px;
}

.fallback-tag {
  display: block;
  font-size: 10px;
  color: #ffb74d;
  font-weight: normal;
}

.form-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
  color: #9898b0;
}

.form-item.grow {
  flex: 1;
}

.select-input,
.text-input {
  background: #2a2a3e;
  border: 1px solid #3a3a55;
  color: #fff;
  padding: 8px 12px;
  border-radius: 6px;
}

.speed-slider {
  width: 110px;
}

.role-notes {
  font-size: 12px;
  color: #9898b0;
  background: rgba(124, 106, 247, 0.05);
  border: 1px dashed rgba(124, 106, 247, 0.3);
  padding: 10px 16px;
  border-radius: 6px;
}

.role-notes ul {
  margin: 4px 0 0 16px;
  padding: 0;
}

.compose-control-section {
  text-align: center;
  padding: 24px;
  background: #252538;
  border-radius: 10px;
}

.action-row {
  display: flex;
  justify-content: center;
  gap: 16px;
}

.compose-status-box {
  margin-top: 16px;
}

/* 步骤 4 设置 */
.settings-card {
  background: #252538;
  border: 1px solid #3a3a55;
  border-radius: 10px;
  padding: 24px;
  max-width: 800px;
  margin: 0 auto;
}

.settings-group {
  margin-top: 20px;
  border-top: 1px solid #3a3a55;
  padding-top: 16px;
}

.setting-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 14px;
}

.setting-item label {
  font-size: 13px;
  color: #e8e8f0;
}

.field-hint {
  font-size: 12px;
  color: #9898b0;
}

.setting-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
}

.save-bar {
  display: flex;
  align-items: center;
  gap: 14px;
  margin-top: 24px;
}

.success-text {
  color: #3ddc84;
}
</style>