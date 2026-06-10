use crate::fs_util::resolve_safe_dir;
use crate::models::{ConfluenceConvertParams, ConfluenceFileEntry, ConfluenceOutputFormat};
use base64::{engine::general_purpose::STANDARD, Engine};
use docx_rs::*;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

const PDF_ERROR: &str = "请使用 HTML 导出后在浏览器中打印为 PDF";

pub fn preprocess_markdown(markdown: &str) -> String {
    let md = preprocess_yuque_diagram_comments(markdown);
    let md = preprocess_blockquote_meta(&md);
    let re = Regex::new(r"(?is)<pre[^>]*>\s*(?:<code[^>]*>)?([\s\S]*?)(?:</code>\s*)?</pre>").unwrap();
    let md = re
        .replace_all(&md, |caps: &regex::Captures| {
            let text = caps[1]
                .replace("<br/>", "\n")
                .replace("<br />", "\n")
                .replace("<br>", "\n");
            let text = Regex::new(r"(?i)</?code[^>]*>").unwrap().replace_all(&text, "");
            let text = Regex::new(r"<[^>]+>").unwrap().replace_all(&text, "");
            format!("\n```text\n{}\n```\n", text.trim())
        })
        .to_string();
    md.replace("\r\n", "\n")
}

fn extract_diagram_source_from_comment_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let re = Regex::new(r"^<!--\s*(?:这是一个)?文本绘图[，,]?\s*源码为[：:]\s*").unwrap();
    if let Some(m) = re.find(trimmed) {
        let start = m.end();
        if let Some(end) = trimmed.rfind("-->") {
            if end > start {
                return Some(trimmed[start..end].trim().trim_end_matches(" -->").to_string());
            }
        }
    }
    None
}

fn preprocess_yuque_diagram_comments(md: &str) -> String {
    let mut out = Vec::new();
    for line in md.lines() {
        if let Some(source) = extract_diagram_source_from_comment_line(line) {
            out.push(String::new());
            out.push("```mermaid".into());
            out.push(source);
            out.push("```".into());
            out.push(String::new());
            continue;
        }
        if Regex::new(r"^\s*<!--[\s\S]*-->\s*$").unwrap().is_match(line.trim()) {
            continue;
        }
        out.push(line.to_string());
    }
    out.join("\n")
}

fn is_horizontal_rule_line(line: &str) -> bool {
    Regex::new(r"^(-{3,}|\*{3,}|_{3,})$")
        .unwrap()
        .is_match(line.trim())
}

fn preprocess_blockquote_meta(md: &str) -> String {
    let lines: Vec<&str> = md.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("> ") || trimmed == ">" {
            out.push(lines[i].to_string());
            i += 1;
            while i < lines.len() {
                let next = lines[i].trim();
                if next.is_empty() {
                    out.push(lines[i].to_string());
                    i += 1;
                    break;
                }
                if next.starts_with('>') {
                    out.push(lines[i].to_string());
                    i += 1;
                    continue;
                }
                if Regex::new(r"^#{1,6}\s").unwrap().is_match(next)
                    || is_horizontal_rule_line(lines[i])
                    || next.starts_with("```")
                {
                    break;
                }
                out.push(format!("> {}", lines[i].trim_start()));
                i += 1;
            }
            continue;
        }
        out.push(lines[i].to_string());
        i += 1;
    }
    out.join("\n")
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn render_mermaid_block(content: &str) -> String {
    let source = content.trim();
    let safe = source.replace("</pre", "<\\/pre");
    format!(
        r#"<div class="mermaid-wrapper">
  <p class="diagram-hint"><em>流程图：打开 HTML 文件或在下方预览中可自动渲染；粘贴到 Confluence 时请复制渲染后的图，或展开源码使用 Mermaid 宏。</em></p>
  <div class="mermaid-render">
    <pre class="mermaid">{safe}</pre>
  </div>
  <details class="mermaid-source">
    <summary>Mermaid 源码</summary>
    <pre class="text-diagram diagram-block"><code>{}</code></pre>
  </details>
</div>"#,
        escape_html(source)
    )
}

