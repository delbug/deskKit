use crate::confluence;
use crate::fs_util::{safe_file_name, unique_dir_path, unique_file_path};
use crate::models::{
    YuqueBatchParams, YuqueDocManifestItem, YuqueDocPreview, YuqueExportFormat, YuqueExportParams,
    YuqueFailedItem, YuqueProgressState,
};
use chrono::Utc;
use once_cell::sync::Lazy;
use rand::Rng;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use url::Url;

static ACTIVE_PROGRESS: Lazy<Mutex<HashMap<String, YuqueProgressState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

static EXPORT_CANCEL: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const API_UA: &str = "deskit-yuque-exporter";

fn client() -> Client {
    Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap_or_else(|_| Client::new())
}

#[derive(Debug, Clone)]
pub struct ParsedYuqueUrl {
    pub link_type: String,
    pub share_id: Option<String>,
    pub path_prefix: Option<String>,
    pub book_slug: Option<String>,
    pub doc_slug: Option<String>,
    pub token: Option<String>,
    pub token_suffix: String,
    pub token_md: String,
    pub book_page_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YuqueDocPlan {
    pub title: String,
    pub slug: String,
    pub dir_segments: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct YuqueBook {
    pub name: String,
    pub docs: Vec<YuqueDocPlan>,
}

pub fn prepare_batch_docs(
    docs: &[YuqueDocPlan],
    export_order: &str,
    selected_slugs: Option<&[String]>,
) -> Result<Vec<YuqueDocPlan>, String> {
    let mut list: Vec<YuqueDocPlan> = if export_order == "custom" {
        let slugs = selected_slugs.ok_or_else(|| "自定义导出请至少勾选一篇文档".to_string())?;
        if slugs.is_empty() {
            return Err("自定义导出请至少勾选一篇文档".into());
        }
        let set: HashSet<_> = slugs.iter().cloned().collect();
        docs.iter().filter(|d| set.contains(&d.slug)).cloned().collect()
    } else {
        docs.to_vec()
    };

    if list.is_empty() {
        return Err("没有可导出的文档".into());
    }

    if export_order == "bottom-up" {
        list.reverse();
    }

    Ok(list)
}

pub fn import_yuque_progress(save_dir: &str, progress: YuqueProgressState) -> Result<(), String> {
    if progress.url.trim().is_empty() {
        return Err("快照缺少语雀链接".into());
    }
    let url = progress.url.clone();
    set_active_progress(save_dir, &url, progress);
    Ok(())
}

pub fn normalize_export_format(
    format: Option<YuqueExportFormat>,
    legacy_confluence_html: bool,
) -> YuqueExportFormat {
    match format {
        Some(YuqueExportFormat::Html) => YuqueExportFormat::Html,
        Some(YuqueExportFormat::Both) => YuqueExportFormat::Both,
        Some(YuqueExportFormat::Md) => YuqueExportFormat::Md,
        None if legacy_confluence_html => YuqueExportFormat::Both,
        None => YuqueExportFormat::Md,
    }
}

fn normalize_url_input(input: &str) -> String {
    let mut text = input.trim().to_string();
    if text.is_empty() {
        return text;
    }
    let re = Regex::new(r#"https?://[^\s<>"']*yuque\.com[^\s<>"']*"#).unwrap();
    if let Some(caps) = re.find(&text) {
        text = caps.as_str().trim_end_matches([')', ']', '}', '>', '，', '。', '；', '、']).to_string();
    } else if text.contains("yuque.com") && !text.starts_with("http") {
        text = format!("https://{}", text.trim_start_matches('/'));
    }
    text
}

pub fn parse_yuque_url(input: &str) -> Result<ParsedYuqueUrl, String> {
    let raw = normalize_url_input(input);
    if raw.is_empty() {
        return Err("链接不能为空".into());
    }
    let url_str = if raw.starts_with("http") {
        raw
    } else {
        format!("https://{raw}")
    };
    let url = Url::parse(&url_str).map_err(|_| "链接格式无效".to_string())?;
    if !url.host_str().unwrap_or("").contains("yuque.com") {
        return Err("仅支持语雀 (yuque.com) 链接".into());
    }

    let parts: Vec<&str> = url
        .path()
        .trim_matches('/')
        .split('/')
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        return Err("链接格式无效".into());
    }

    let token = url
        .query_pairs()
        .find(|(k, _)| k == "token" || k == "share_token")
        .map(|(_, v)| v.to_string());
    let token_suffix = token
        .as_ref()
        .map(|t| format!("?token={}", urlencoding_simple(t)))
        .unwrap_or_default();
    let token_md = token
        .as_ref()
        .map(|t| format!("&token={}", urlencoding_simple(t)))
        .unwrap_or_default();

    if parts.len() >= 3 && parts[0] == "docs" && parts[1] == "share" {
        return Ok(ParsedYuqueUrl {
            link_type: "share".into(),
            share_id: Some(parts[2].to_string()),
            path_prefix: None,
            book_slug: None,
            doc_slug: None,
            token,
            token_suffix: token_suffix.clone(),
            token_md,
            book_page_url: format!("https://www.yuque.com/docs/share/{}{token_suffix}", parts[2]),
        });
    }

    let doc_slug = if parts.len() >= 3 {
        Some(parts[parts.len() - 1].to_string())
    } else {
        None
    };
    let path_prefix = if doc_slug.is_some() {
        Some(parts[..parts.len() - 1].join("/"))
    } else {
        Some(parts.join("/"))
    };
    let book_slug = if doc_slug.is_some() {
        Some(parts[parts.len() - 2].to_string())
    } else {
        Some(parts[parts.len() - 1].to_string())
    };

    Ok(ParsedYuqueUrl {
        link_type: if doc_slug.is_some() { "doc".into() } else { "book".into() },
        share_id: None,
        path_prefix: path_prefix.clone(),
        book_slug,
        doc_slug,
        token,
        token_suffix: token_suffix.clone(),
        token_md,
        book_page_url: format!(
            "https://www.yuque.com/{}{token_suffix}",
            path_prefix.unwrap_or_default()
        ),
    })
}

fn urlencoding_simple(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

fn build_doc_urls(parsed: &ParsedYuqueUrl, doc_slug: &str) -> (String, String) {
    let base = format!(
        "https://www.yuque.com/{}/{}",
        parsed.path_prefix.as_deref().unwrap_or(""),
        doc_slug
    );
    (
        format!("{}{}", base, parsed.token_suffix),
        format!(
            "{}/markdown?plain=true&linebreak=false&anchor=false{}",
            base, parsed.token_md
        ),
    )
}

fn extract_app_data(html: &str) -> Option<Value> {
    let patterns = [
        Regex::new(r#"window\.appData = JSON\.parse\(decodeURIComponent\("([^"]+)"\)\)"#).ok(),
        Regex::new(r#"window\.appData = JSON\.parse\(decodeURIComponent\('([^']+)'\)\)"#).ok(),
    ];
    for re in patterns.into_iter().flatten() {
        if let Some(caps) = re.captures(html) {
            if let Some(encoded) = caps.get(1) {
                let decoded = percent_encoding::percent_decode_str(encoded.as_str())
                    .decode_utf8()
                    .ok()?;
                if let Ok(val) = serde_json::from_str::<Value>(&decoded) {
                    return Some(val);
                }
            }
        }
    }
    None
}

async fn http_get(url: &str) -> Result<String, String> {
    let resp = client()
        .get(url)
        .header("Accept", "text/html,text/plain,*/*")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("请求失败 HTTP {}", resp.status()));
    }
    resp.text().await.map_err(|e| e.to_string())
}

async fn http_get_bytes(url: &str) -> Result<Vec<u8>, String> {
    let resp = client().get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("下载失败 HTTP {}", resp.status()));
    }
    resp.bytes().await.map(|b| b.to_vec()).map_err(|e| e.to_string())
}

pub fn build_export_plan(toc: &[Value]) -> Vec<YuqueDocPlan> {
    let mut title_path_by_uuid: HashMap<String, Vec<String>> = HashMap::new();
    let mut plan = Vec::new();

    for item in toc {
        let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let uuid = item.get("uuid").and_then(|v| v.as_str()).unwrap_or("");
        if item_type == "TITLE" {
            let parent_segments = item
                .get("parent_uuid")
                .and_then(|v| v.as_str())
                .and_then(|pid| title_path_by_uuid.get(pid).cloned())
                .unwrap_or_default();
            let title = item
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("未命名分组");
            let mut segments = parent_segments;
            segments.push(safe_file_name(title));
            title_path_by_uuid.insert(uuid.to_string(), segments);
        } else if item_type == "DOC" {
            if let Some(slug) = item.get("url").and_then(|v| v.as_str()) {
                let dir_segments = item
                    .get("parent_uuid")
                    .and_then(|v| v.as_str())
                    .and_then(|pid| title_path_by_uuid.get(pid).cloned())
                    .unwrap_or_default();
                let title = item
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or(slug);
                plan.push(YuqueDocPlan {
                    title: title.to_string(),
                    slug: slug.to_string(),
                    dir_segments,
                });
            }
        }
    }
    plan
}

fn book_from_app_data(app_data: &Value) -> Option<YuqueBook> {
    let toc = app_data.get("book")?.get("toc")?.as_array()?;
    if toc.is_empty() {
        return None;
    }
    let name = app_data
        .get("book")
        .and_then(|b| b.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("知识库");
    Some(YuqueBook {
        name: name.to_string(),
        docs: build_export_plan(toc),
    })
}

fn describe_fetch_failure(app_data: Option<&Value>, html: &str) -> String {
    if app_data.is_none() {
        if html.contains("登录") || html.to_lowercase().contains("login") || html.contains("密码") {
            return "需要登录或访问密码".into();
        }
        return "页面未返回有效数据".into();
    }
    let data = app_data.unwrap();
    if data
        .get("matchCondition")
        .and_then(|m| m.get("page"))
        .and_then(|v| v.as_str())
        == Some("404")
    {
        return "页面不存在或未开启分享".into();
    }
    if data.get("book").is_some() {
        let toc_len = data
            .get("book")
            .and_then(|b| b.get("toc"))
            .and_then(|t| t.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        if toc_len == 0 {
            return "分享页未包含知识库目录".into();
        }
        let has_doc = data
            .get("book")
            .and_then(|b| b.get("toc"))
            .and_then(|t| t.as_array())
            .map(|arr| arr.iter().any(|i| i.get("type").and_then(|v| v.as_str()) == Some("DOC")))
            .unwrap_or(false);
        if !has_doc {
            return "知识库目录中没有可导出的文档".into();
        }
    }
    "未能解析知识库目录".into()
}

fn enrich_parsed_from_app_data(parsed: &mut ParsedYuqueUrl, app_data: &Value) {
    if let Some(slug) = app_data.get("doc").and_then(|d| d.get("slug")).and_then(|v| v.as_str()) {
        parsed.doc_slug = Some(slug.to_string());
        parsed.link_type = "doc".into();
    }
    if parsed.path_prefix.is_none() {
        if let Some(book_slug) = app_data.get("book").and_then(|b| b.get("slug")).and_then(|v| v.as_str()) {
            let login = app_data
                .get("group")
                .and_then(|g| g.get("login"))
                .and_then(|v| v.as_str())
                .or_else(|| {
                    app_data
                        .get("book")
                        .and_then(|b| b.get("user"))
                        .and_then(|u| u.get("login"))
                        .and_then(|v| v.as_str())
                });
            if let Some(login) = login {
                parsed.path_prefix = Some(format!("{login}/{book_slug}"));
                parsed.book_slug = Some(book_slug.to_string());
                parsed.book_page_url = format!(
                    "https://www.yuque.com/{login}/{book_slug}{}",
                    parsed.token_suffix
                );
            }
        }
    }
}

async fn fetch_book_from_html_candidates(mut parsed: ParsedYuqueUrl) -> Result<(ParsedYuqueUrl, YuqueBook), String> {
    let mut candidates = Vec::new();

    if parsed.link_type == "share" {
        if let Some(ref sid) = parsed.share_id {
            candidates.push(format!("https://www.yuque.com/docs/share/{sid}{}", parsed.token_suffix));
        }
    }

    if let (Some(ref prefix), Some(ref doc)) = (&parsed.path_prefix, &parsed.doc_slug) {
        let base = format!("https://www.yuque.com/{prefix}/{doc}");
        candidates.push(format!("{}{}", base, parsed.token_suffix));
        if let Some(ref token) = parsed.token {
            candidates.push(format!("{base}?singleDoc&token={}", urlencoding_simple(token)));
        } else {
            candidates.push(format!("{base}?singleDoc"));
        }
        candidates.push(base);
    }

    if parsed.link_type == "book" {
        candidates.push(parsed.book_page_url.clone());
        let sep = if parsed.book_page_url.contains('?') { '&' } else { '?' };
        candidates.push(format!("{}{sep}singleDoc", parsed.book_page_url));
    }

    let mut tried = std::collections::HashSet::new();
    let mut last_reason = String::new();

    for page_url in candidates {
        if tried.contains(&page_url) {
            continue;
        }
        tried.insert(page_url.clone());

        let html = match http_get(&page_url).await {
            Ok(h) => h,
            Err(e) => {
                last_reason = format!("请求失败: {e}");
                continue;
            }
        };

        let app_data = extract_app_data(&html);
        if book_from_app_data(app_data.as_ref().unwrap_or(&Value::Null)).is_none() {
            last_reason = describe_fetch_failure(app_data.as_ref(), &html);
        }

        if let Some(ref data) = app_data {
            enrich_parsed_from_app_data(&mut parsed, data);
            if let Some(book) = book_from_app_data(data) {
                if !book.docs.is_empty() {
                    return Ok((parsed, book));
                }
            }

            if let Some(doc_slug) = data.get("doc").and_then(|d| d.get("slug")).and_then(|v| v.as_str()) {
                if parsed.path_prefix.is_some() {
                    let (page, _) = build_doc_urls(&parsed, doc_slug);
                    if let Ok(doc_html) = http_get(&page).await {
                        let doc_app = extract_app_data(&doc_html);
                        if let Some(ref ddata) = doc_app {
                            enrich_parsed_from_app_data(&mut parsed, ddata);
                            if let Some(book) = book_from_app_data(ddata) {
                                if !book.docs.is_empty() {
                                    return Ok((parsed, book));
                                }
                            }
                            last_reason = describe_fetch_failure(doc_app.as_ref(), &doc_html);
                        }
                    }
                }
            }
        }
    }

    let link_hint = if let Some(ref doc) = parsed.doc_slug {
        format!("已识别：/{}/{}", parsed.path_prefix.as_deref().unwrap_or(""), doc)
    } else if let Some(ref prefix) = parsed.path_prefix {
        format!("已识别：/{prefix}（缺少文档段）")
    } else if let Some(ref sid) = parsed.share_id {
        format!("已识别：/docs/share/{sid}")
    } else {
        "未能识别链接结构".into()
    };

    Err(format!(
        "无法读取知识库目录（{}）。\n\n{link_hint}\n\n分享链接模式需粘贴知识库内任意一篇文档的链接，格式类似：\nhttps://www.yuque.com/用户/知识库/文档slug?singleDoc\n\n若只有「用户/知识库」链接，请切换到「API Token」模式，填写 Token 后可直接用知识库链接批量导出。",
        if last_reason.is_empty() { "未知原因" } else { &last_reason }
    ))
}

fn extract_title_from_html(html: &str) -> String {
    if let Some(data) = extract_app_data(html) {
        if let Some(title) = data.get("doc").and_then(|d| d.get("title")).and_then(|v| v.as_str()) {
            return title.to_string();
        }
    }
    let re = Regex::new(r#"<meta property="og:title" content="([^"]+)""#).unwrap();
    if let Some(caps) = re.captures(html) {
        return caps
            .get(1)
            .map(|m| m.as_str().trim_end_matches("· 语雀").trim().to_string())
            .unwrap_or_else(|| "untitled".into());
    }
    "untitled".into()
}

pub fn images_from_markdown(md: &str) -> Vec<String> {
    let re = Regex::new(r#"!\[[^\]]*\]\((https?://[^)\s]+)\)"#).unwrap();
    re.captures_iter(md)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

pub fn normalize_yuque_markdown(raw: &str, standard: bool) -> String {
    if !standard {
        return raw.to_string();
    }
    let mut md = raw.to_string();
    md = md.replace("<br/>", "\n").replace("<br />", "\n").replace("<br>", "\n");
    md = Regex::new(r"<\/?(span|div|p|u|sub|sup|section|article)[^>]*>")
        .unwrap()
        .replace_all(&md, "")
        .to_string();
    md = md.replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&");

    let font_re = Regex::new(r"(?i)<font[^>]*>([\s\S]*?)<\/font>").unwrap();
    loop {
        let next = font_re.replace_all(&md, "$1").to_string();
        if next == md {
            break;
        }
        md = next;
    }

    md = Regex::new(r"\*{4,}").unwrap().replace_all(&md, "").to_string();
    md = md.replace("** **", " ");

    md = Regex::new(r"^(#{1,6})\s+\*\*(.+)\*\*\s*$")
        .unwrap()
        .replace_all(&md, |caps: &regex::Captures| {
            let hashes = &caps[1];
            let inner = caps[2].replace("**", "").split_whitespace().collect::<Vec<_>>().join(" ");
            format!("{hashes} {inner}")
        })
        .to_string();

    if let Ok(re) = Regex::new(r"^# ([^\n]+)\n(# .+)") {
        md = re
            .replace(&md, "> $1\n\n$2")
            .to_string();
    }

    md = md.trim().to_string();
    format!("{md}\n")
}

async fn fetch_yuque_markdown(markdown_url: &str) -> Result<String, String> {
    let markdown = http_get(markdown_url).await?;
    let trimmed = markdown.trim_start();
    if trimmed.starts_with("<!doctype") || trimmed.starts_with("<!DOCTYPE") {
        return Err("无法获取 Markdown，文档可能未公开或需要 token".into());
    }
    Ok(markdown)
}

pub async fn fetch_yuque_doc(url: &str) -> Result<(String, String, Vec<String>, ParsedYuqueUrl), String> {
    let parsed = parse_yuque_url(url)?;
    let doc_slug = parsed
        .doc_slug
        .clone()
        .ok_or_else(|| "单篇导出需要文档级链接，格式：yuque.com/用户/知识库/文档".to_string())?;
    let _prefix = parsed
        .path_prefix
        .clone()
        .ok_or_else(|| "链接格式无效".to_string())?;
    let (page_url, markdown_url) = build_doc_urls(&parsed, &doc_slug);
    let markdown = fetch_yuque_markdown(&markdown_url).await?;
    let html = http_get(&page_url).await?;
    let title = extract_title_from_html(&html);
    let images = images_from_markdown(&markdown);
    Ok((title, markdown, images, parsed))
}

pub async fn fetch_yuque_book(url: &str) -> Result<(ParsedYuqueUrl, YuqueBook), String> {
    let parsed = parse_yuque_url(url)?;
    fetch_book_from_html_candidates(parsed).await
}

async fn download_images(markdown: &str, save_dir: &Path) -> Result<(String, usize), String> {
    let assets_dir = save_dir.join("assets");
    fs::create_dir_all(&assets_dir).map_err(|e| e.to_string())?;
    let mut result = markdown.to_string();
    let mut downloaded = 0;
    let urls: std::collections::HashSet<_> = images_from_markdown(markdown).into_iter().collect();

    for img_url in urls {
        match http_get_bytes(&img_url).await {
            Ok(buf) => {
                let parsed_url = Url::parse(&img_url).ok();
                let mut ext = parsed_url
                    .as_ref()
                    .and_then(|u| Path::new(u.path()).extension())
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                if ext.is_empty() || ext.len() > 6 {
                    ext = "png";
                }
                let hash = format!("{:x}", md5::compute(img_url.as_bytes()));
                let file_name = format!("{}_{}.{}", &hash[..8], downloaded, ext);
                let file_path = assets_dir.join(&file_name);
                fs::write(&file_path, &buf).map_err(|e| e.to_string())?;
                let local_ref = format!("assets/{file_name}");
                result = result.replace(&img_url, &local_ref);
                downloaded += 1;
            }
            Err(_) => {}
        }
    }
    Ok((result, downloaded))
}

pub async fn save_yuque_doc_content(
    title: &str,
    markdown: &str,
    images: &[String],
    save_dir: &Path,
    download_images_flag: bool,
    standard_markdown: bool,
    use_doc_folder: bool,
    export_format: YuqueExportFormat,
) -> Result<serde_json::Value, String> {
    let doc_dir = if use_doc_folder {
        unique_dir_path(save_dir, &safe_file_name(title))
    } else {
        save_dir.to_path_buf()
    };
    fs::create_dir_all(&doc_dir).map_err(|e| e.to_string())?;

    let mut content = normalize_yuque_markdown(markdown, standard_markdown);
    let mut downloaded_images = 0usize;
    if download_images_flag && !images.is_empty() {
        let (md, count) = download_images(&content, &doc_dir).await?;
        content = md;
        downloaded_images = count;
    }

    let base_name = safe_file_name(title);
    let mut file_path: Option<PathBuf> = None;
    let mut html_path: Option<PathBuf> = None;

    if matches!(export_format, YuqueExportFormat::Md | YuqueExportFormat::Both) {
        let md_path = unique_file_path(&doc_dir, &base_name, ".md");
        fs::write(&md_path, &content).map_err(|e| e.to_string())?;
        file_path = Some(md_path);
    }

    if matches!(export_format, YuqueExportFormat::Html | YuqueExportFormat::Both) {
        let html_out = if let Some(ref md) = file_path {
            md.with_extension("html")
        } else {
            unique_file_path(&doc_dir, &base_name, ".html")
        };
        confluence::write_converted_file("html", &content, title, &html_out, &doc_dir).await?;
        html_path = Some(html_out);
    }

    let primary_path = html_path.clone().or(file_path.clone()).unwrap();

    Ok(serde_json::json!({
        "title": title,
        "fileName": primary_path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
        "filePath": primary_path.to_string_lossy(),
        "mdPath": file_path.as_ref().map(|p| p.to_string_lossy().to_string()),
        "mdFileName": file_path.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str()),
        "htmlPath": html_path.as_ref().map(|p| p.to_string_lossy().to_string()),
        "htmlFileName": html_path.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str()),
        "exportFormat": match export_format {
            YuqueExportFormat::Md => "md",
            YuqueExportFormat::Html => "html",
            YuqueExportFormat::Both => "both",
        },
        "folderPath": if use_doc_folder { Some(doc_dir.to_string_lossy().to_string()) } else { None::<String> },
        "imageCount": images.len(),
        "downloadedImages": if download_images_flag { downloaded_images } else { 0 },
        "charCount": content.len(),
    }))
}

pub async fn export_yuque_doc(params: YuqueExportParams) -> Result<serde_json::Value, String> {
    let save_dir = PathBuf::from(params.save_dir.trim());
    if !save_dir.exists() {
        return Err(format!("保存目录不存在: {}", save_dir.display()));
    }
    if !save_dir.is_dir() {
        return Err(format!("不是文件夹: {}", save_dir.display()));
    }

    let (title, markdown, images, _) = fetch_yuque_doc(&params.url).await?;
    let format = normalize_export_format(params.export_format, params.export_confluence_html);
    save_yuque_doc_content(
        &title,
        &markdown,
        &images,
        &save_dir,
        params.download_images,
        params.standard_markdown,
        params.use_doc_folder,
        format,
    )
    .await
}

pub async fn preview_yuque(url: &str, standard_markdown: bool) -> Result<serde_json::Value, String> {
    let (title, markdown, images, _) = fetch_yuque_doc(url).await?;
    let md = normalize_yuque_markdown(&markdown, standard_markdown);
    let preview: String = md.chars().take(3000).collect();
    Ok(serde_json::json!({
        "title": title,
        "preview": preview,
        "imageCount": images.len(),
        "charCount": md.len(),
    }))
}

pub async fn preview_yuque_book(url: &str, token: Option<String>) -> Result<serde_json::Value, String> {
    let (auth_mode, book) = if let Some(ref t) = token {
        if t.trim().is_empty() {
            let (_, book) = fetch_yuque_book(url).await?;
            ("share", book)
        } else {
            let (_, book) = fetch_yuque_book_by_api(t, url).await?;
            ("token", book)
        }
    } else {
        let (_, book) = fetch_yuque_book(url).await?;
        ("share", book)
    };

    Ok(serde_json::json!({
        "authMode": auth_mode,
        "bookName": book.name,
        "total": book.docs.len(),
        "docs": book.docs.iter().map(|d| YuqueDocPreview {
            title: d.title.clone(),
            slug: d.slug.clone(),
            dir_path: if d.dir_segments.is_empty() {
                "(根目录)".into()
            } else {
                d.dir_segments.join("/")
            },
        }).collect::<Vec<_>>(),
    }))
}

async fn api_request_once(token: &str, api_path: &str, query: &[(&str, String)]) -> Result<Value, String> {
    let mut url = format!("https://www.yuque.com/api/v2{api_path}");
    if !query.is_empty() {
        url.push('?');
        url.push_str(
            &query
                .iter()
                .map(|(k, v)| format!("{k}={}", urlencoding_simple(v)))
                .collect::<Vec<_>>()
                .join("&"),
        );
    }

    let resp = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap_or_else(|_| Client::new())
        .get(&url)
        .header("User-Agent", API_UA)
        .header("Accept", "application/json")
        .header("X-Auth-Token", token)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    let json: Value = serde_json::from_str(&body).map_err(|_| format!("API 响应非 JSON (HTTP {status})"))?;

    if status.as_u16() == 401 || json.get("status").and_then(|v| v.as_u64()) == Some(401) {
        return Err("Token 无效或已过期，请在语雀「设置 → Token」重新生成".into());
    }
    if status.as_u16() == 403 || json.get("status").and_then(|v| v.as_u64()) == Some(403) {
        return Err("Token 无权访问该知识库".into());
    }
    if status.as_u16() == 429
        || json.get("status").and_then(|v| v.as_u64()) == Some(429)
        || json
            .get("message")
            .and_then(|v| v.as_str())
            .map(|m| m.to_lowercase().contains("too many"))
            .unwrap_or(false)
    {
        return Err("RATE_LIMIT".into());
    }
    if status.as_u16() >= 400
        || json.get("status").and_then(|v| v.as_u64()).unwrap_or(0) >= 400
    {
        return Err(json
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or(&format!("API 错误 HTTP {status}"))
            .to_string());
    }

    Ok(json.get("data").cloned().unwrap_or(json))
}

async fn api_request(token: &str, api_path: &str, query: &[(&str, String)], retry: u32) -> Result<Value, String> {
    match api_request_once(token, api_path, query).await {
        Ok(v) => Ok(v),
        Err(e) if e == "RATE_LIMIT" && retry < 5 => {
            let wait = std::cmp::min(90, 8 * (retry + 1));
            tokio::time::sleep(Duration::from_secs(wait as u64)).await;
            Box::pin(api_request(token, api_path, query, retry + 1)).await
        }
        Err(e) if e == "RATE_LIMIT" => {
            Err("语雀 API 请求过于频繁 (Too Many Requests)，请等待 5~10 分钟后再试".into())
        }
        Err(e) => Err(e),
    }
}

pub async fn fetch_yuque_book_by_api(token: &str, url_input: &str) -> Result<(ParsedYuqueUrl, YuqueBook), String> {
    let mut parsed = parse_yuque_url(url_input)?;
    let namespace = parsed
        .path_prefix
        .clone()
        .ok_or_else(|| "无法识别知识库路径，请粘贴形如 yuque.com/用户/知识库 的链接".to_string())?;
    let ns_path: String = namespace
        .split('/')
        .map(|s| urlencoding_simple(s))
        .collect::<Vec<_>>()
        .join("/");

    let toc = api_request(token, &format!("/repos/{ns_path}/toc"), &[], 0).await?;
    let book_info = api_request(token, &format!("/repos/{ns_path}"), &[], 0)
        .await
        .ok();

    let toc_arr = toc.as_array().cloned().unwrap_or_default();
    let plan = build_export_plan(&toc_arr);
    if plan.is_empty() {
        return Err("API 返回的知识库目录为空，请确认 Token 有该知识库权限".into());
    }

    parsed.path_prefix = Some(namespace.clone());
    let name = book_info
        .as_ref()
        .and_then(|b| b.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or(namespace.split('/').last().unwrap_or("知识库"));

    Ok((
        parsed,
        YuqueBook {
            name: name.to_string(),
            docs: plan,
        },
    ))
}

async fn fetch_doc_markdown_by_api(token: &str, namespace: &str, slug: &str) -> Result<String, String> {
    let ns_path: String = namespace
        .split('/')
        .map(|s| urlencoding_simple(s))
        .collect::<Vec<_>>()
        .join("/");
    let data = api_request(
        token,
        &format!("/repos/{ns_path}/docs/{}", urlencoding_simple(slug)),
        &[("raw", "1".into())],
        0,
    )
    .await?;
    if let Some(s) = data.as_str() {
        return Ok(s.to_string());
    }
    if let Some(body) = data.get("body").and_then(|v| v.as_str()) {
        return Ok(body.to_string());
    }
    if let Some(content) = data.get("content").and_then(|v| v.as_str()) {
        return Ok(content.to_string());
    }
    Err(format!("无法获取文档正文: {slug}"))
}

fn normalize_url_key(input: &str) -> String {
    let raw = input.trim();
    if raw.is_empty() {
        return String::new();
    }
    let url_input = if raw.starts_with("http") {
        raw.to_string()
    } else {
        format!("https://{raw}")
    };
    if let Ok(u) = Url::parse(&url_input) {
        let parts: Vec<&str> = u.path().split('/').filter(|p| !p.is_empty()).collect();
        if parts.len() >= 3 && parts[0] == "docs" && parts[1] == "share" {
            return format!("share:{}", parts[2]);
        }
        if parts.len() >= 2 {
            return format!("book:{}/{}", parts[0], parts[1]);
        }
        return u.path().trim_end_matches('/').to_lowercase();
    }
    raw.to_lowercase()
}

fn progress_store_key(save_dir: &str, url: &str) -> String {
    format!(
        "{}|{}",
        PathBuf::from(save_dir.trim())
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(save_dir.trim()))
            .to_string_lossy(),
        normalize_url_key(url)
    )
}

fn progress_file_path(save_dir: &Path, url: &str) -> PathBuf {
    let key = normalize_url_key(url);
    let safe: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    save_dir.join(format!(".deskit-yuque-{safe}.json"))
}

fn save_progress_file(save_dir: &Path, url: &str, state: &YuqueProgressState) {
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let path = progress_file_path(save_dir, url);
        let _ = fs::write(path, json);
    }
}

fn load_progress_file(save_dir: &Path, url: &str) -> Option<YuqueProgressState> {
    let path = progress_file_path(save_dir, url);
    let raw = fs::read_to_string(path).ok()?;
    let state: YuqueProgressState = serde_json::from_str(&raw).ok()?;
    if normalize_url_key(&state.url) != normalize_url_key(url) {
        return None;
    }
    Some(state)
}

fn pick_latest_progress(candidates: Vec<YuqueProgressState>) -> Option<YuqueProgressState> {
    candidates.into_iter().max_by(|a, b| {
        a.updated_at
            .as_deref()
            .unwrap_or("")
            .cmp(b.updated_at.as_deref().unwrap_or(""))
    })
}

fn request_export_cancel(save_dir: &str, url: &str) {
    let key = progress_store_key(save_dir, url);
    if let Ok(mut set) = EXPORT_CANCEL.lock() {
        set.insert(key);
    }
}

fn clear_export_cancel(save_dir: &str, url: &str) {
    let key = progress_store_key(save_dir, url);
    if let Ok(mut set) = EXPORT_CANCEL.lock() {
        set.remove(&key);
    }
}

fn is_export_cancel_requested(save_dir: &str, url: &str) -> bool {
    let key = progress_store_key(save_dir, url);
    EXPORT_CANCEL
        .lock()
        .ok()
        .map(|set| set.contains(&key))
        .unwrap_or(false)
}

pub fn cancel_yuque_export(save_dir: &str, url: &str) {
    request_export_cancel(save_dir, url);
}

pub fn reset_yuque_export(save_dir: &str, url: &str) {
    let key = progress_store_key(save_dir, url);
    if let Ok(mut map) = ACTIVE_PROGRESS.lock() {
        map.remove(&key);
    }
    clear_export_cancel(save_dir, url);
    let path = progress_file_path(Path::new(save_dir), url);
    let _ = fs::remove_file(path);
}

async fn sleep_interruptible(save_dir: &str, url: &str, ms: u64) -> bool {
    if ms == 0 {
        return is_export_cancel_requested(save_dir, url);
    }
    let mut remaining = ms;
    while remaining > 0 {
        if is_export_cancel_requested(save_dir, url) {
            return true;
        }
        let chunk = remaining.min(400);
        tokio::time::sleep(Duration::from_millis(chunk)).await;
        remaining = remaining.saturating_sub(chunk);
    }
    is_export_cancel_requested(save_dir, url)
}

fn batch_export_result(
    book: &YuqueBook,
    book_dir: &Path,
    progress: YuqueProgressState,
    resumed: bool,
    delay_mode: &str,
    exported_total: usize,
    newly_exported: usize,
    skipped_count: usize,
    stopped_early: bool,
    paused: bool,
    success: Vec<serde_json::Value>,
    failed: Vec<serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "bookName": book.name,
        "bookDir": book_dir.to_string_lossy(),
        "total": book.docs.len(),
        "exported": exported_total,
        "newlyExported": newly_exported,
        "skippedCount": skipped_count,
        "remainingCount": book.docs.len().saturating_sub(exported_total),
        "failedCount": progress.failed.as_ref().map(|f| f.len()).unwrap_or(0),
        "resume": resumed,
        "stoppedEarly": stopped_early,
        "paused": paused,
        "delayMode": delay_mode,
        "success": success,
        "failed": failed,
        "progress": progress,
    })
}

fn finish_paused_export(
    save_dir_str: &str,
    url: &str,
    book: &YuqueBook,
    book_dir: &Path,
    mut progress: YuqueProgressState,
    resumed: bool,
    delay_mode: &str,
    completed_set: &std::collections::HashSet<String>,
    newly_exported: usize,
    skipped_count: usize,
    success: Vec<serde_json::Value>,
    failed: Vec<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    progress.status = Some("paused".into());
    progress.current_slug = None;
    progress.updated_at = Some(Utc::now().to_rfc3339());
    set_active_progress(save_dir_str, url, progress.clone());
    clear_export_cancel(save_dir_str, url);
    Ok(batch_export_result(
        book,
        book_dir,
        progress,
        resumed,
        delay_mode,
        completed_set.len(),
        newly_exported,
        skipped_count,
        false,
        true,
        success,
        failed,
    ))
}

fn set_active_progress(save_dir: &str, url: &str, state: YuqueProgressState) {
    let key = progress_store_key(save_dir, url);
    if let Ok(mut map) = ACTIVE_PROGRESS.lock() {
        map.insert(key, state.clone());
    }
    save_progress_file(Path::new(save_dir), url, &state);
}

fn get_active_progress(save_dir: &str, url: &str) -> Option<YuqueProgressState> {
    let key = progress_store_key(save_dir, url);
    ACTIVE_PROGRESS.lock().ok()?.get(&key).cloned()
}

fn doc_output_file_exists(target_dir: &Path, base_name: &str) -> bool {
    for i in 0..=20 {
        let stem = if i == 0 {
            base_name.to_string()
        } else {
            format!("{base_name}_{i}")
        };
        for ext in ["md", "html"] {
            if target_dir.join(format!("{stem}.{ext}")).exists() {
                return true;
            }
        }
    }
    false
}

fn doc_output_exists(book_dir: &Path, doc: &YuqueDocPlan, use_doc_folder: bool) -> bool {
    let target_dir = if doc.dir_segments.is_empty() {
        book_dir.to_path_buf()
    } else {
        book_dir.join(doc.dir_segments.join("/"))
    };
    if !target_dir.exists() {
        return false;
    }
    let base_name = safe_file_name(&doc.title);
    if use_doc_folder {
        for i in 0..=20 {
            let folder_name = if i == 0 {
                base_name.clone()
            } else {
                format!("{base_name}_{i}")
            };
            let folder = target_dir.join(&folder_name);
            if folder.is_dir() && doc_output_file_exists(&folder, &base_name) {
                return true;
            }
        }
        false
    } else {
        doc_output_file_exists(&target_dir, &base_name)
    }
}

pub fn get_export_progress_summary(
    save_dir: &str,
    url: &str,
    auth_mode: Option<&str>,
    client_progress: Option<YuqueProgressState>,
) -> serde_json::Value {
    let mut candidates = Vec::new();
    if let Some(a) = get_active_progress(save_dir, url) {
        candidates.push(a);
    }
    if let Some(cp) = client_progress {
        if normalize_url_key(&cp.url) == normalize_url_key(url) {
            candidates.push(cp);
        }
    }
    if let Some(fp) = load_progress_file(Path::new(save_dir), url) {
        candidates.push(fp);
    }

    let progress = pick_latest_progress(candidates).and_then(|p| {
        if let Some(mode) = auth_mode {
            if p.auth_mode.as_deref().is_some_and(|m| m != mode) {
                return None;
            }
        }
        Some(p)
    });

    match progress {
        None => serde_json::json!({ "found": false }),
        Some(p) => build_progress_summary(&p),
    }
}

fn build_progress_summary(progress: &YuqueProgressState) -> serde_json::Value {
    let completed_slugs = progress.completed_slugs.clone().unwrap_or_default();
    let failed_list = progress.failed.clone().unwrap_or_default();
    let total = progress.total.unwrap_or(completed_slugs.len());
    let completed = completed_slugs.len();
    let failed_count = failed_list.len();
    let completed_set: std::collections::HashSet<_> = completed_slugs.iter().cloned().collect();
    let failed_map: HashMap<_, _> = failed_list.iter().map(|f| (f.slug.clone(), f)).collect();

    let manifest = progress.doc_manifest.clone().unwrap_or_default();
    let docs: Vec<_> = manifest
        .iter()
        .map(|d| {
            let status = if progress.current_slug.as_deref() == Some(d.slug.as_str()) {
                "exporting"
            } else if completed_set.contains(&d.slug) {
                "done"
            } else if failed_map.contains_key(&d.slug) {
                "failed"
            } else {
                "pending"
            };
            serde_json::json!({
                "slug": d.slug,
                "title": d.title,
                "dirPath": d.dir_path,
                "status": status,
                "failMessage": failed_map.get(&d.slug).map(|f| f.message.clone()),
            })
        })
        .collect();

    serde_json::json!({
        "found": true,
        "bookName": progress.book_name,
        "bookDir": progress.book_dir,
        "total": total,
        "completed": completed,
        "remaining": total.saturating_sub(completed),
        "failedCount": failed_count,
        "status": progress.status,
        "updatedAt": progress.updated_at,
        "startedAt": progress.started_at,
        "currentSlug": progress.current_slug,
        "completedSlugs": completed_slugs,
        "failed": failed_list,
        "docs": docs,
        "progress": progress,
    })
}

fn resolve_delay_ms(mode: &str, fixed_sec: u64, min_sec: u64, max_sec: u64) -> u64 {
    match mode {
        "none" => 0,
        "fixed" => fixed_sec * 1000,
        _ => {
            let min = min_sec.min(max_sec);
            let max = min_sec.max(max_sec);
            if max <= min {
                min * 1000
            } else {
                rand::thread_rng().gen_range(min..=max) * 1000
            }
        }
    }
}

fn has_pending_docs(
    docs: &[YuqueDocPlan],
    from: usize,
    completed: &std::collections::HashSet<String>,
    book_dir: &Path,
    use_doc_folder: bool,
) -> bool {
    docs.iter()
        .skip(from)
        .any(|d| !completed.contains(&d.slug) && !doc_output_exists(book_dir, d, use_doc_folder))
}

async fn run_yuque_batch_export<F, Fut>(
    url: &str,
    save_dir: &Path,
    auth_mode: &str,
    namespace: Option<String>,
    book: &YuqueBook,
    resume: bool,
    existing_progress: Option<YuqueProgressState>,
    delay_mode: &str,
    delay_fixed_sec: u64,
    delay_min_sec: u64,
    delay_max_sec: u64,
    use_doc_folder: bool,
    stop_on_error: bool,
    export_order: &str,
    selected_slugs: Option<&[String]>,
    export_one: F,
) -> Result<serde_json::Value, String>
where
    F: Fn(YuqueDocPlan, PathBuf) -> Fut,
    Fut: std::future::Future<Output = Result<serde_json::Value, String>>,
{
    fs::create_dir_all(save_dir).map_err(|e| e.to_string())?;

    let save_dir_str = save_dir.to_string_lossy();
    let resumed = if resume {
        let mut candidates = Vec::new();
        if let Some(p) = get_active_progress(&save_dir_str, url) {
            candidates.push(p);
        }
        if let Some(p) = existing_progress {
            candidates.push(p);
        }
        if let Some(p) = load_progress_file(save_dir, url) {
            candidates.push(p);
        }
        pick_latest_progress(candidates).and_then(|p| {
            p.book_dir
                .as_ref()
                .map(|d| PathBuf::from(d))
                .filter(|d| d.exists())
                .map(|_| p)
        })
    } else {
        None
    };

    let (book_dir, mut progress) = if let Some(ref rp) = resumed {
        if let Some(ref dir) = rp.book_dir {
            (
                PathBuf::from(dir),
                rp.clone(),
            )
        } else {
            let dir = unique_dir_path(save_dir, &safe_file_name(&book.name));
            fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let progress = new_progress(url, auth_mode, namespace.clone(), &book.name, &dir, save_dir);
            (dir, progress)
        }
    } else {
        let dir = unique_dir_path(save_dir, &safe_file_name(&book.name));
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let progress = new_progress(url, auth_mode, namespace.clone(), &book.name, &dir, save_dir);
        (dir, progress)
    };

    progress.total = Some(book.docs.len());
    progress.doc_manifest = Some(
        book.docs
            .iter()
            .map(|d| YuqueDocManifestItem {
                slug: d.slug.clone(),
                title: d.title.clone(),
                dir_path: if d.dir_segments.is_empty() {
                    "(根目录)".into()
                } else {
                    d.dir_segments.join("/")
                },
            })
            .collect(),
    );
    progress.export_order = Some(export_order.to_string());
    progress.selected_slugs = selected_slugs.map(|s| s.to_vec());
    progress.current_slug = None;
    progress.updated_at = Some(Utc::now().to_rfc3339());
    set_active_progress(&save_dir_str, url, progress.clone());

    let mut completed_set: std::collections::HashSet<String> =
        progress.completed_slugs.clone().unwrap_or_default().into_iter().collect();
    for doc in &book.docs {
        if completed_set.contains(&doc.slug) {
            continue;
        }
        if doc_output_exists(&book_dir, doc, use_doc_folder) {
            completed_set.insert(doc.slug.clone());
        }
    }
    progress.completed_slugs = Some(completed_set.iter().cloned().collect());

    let mut success = Vec::new();
    let mut failed = Vec::new();
    let mut skipped_count = 0usize;
    let mut newly_exported = 0usize;
    let mut stopped_early = false;

    clear_export_cancel(&save_dir_str, url);

    for (i, doc) in book.docs.iter().enumerate() {
        if completed_set.contains(&doc.slug) {
            skipped_count += 1;
            continue;
        }

        if is_export_cancel_requested(&save_dir_str, url) {
            return finish_paused_export(
                &save_dir_str,
                url,
                book,
                &book_dir,
                progress,
                resumed.is_some(),
                delay_mode,
                &completed_set,
                newly_exported,
                skipped_count,
                success,
                failed,
            );
        }

        let target_dir = if doc.dir_segments.is_empty() {
            book_dir.clone()
        } else {
            book_dir.join(doc.dir_segments.join("/"))
        };
        fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;

        progress.current_slug = Some(doc.slug.clone());
        progress.updated_at = Some(Utc::now().to_rfc3339());
        set_active_progress(&save_dir_str, url, progress.clone());

        match export_one(doc.clone(), target_dir.clone()).await {
            Ok(mut result) => {
                completed_set.insert(doc.slug.clone());
                progress.completed_slugs = Some(completed_set.iter().cloned().collect());
                if let Some(ref mut fl) = progress.failed {
                    fl.retain(|f| f.slug != doc.slug);
                }
                progress.current_slug = None;
                progress.updated_at = Some(Utc::now().to_rfc3339());
                set_active_progress(&save_dir_str, url, progress.clone());

                if let Some(obj) = result.as_object_mut() {
                    obj.insert("slug".into(), doc.slug.clone().into());
                }
                success.push(result);
                newly_exported += 1;
            }
            Err(e) => {
                let fail = YuqueFailedItem {
                    slug: doc.slug.clone(),
                    title: Some(doc.title.clone()),
                    message: e,
                    at: Some(Utc::now().to_rfc3339()),
                };
                let mut fl = progress.failed.clone().unwrap_or_default();
                fl.retain(|f| f.slug != doc.slug);
                fl.push(fail.clone());
                progress.failed = Some(fl);
                progress.current_slug = None;
                progress.updated_at = Some(Utc::now().to_rfc3339());
                set_active_progress(&save_dir_str, url, progress.clone());
                let dir_path = if doc.dir_segments.is_empty() {
                    "(根目录)".to_string()
                } else {
                    doc.dir_segments.join("/")
                };
                failed.push(serde_json::json!({
                    "slug": doc.slug,
                    "title": doc.title,
                    "dirPath": dir_path,
                    "message": fail.message,
                }));
                if stop_on_error {
                    progress.status = Some("stopped_on_error".into());
                    progress.updated_at = Some(Utc::now().to_rfc3339());
                    set_active_progress(&save_dir_str, url, progress.clone());
                    stopped_early = true;
                    break;
                }
            }
        }

        if i + 1 < book.docs.len()
            && has_pending_docs(&book.docs, i + 1, &completed_set, &book_dir, use_doc_folder)
        {
            let wait = resolve_delay_ms(delay_mode, delay_fixed_sec, delay_min_sec, delay_max_sec);
            if sleep_interruptible(&save_dir_str, url, wait).await {
                return finish_paused_export(
                    &save_dir_str,
                    url,
                    book,
                    &book_dir,
                    progress,
                    resumed.is_some(),
                    delay_mode,
                    &completed_set,
                    newly_exported,
                    skipped_count,
                    success,
                    failed,
                );
            }
        }
    }

    let exported_total = completed_set.len();
    progress.status = Some(
        if exported_total >= book.docs.len() && progress.failed.as_ref().map(|f| f.is_empty()).unwrap_or(true) {
            "completed".into()
        } else {
            "in_progress".into()
        },
    );
    progress.current_slug = None;
    progress.updated_at = Some(Utc::now().to_rfc3339());
    set_active_progress(&save_dir_str, url, progress.clone());

    let success: Vec<_> = success
        .into_iter()
        .map(|mut item| {
            if let Some(obj) = item.as_object_mut() {
                if let Some(folder) = obj.get("folderPath").and_then(|v| v.as_str()) {
                    let rel = pathdiff::diff_paths(folder, &book_dir)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();
                    obj.insert("relativePath".into(), rel.into());
                } else if let Some(fp) = obj.get("filePath").and_then(|v| v.as_str()) {
                    let rel = pathdiff::diff_paths(fp, &book_dir)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();
                    obj.insert("relativePath".into(), rel.into());
                }
            }
            item
        })
        .collect();

    clear_export_cancel(&save_dir_str, url);
    Ok(batch_export_result(
        book,
        &book_dir,
        progress,
        resumed.is_some(),
        delay_mode,
        exported_total,
        newly_exported,
        skipped_count,
        stopped_early,
        false,
        success,
        failed,
    ))
}

fn new_progress(
    url: &str,
    auth_mode: &str,
    namespace: Option<String>,
    book_name: &str,
    book_dir: &Path,
    save_dir: &Path,
) -> YuqueProgressState {
    YuqueProgressState {
        version: 1,
        url: url.to_string(),
        auth_mode: Some(auth_mode.to_string()),
        namespace,
        book_name: Some(book_name.to_string()),
        book_dir: Some(book_dir.to_string_lossy().to_string()),
        save_dir: Some(save_dir.to_string_lossy().to_string()),
        total: None,
        completed_slugs: Some(vec![]),
        failed: Some(vec![]),
        doc_manifest: None,
        current_slug: None,
        status: Some("in_progress".into()),
        started_at: Some(Utc::now().to_rfc3339()),
        updated_at: Some(Utc::now().to_rfc3339()),
        export_order: None,
        selected_slugs: None,
    }
}

pub async fn export_yuque_batch(params: YuqueBatchParams) -> Result<serde_json::Value, String> {
    let save_dir = PathBuf::from(params.save_dir.trim());
    if params.url.trim().is_empty() {
        return Err("请提供语雀链接".into());
    }
    if params.save_dir.trim().is_empty() {
        return Err("请选择保存目录".into());
    }
    fs::create_dir_all(&save_dir).map_err(|e| e.to_string())?;

    let format = normalize_export_format(params.export_format, params.export_confluence_html);
    let delay_mode = params.delay_mode.clone();
    let use_doc_folder = params.use_doc_folder;
    let stop_on_error = params.stop_on_error;
    let export_order = params.export_order.clone();
    let selected_slugs = params.selected_slugs.clone();

    if let Some(ref token) = params.token {
        if !token.trim().is_empty() {
            let (parsed, book) = fetch_yuque_book_by_api(token, &params.url).await?;
            let namespace = parsed.path_prefix.clone();
            let export_docs = prepare_batch_docs(&book.docs, &export_order, selected_slugs.as_deref())?;
            let export_book = YuqueBook {
                name: book.name.clone(),
                docs: export_docs,
            };
            return run_yuque_batch_export(
                &params.url,
                &save_dir,
                "token",
                namespace.clone(),
                &export_book,
                params.resume,
                params.progress,
                &delay_mode,
                params.delay_fixed_sec,
                params.delay_min_sec,
                params.delay_max_sec,
                use_doc_folder,
                stop_on_error,
                &export_order,
                selected_slugs.as_deref(),
                |doc, target_dir| {
                    let token = token.clone();
                    let ns = namespace.clone().unwrap_or_default();
                    let dl = params.download_images;
                    let std = params.standard_markdown;
                    let fmt = format;
                    let use_doc_folder = use_doc_folder;
                    async move {
                        let raw = fetch_doc_markdown_by_api(&token, &ns, &doc.slug).await?;
                        let images = images_from_markdown(&raw);
                        save_yuque_doc_content(
                            &doc.title,
                            &raw,
                            &images,
                            &target_dir,
                            dl,
                            std,
                            use_doc_folder,
                            fmt,
                        )
                        .await
                    }
                },
            )
            .await;
        }
    }

    let (parsed, book) = fetch_yuque_book(&params.url).await?;
    let namespace = parsed.path_prefix.clone();
    let export_docs = prepare_batch_docs(&book.docs, &export_order, selected_slugs.as_deref())?;
    let export_book = YuqueBook {
        name: book.name.clone(),
        docs: export_docs,
    };
    run_yuque_batch_export(
        &params.url,
        &save_dir,
        "share",
        namespace.clone(),
        &export_book,
        params.resume,
        params.progress,
        &delay_mode,
        params.delay_fixed_sec,
        params.delay_min_sec,
        params.delay_max_sec,
        use_doc_folder,
        stop_on_error,
        &export_order,
        selected_slugs.as_deref(),
        move |doc, target_dir| {
            let parsed = parsed.clone();
            let dl = params.download_images;
            let std = params.standard_markdown;
            let fmt = format;
            let use_doc_folder = use_doc_folder;
            async move {
                let (page_url, markdown_url) = build_doc_urls(&parsed, &doc.slug);
                let markdown = fetch_yuque_markdown(&markdown_url).await?;
                let html = http_get(&page_url).await.unwrap_or_default();
                let title = if doc.title.is_empty() {
                    extract_title_from_html(&html)
                } else {
                    doc.title.clone()
                };
                let images = images_from_markdown(&markdown);
                save_yuque_doc_content(&title, &markdown, &images, &target_dir, dl, std, use_doc_folder, fmt).await
            }
        },
    )
    .await
}
