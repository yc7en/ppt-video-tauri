use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{bail, Context, Result};
use crate::models::SlideInfo;

#[async_trait::async_trait]
pub trait PptParser: Send + Sync {
    /// 导出演示文稿（PPT/PPTX/PDF）幻灯片为 PNG 文件列表
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

        collect_png_slides(output_dir, "PowerPoint 导出未产生任何 PNG 图片")
    }
}

pub struct WindowsNativePdfParser;

#[async_trait::async_trait]
impl PptParser for WindowsNativePdfParser {
    async fn export_slides(&self, pdf_path: &Path, output_dir: &Path) -> Result<Vec<SlideInfo>> {
        fs::create_dir_all(output_dir)?;

        let pdf_abs = pdf_path.canonicalize().context("Invalid PDF path")?;
        let out_abs = output_dir.canonicalize().unwrap_or_else(|_| output_dir.to_path_buf());

        let pdf_clean = pdf_abs.to_string_lossy().trim_start_matches(r"\\?\").to_string();
        let out_clean = out_abs.to_string_lossy().trim_start_matches(r"\\?\").to_string();

        // 使用 Windows 10/11 原生 WinRT (Windows.Data.Pdf) 执行超高保真高解析度渲染 (1920宽)
        let script = format!(
            r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName 'System.Runtime.WindowsRuntime'

$asTaskGeneric = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {{ 
    $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1'
}})[0]

function Await($asyncOperation, $resultType) {{
    $method = $asTaskGeneric.MakeGenericMethod($resultType)
    $task = $method.Invoke($null, @($asyncOperation))
    $task.Wait()
    return $task.Result
}}

$asTaskAction = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {{ 
    $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncAction'
}})[0]

function AwaitAction($asyncAction) {{
    $task = $asTaskAction.Invoke($null, @($asyncAction))
    $task.Wait()
}}

[Windows.Storage.StorageFile, Windows.Storage, ContentType = WindowsRuntime] | Out-Null
[Windows.Data.Pdf.PdfDocument, Windows.Data.Pdf, ContentType = WindowsRuntime] | Out-Null

$storageFileAsync = [Windows.Storage.StorageFile]::GetFileFromPathAsync('{pdf}')
$storageFile = Await $storageFileAsync ([Windows.Storage.StorageFile])

$pdfDocAsync = [Windows.Data.Pdf.PdfDocument]::LoadFromFileAsync($storageFile)
$pdfDoc = Await $pdfDocAsync ([Windows.Data.Pdf.PdfDocument])

$count = $pdfDoc.PageCount
Write-Output "COUNT:$count"

for ($i = 0; $i -lt $count; $i++) {{
    $page = $pdfDoc.GetPage($i)
    $outStream = [Windows.Storage.Streams.InMemoryRandomAccessStream]::new()
    
    $opts = [Windows.Data.Pdf.PdfPageRenderOptions]::new()
    $opts.DestinationWidth = 1920
    
    $renderAsync = $page.RenderToStreamAsync($outStream, $opts)
    AwaitAction $renderAsync
    
    $outFileName = ("slide_{{0:D4}}.png" -f ($i + 1))
    $outPath = Join-Path '{out}' $outFileName
    
    $inputStream = $outStream.GetInputStreamAt(0)
    $dotnetStream = [System.IO.WindowsRuntimeStreamExtensions]::AsStreamForRead($inputStream)
    $fileStream = [System.IO.File]::Create($outPath)
    $dotnetStream.CopyTo($fileStream)
    
    $fileStream.Dispose()
    $dotnetStream.Dispose()
    $outStream.Dispose()
    $page.Dispose()
}}
"#,
            pdf = pdf_clean.replace('\'', "''"),
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
            .context("Failed to invoke PowerShell for Native WinRT PDF export")?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            bail!("PDF 解析失败: {err_msg}");
        }

        collect_png_slides(output_dir, "PDF 渲染未产生任何 PNG 图片")
    }
}

/// 统一的演示文稿解析入口：根据后缀自动分流 PPT/PPTX 或 PDF
pub struct UniversalPresentationParser;

#[async_trait::async_trait]
impl PptParser for UniversalPresentationParser {
    async fn export_slides(&self, doc_path: &Path, output_dir: &Path) -> Result<Vec<SlideInfo>> {
        let ext = doc_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if ext == "pdf" {
            WindowsNativePdfParser.export_slides(doc_path, output_dir).await
        } else if ext == "ppt" || ext == "pptx" {
            WindowsOfficeComParser.export_slides(doc_path, output_dir).await
        } else {
            bail!("不支持的文件格式: .{ext}，仅支持 .pptx, .ppt, .pdf");
        }
    }
}

fn collect_png_slides(output_dir: &Path, empty_error_msg: &str) -> Result<Vec<SlideInfo>> {
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
        bail!("{empty_error_msg}");
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

pub fn load_image_base64(path: &Path) -> Option<String> {
    use base64::Engine;
    if let Ok(bytes) = fs::read(path) {
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        Some(format!("data:image/png;base64,{}", b64))
    } else {
        None
    }
}