const MERMAID_BOOTSTRAP: &str = r#"<script type="module">
import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs';
mermaid.initialize({
  startOnLoad: false,
  theme: 'neutral',
  securityLevel: 'loose',
  flowchart: { useMaxWidth: true, htmlLabels: true, curve: 'basis' },
});
await mermaid.run({ querySelector: '.mermaid' });
</script>"#;

const HTML_STYLES: &str = r#"body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "PingFang SC", "Microsoft YaHei", sans-serif; line-height: 1.6; max-width: 900px; margin: 40px auto; padding: 0 24px; color: #172b4d; }
    h1 { font-size: 28px; border-bottom: 1px solid #dfe1e6; padding-bottom: 8px; }
    h2 { font-size: 22px; margin-top: 28px; }
    h3 { font-size: 18px; margin-top: 22px; }
    pre, pre code { white-space: pre; word-wrap: normal; overflow-x: auto; tab-size: 4; }
    pre { background: #f4f5f7; padding: 12px 16px; border-radius: 4px; margin: 16px 0; }
    pre.text-diagram, pre.diagram-block { font-family: SFMono-Regular, Consolas, "Liberation Mono", Menlo, "Courier New", monospace; line-height: 1.35; font-size: 13px; letter-spacing: 0; }
    .diagram-hint { color: #5e6c84; font-size: 13px; margin: 16px 0 6px; }
    .mermaid-wrapper { margin: 20px 0 28px; }
    .mermaid-render { background: #fff; border: 1px solid #dfe1e6; border-radius: 8px; padding: 16px; overflow-x: auto; }
    pre.mermaid { background: transparent; padding: 0; margin: 0; white-space: pre-wrap; font-family: inherit; }
    .mermaid-render svg { display: block; max-width: 100%; height: auto; margin: 0 auto; }
    .mermaid-source { margin-top: 10px; }
    .mermaid-source summary { cursor: pointer; color: #5e6c84; font-size: 13px; user-select: none; }
    code { font-family: SFMono-Regular, Consolas, "Liberation Mono", Menlo, monospace; font-size: 0.9em; }
    table { border-collapse: collapse; width: 100%; margin: 16px 0; }
    th, td { border: 1px solid #dfe1e6; padding: 8px 12px; text-align: left; }
    th { background: #f4f5f7; }
    blockquote { border-left: 4px solid #dfe1e6; margin: 16px 0; padding: 4px 16px; color: #5e6c84; }
    img { max-width: 100%; height: auto; }
    hr { border: none; border-top: 1px solid #dfe1e6; margin: 24px 0; }"#;

pub fn markdown_to_confluence_html(markdown: &str, title: &str, wrap_document: bool) -> String {
    let md = preprocess_markdown(markdown);
    let parts = split_mermaid_fences(&md);
    let mut body = String::new();

    for part in parts {
        match part {
            MdPart::Mermaid(content) => body.push_str(&render_mermaid_block(&content)),
            MdPart::Markdown(text) => {
                body.push_str(&render_markdown_with_pulldown(&text));
            }
        }
    }

    if !wrap_document {
        return body;
    }

    let page_title = if title.is_empty() { "未命名文档" } else { title };
    format!(
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>{}</title>
  <style>{HTML_STYLES}</style>
</head>
<body>
{}
{MERMAID_BOOTSTRAP}
</body>
</html>"#,
        escape_html(page_title),
        body
    )
}

enum MdPart {
    Markdown(String),
    Mermaid(String),
}

fn split_mermaid_fences(md: &str) -> Vec<MdPart> {
    let re = Regex::new(r"(?ms)^```mermaid\s*\n(.*?)^```\s*$").unwrap();
    let mut parts = Vec::new();
    let mut last = 0;
    for caps in re.captures_iter(md) {
        let full = caps.get(0).unwrap();
        if full.start() > last {
            parts.push(MdPart::Markdown(md[last..full.start()].to_string()));
        }
        parts.push(MdPart::Mermaid(caps.get(1).unwrap().as_str().to_string()));
        last = full.end();
    }
    if last < md.len() {
        parts.push(MdPart::Markdown(md[last..].to_string()));
    }
    if parts.is_empty() {
        parts.push(MdPart::Markdown(md.to_string()));
    }
    parts
}

fn render_markdown_with_pulldown(md: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(md, options);
    let mut html = String::new();
    let mut in_pre = false;
    let mut code_lang = String::new();
    let mut code_buf = String::new();

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                in_pre = true;
                code_buf.clear();
                code_lang = match kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
            }
            Event::End(TagEnd::CodeBlock) => {
                in_pre = false;
                if code_lang.to_lowercase() == "mermaid" {
                    html.push_str(&render_mermaid_block(&code_buf));
                } else {
                    let class = if code_lang.is_empty() {
                        String::new()
                    } else {
                        format!(" class=\"language-{}\"", escape_html(&code_lang))
                    };
                    html.push_str(&format!(
                        "<pre{class}><code>{}</code></pre>\n",
                        escape_html(&code_buf)
                    ));
                }
                code_lang.clear();
                code_buf.clear();
            }
            Event::Code(text) if in_pre => {
                code_buf.push_str(&text);
            }
            Event::Text(text) if in_pre => {
                code_buf.push_str(&text);
            }
            Event::SoftBreak | Event::HardBreak if in_pre => {
                code_buf.push('\n');
            }
            other if !in_pre => {
                pulldown_cmark::html::push_html(&mut html, std::iter::once(other));
            }
            _ => {}
        }
    }
    html
}

fn collect_image_urls(markdown: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let md_re = Regex::new(r#"!\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)"#).unwrap();
    for caps in md_re.captures_iter(markdown) {
        urls.push(caps[1].trim().to_string());
    }
    let img_re = Regex::new(r#"(?i)<img[^>]+src=["']([^"']+)["']"#).unwrap();
    for caps in img_re.captures_iter(markdown) {
        urls.push(caps[1].trim().to_string());
    }
    urls.sort();
    urls.dedup();
    urls
}

fn mime_from_ext(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "image/png",
    }
}

async fn load_image_buffer(image_url: &str, base_dir: &Path) -> Result<Vec<u8>, String> {
    let raw = image_url.trim();
    if raw.is_empty() {
        return Err("图片地址为空".into());
    }
    if raw.starts_with("data:") {
        if let Some(idx) = raw.find(";base64,") {
            let b64 = &raw[idx + 8..];
            return STANDARD.decode(b64).map_err(|e| e.to_string());
        }
        return Err("无效的 data URL".into());
    }
    if raw.starts_with("http://") || raw.starts_with("https://") {
        let resp = reqwest::Client::new()
            .get(raw)
            .header("User-Agent", "DeskKit/1.0")
            .header("Referer", "https://www.yuque.com/")
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status()));
        }
        return resp.bytes().await.map(|b| b.to_vec()).map_err(|e| e.to_string());
    }
    let local = base_dir.join(raw.trim_start_matches("./"));
    if !local.exists() {
        return Err(format!("本地图片不存在: {raw}"));
    }
    fs::read(&local).map_err(|e| e.to_string())
}

fn buffer_to_data_url(buffer: &[u8], source_url: &str) -> String {
    let ext = Path::new(source_url)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png");
    let mime = mime_from_ext(ext);
    format!("data:{mime};base64,{}", STANDARD.encode(buffer))
}

pub async fn embed_images_in_markdown(
    markdown: &str,
    base_dir: &Path,
) -> (String, Vec<String>, Vec<serde_json::Value>) {
    let urls = collect_image_urls(markdown);
    let mut result = markdown.to_string();
    let mut embedded = Vec::new();
    let mut failed = Vec::new();

    for url in urls {
        if url.starts_with("data:") {
            continue;
        }
        match load_image_buffer(&url, base_dir).await {
            Ok(buf) => {
                let data_url = buffer_to_data_url(&buf, &url);
                result = result.replace(&url, &data_url);
                embedded.push(url);
            }
            Err(e) => {
                failed.push(serde_json::json!({ "url": url, "message": e }));
            }
        }
    }
    (result, embedded, failed)
}

pub async fn preview_markdown_file(file_path: &str) -> Result<serde_json::Value, String> {
    let resolved = PathBuf::from(file_path.trim());
    if !resolved.exists() {
        return Err(format!("文件不存在: {}", resolved.display()));
    }
    if !resolved.is_file() {
        return Err(format!("不是文件: {}", resolved.display()));
    }
    if !resolved.to_string_lossy().to_lowercase().ends_with(".md") {
        return Err("仅支持 .md 文件".into());
    }

    let markdown = fs::read_to_string(&resolved).map_err(|e| e.to_string())?;
    let base_name = resolved
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled")
        .to_string();
    let base_dir = resolved.parent().unwrap_or(Path::new("."));

    let (md_with_images, embedded, failed) = embed_images_in_markdown(&markdown, base_dir).await;
    let html = markdown_to_confluence_html(&md_with_images, &base_name, true);
    let body_html = markdown_to_confluence_html(&md_with_images, &base_name, false);

    Ok(serde_json::json!({
        "filePath": resolved.to_string_lossy(),
        "fileName": resolved.file_name().and_then(|n| n.to_str()).unwrap_or(""),
        "title": base_name,
        "charCount": markdown.len(),
        "html": html,
        "bodyHtml": body_html,
        "imagesEmbedded": embedded.len(),
        "imagesFailed": failed,
    }))
}

fn find_markdown_files(root_dir: &Path, recursive: bool) -> Vec<PathBuf> {
    let mut results = Vec::new();
    fn walk(dir: &Path, recursive: bool, results: &mut Vec<PathBuf>) {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for ent in entries.filter_map(|e| e.ok()) {
            let name = ent.file_name().to_string_lossy().into_owned();
            if name == ".DS_Store" || name == "node_modules" || name == ".git" {
                continue;
            }
            let full = ent.path();
            if full.is_dir() && recursive {
                walk(&full, recursive, results);
            } else if full.is_file() && name.to_lowercase().ends_with(".md") {
                results.push(full);
            }
        }
    }
    walk(root_dir, recursive, &mut results);
    results.sort();
    results
}

pub fn list_markdown_files(source_dir: &Path, recursive: bool) -> Vec<ConfluenceFileEntry> {
    find_markdown_files(source_dir, recursive)
        .into_iter()
        .map(|abs| {
            let rel = abs
                .strip_prefix(source_dir)
                .unwrap_or(&abs)
                .to_string_lossy()
                .replace('\\', "/");
            ConfluenceFileEntry {
                absolute_path: abs.to_string_lossy().into_owned(),
                relative_path: rel,
                file_name: abs.file_name().and_then(|n| n.to_str()).unwrap_or("").into(),
            }
        })
        .collect()
}

fn normalize_output_format(format: Option<ConfluenceOutputFormat>) -> Result<ConfluenceOutputFormat, String> {
    match format {
        Some(ConfluenceOutputFormat::Html) => Ok(ConfluenceOutputFormat::Html),
        Some(ConfluenceOutputFormat::Docx) => Ok(ConfluenceOutputFormat::Docx),
        Some(ConfluenceOutputFormat::Md) => Ok(ConfluenceOutputFormat::Md),
        Some(ConfluenceOutputFormat::Pdf) => Ok(ConfluenceOutputFormat::Pdf),
        None => Ok(ConfluenceOutputFormat::Docx),
    }
}

fn output_path_for(
    input_path: &Path,
    source_dir: &Path,
    output_dir: &Path,
    same_dir: bool,
    format: ConfluenceOutputFormat,
) -> PathBuf {
    let ext = match format {
        ConfluenceOutputFormat::Html => ".html",
        ConfluenceOutputFormat::Docx => ".docx",
        ConfluenceOutputFormat::Md => ".md",
        ConfluenceOutputFormat::Pdf => ".pdf",
    };
    let rel = input_path
        .strip_prefix(source_dir)
        .unwrap_or(input_path)
        .to_string_lossy()
        .replace('\\', "/");
    let base = Regex::new(r"\.md$").unwrap().replace(&rel, ext).to_string();
    if same_dir {
        source_dir.join(&base)
    } else {
        output_dir.join(&base)
    }
}

fn markdown_to_docx(markdown: &str, title: &str) -> Result<Vec<u8>, String> {
    let md = preprocess_markdown(markdown);
    let mut docx = Docx::new();
    if !title.is_empty() {
        docx = docx.add_paragraph(
            Paragraph::new().add_run(Run::new().add_text(title).bold()),
        );
    }

    for line in md.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(h) = Regex::new(r"^(#{1,6})\s+(.+)$").unwrap().captures(trimmed) {
            docx = docx.add_paragraph(
                Paragraph::new().add_run(Run::new().add_text(h[2].to_string()).bold()),
            );
            continue;
        }
        if trimmed.starts_with("```") {
            continue;
        }
        docx = docx.add_paragraph(Paragraph::new().add_run(Run::new().add_text(trimmed)));
    }

    let mut buffer = std::io::Cursor::new(Vec::new());
    docx.build()
        .pack(&mut buffer)
        .map_err(|e| e.to_string())?;
    Ok(buffer.into_inner())
}

pub async fn write_converted_file(
    format: &str,
    markdown: &str,
    base_name: &str,
    out_path: &Path,
    image_base_dir: &Path,
) -> Result<(), String> {
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    match format {
        "md" => {
            fs::write(out_path, preprocess_markdown(markdown)).map_err(|e| e.to_string())?;
        }
        "html" => {
            let (md_with_images, _, _) = embed_images_in_markdown(markdown, image_base_dir).await;
            let html = markdown_to_confluence_html(&md_with_images, base_name, true);
            fs::write(out_path, html).map_err(|e| e.to_string())?;
            let docx_path = out_path.with_file_name(format!(
                "{}-confluence.docx",
                out_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(base_name)
            ));
            let docx_bytes = markdown_to_docx(markdown, base_name)?;
            fs::write(&docx_path, docx_bytes).map_err(|e| e.to_string())?;
        }
        "docx" => {
            let bytes = markdown_to_docx(markdown, base_name)?;
            fs::write(out_path, bytes).map_err(|e| e.to_string())?;
        }
        "pdf" => return Err(PDF_ERROR.into()),
        _ => return Err(format!("未知格式: {format}")),
    }
    Ok(())
}

fn resolve_selected_files(
    source_dir: &Path,
    all_files: &[PathBuf],
    files: Option<&[String]>,
) -> Result<Vec<PathBuf>, String> {
    let Some(list) = files else {
        return Ok(all_files.to_vec());
    };
    if list.is_empty() {
        return Ok(all_files.to_vec());
    }
    let all_set: std::collections::HashSet<_> = all_files.iter().map(|p| p.canonicalize().unwrap_or(p.clone())).collect();
    let mut selected = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for item in list {
        let raw = item.trim();
        if raw.is_empty() {
            continue;
        }
        let resolved = if Path::new(raw).is_absolute() {
            PathBuf::from(raw)
        } else {
            source_dir.join(raw)
        };
        let canon = resolved.canonicalize().unwrap_or(resolved);
        if !all_set.contains(&canon) {
            return Err(format!("所选文件不在源目录内或不是 .md: {raw}"));
        }
        if seen.insert(canon.clone()) {
            selected.push(canon);
        }
    }
    selected.sort();
    Ok(selected)
}

pub async fn batch_convert_markdown(params: ConfluenceConvertParams) -> Result<serde_json::Value, String> {
    let output_format = normalize_output_format(params.format)?;
    if output_format == ConfluenceOutputFormat::Pdf {
        return Err(PDF_ERROR.into());
    }

    let source_dir = resolve_safe_dir(&params.source_dir).map_err(|e| e.to_string())?;
    let same_dir = params.same_dir;
    let output_dir = if same_dir {
        source_dir.clone()
    } else {
        resolve_safe_dir(params.output_dir.as_deref().unwrap_or("")).map_err(|e| e.to_string())?
    };

    let all_md = find_markdown_files(&source_dir, params.recursive);
    let md_files = resolve_selected_files(&source_dir, &all_md, params.files.as_deref())?;
    if md_files.is_empty() {
        return Err("请至少选择一个 Markdown 文件".into());
    }

    let mut converted = Vec::new();
    let mut skipped = Vec::new();
    let mut failed = Vec::new();

    for input_path in &md_files {
        let out_path = output_path_for(
            input_path,
            &source_dir,
            &output_dir,
            same_dir,
            output_format,
        );
        let rel_path = input_path
            .strip_prefix(&source_dir)
            .unwrap_or(input_path)
            .to_string_lossy()
            .replace('\\', "/");

        if output_format == ConfluenceOutputFormat::Md
            && out_path.canonicalize().ok() == input_path.canonicalize().ok()
            && !params.overwrite
        {
            skipped.push(serde_json::json!({
                "relativePath": rel_path,
                "outputPath": out_path.to_string_lossy(),
                "reason": "same-md"
            }));
            continue;
        }

        if out_path.exists() && !params.overwrite {
            skipped.push(serde_json::json!({
                "relativePath": rel_path,
                "outputPath": out_path.to_string_lossy(),
            }));
            continue;
        }

        match fs::read_to_string(input_path) {
            Ok(markdown) => {
                let base_name = input_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("untitled");
                let fmt_str = match output_format {
                    ConfluenceOutputFormat::Html => "html",
                    ConfluenceOutputFormat::Docx => "docx",
                    ConfluenceOutputFormat::Md => "md",
                    ConfluenceOutputFormat::Pdf => "pdf",
                };
                match write_converted_file(
                    fmt_str,
                    &markdown,
                    base_name,
                    &out_path,
                    input_path.parent().unwrap_or(&source_dir),
                )
                .await
                {
                    Ok(()) => converted.push(serde_json::json!({
                        "relativePath": rel_path,
                        "outputPath": out_path.to_string_lossy(),
                        "title": base_name,
                    })),
                    Err(e) => failed.push(serde_json::json!({
                        "relativePath": rel_path,
                        "message": e,
                    })),
                }
            }
            Err(e) => failed.push(serde_json::json!({
                "relativePath": rel_path,
                "message": e.to_string(),
            })),
        }
    }

    Ok(serde_json::json!({
        "sourceDir": source_dir.to_string_lossy(),
        "outputDir": output_dir.to_string_lossy(),
        "outputFormat": match output_format {
            ConfluenceOutputFormat::Html => "html",
            ConfluenceOutputFormat::Docx => "docx",
            ConfluenceOutputFormat::Md => "md",
            ConfluenceOutputFormat::Pdf => "pdf",
        },
        "total": md_files.len(),
        "selectedCount": md_files.len(),
        "allCount": all_md.len(),
        "convertedCount": converted.len(),
        "skippedCount": skipped.len(),
        "failedCount": failed.len(),
        "converted": converted,
        "skipped": skipped,
        "failed": failed,
    }))
}

pub fn confluence_list(source_dir: &str, recursive: bool) -> Result<serde_json::Value, String> {
    let dir = resolve_safe_dir(source_dir).map_err(|e| e.to_string())?;
    let files = list_markdown_files(&dir, recursive);
    Ok(serde_json::json!({
        "files": files,
        "count": files.len(),
    }))
}
