//! 报告与拓扑的导出：PNG、PDF、Word。
//!
//! 三者的内容都来自 `render` 里的 SVG 或同一份漏洞数据：
//! - PNG、PDF 把 SVG 光栅化，PDF 只是把每页图片封进一个文件；
//! - Word 走结构化生成，图片按原始像素比例嵌入。

use std::sync::Arc;

use docx_rs::{
    Docx, Paragraph, Pic, Run, Table, TableCell, TableRow, WidthType,
};

use crate::error::AppError;
use crate::model::graph::{Graph, Vulnerability};
use crate::mcp::render::{self, ReportMeta, PAGE_H, PAGE_W, SCALE};

/// 一像素换算成 docx 的 EMU
const EMU: u32 = 9525;

/// 报告里正文的最大图片宽度（px），A4 可用宽度约 486pt
const SHOT_W: u32 = 460;
const SHOT_MAX_H: u32 = 520;

/// 加载字体：先读系统字体库，再补几个常见目录。
///
/// resvg 渲染中文必须有 CJK 字体，找不到就只能画方框，
/// 所以这里把常见位置都试一遍，并且允许用环境变量指定。
fn font_db() -> Arc<fontdb::Database> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();

    let mut dirs: Vec<String> = Vec::new();
    if let Ok(custom) = std::env::var("ATLAS_FONT_DIR") {
        dirs.push(custom);
    }
    dirs.extend(
        [
            "/usr/share/fonts",
            "/usr/local/share/fonts",
            "C:\\Windows\\Fonts",
            "/System/Library/Fonts",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    for dir in dirs {
        if std::path::Path::new(&dir).is_dir() {
            db.load_fonts_dir(&dir);
        }
    }
    Arc::new(db)
}

/// SVG 光栅化成 PNG 字节
fn svg_to_png(svg: &str, scale: f32) -> Result<Vec<u8>, AppError> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = font_db();
    let tree = resvg::usvg::Tree::from_str(svg, &opt)
        .map_err(|e| AppError::BadRequest(format!("SVG 解析失败：{e}")))?;

    let size = tree.size();
    let width = (size.width() * scale).round().max(1.0) as u32;
    let height = (size.height() * scale).round().max(1.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| AppError::BadRequest("图片尺寸超出可渲染范围".into()))?;

    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
        .encode_png()
        .map_err(|e| AppError::BadRequest(format!("PNG 编码失败：{e}")))
}

/// SVG 光栅化成 JPEG，PDF 里直嵌，体积比 PNG 小得多
fn svg_to_jpeg(svg: &str, scale: f32) -> Result<(Vec<u8>, u32, u32), AppError> {
    let png = svg_to_png(svg, scale)?;
    let img = image::load_from_memory(&png)
        .map_err(|e| AppError::BadRequest(format!("图像解码失败：{e}")))?;
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width(), rgb.height());

    let mut out = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 88);
    encoder
        .encode(&rgb, w, h, image::ExtendedColorType::Rgb8)
        .map_err(|e| AppError::BadRequest(format!("JPEG 编码失败：{e}")))?;
    Ok((out, w, h))
}

/// 画布拓扑导出 PNG
pub fn canvas_png(graph: &Graph, title: &str) -> Result<Vec<u8>, AppError> {
    let (svg, _w, _h) = render::canvas_svg(graph, title);
    // 画布信息密度高，2 倍图在缩放后仍然清晰
    svg_to_png(&svg, 2.0)
}

/// 报告导出 PNG：多页竖向拼成一张长图，便于直接贴进即时通讯工具
pub fn report_png(meta: &ReportMeta, vulns: &[Vulnerability], images: &ImageStore) -> Result<Vec<u8>, AppError> {
    let pages = render::report_pages_svg(meta, vulns);
    let total_h = PAGE_H * pages.len() as f64;
    let mut combined = String::with_capacity(pages.len() * 4096);
    combined.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{PAGE_W}" height="{total_h}" viewBox="0 0 {PAGE_W} {total_h}"><rect width="100%" height="100%" fill="#ffffff"/>"##
    ));
    for (index, page) in pages.iter().enumerate() {
        let body = strip_svg(&page.to_string());
        combined.push_str(&format!(
            r#"<g transform="translate(0,{:.0})">{}</g>"#,
            PAGE_H * index as f64,
            body
        ));
    }
    combined.push_str("</svg>");
    let inline = inline_images(&combined, images);
    svg_to_png(&inline, 1.5)
}

/// 去掉 `<svg ...>` 外壳和根背景，只留内容
fn strip_svg(svg: &str) -> String {
    let start = svg.find("\"/>\n").map(|i| i + 4).or_else(|| svg.find("\">").map(|i| i + 2));
    let body = match start {
        Some(i) => &svg[i..],
        None => svg,
    };
    body.trim_end().trim_end_matches("</svg>").to_string()
}

/// 把 SVG 里引用的站内图片换成 data URI，resvg 才能渲染
fn inline_images(svg: &str, images: &ImageStore) -> String {
    let mut out = String::with_capacity(svg.len() + 4096);
    let mut rest = svg;
    while let Some(at) = rest.find("href=\"") {
        out.push_str(&rest[..at + 6]);
        let tail = &rest[at + 6..];
        let end = tail.find('"').unwrap_or(tail.len());
        let src = &tail[..end];
        if let Some(data) = images.data_uri(src) {
            out.push_str(&data);
        } else {
            out.push_str(src);
        }
        out.push('"');
        rest = &tail[(end + 1).min(tail.len())..];
    }
    out.push_str(rest);
    out
}

/// 报告导出 PDF：每页一张 JPEG，用 DCTDecode 直嵌，不重新编码
pub fn report_pdf(
    meta: &ReportMeta,
    vulns: &[Vulnerability],
    images: &ImageStore,
) -> Result<Vec<u8>, AppError> {
    let pages = render::report_pages_svg(meta, vulns);
    let mut encoded: Vec<(Vec<u8>, u32, u32)> = Vec::new();
    for page in &pages {
        let inline = inline_images(page, images);
        encoded.push(svg_to_jpeg(&inline, SCALE as f32)?);
    }
    Ok(build_pdf(&encoded, &meta.node))
}

/// 手写 PDF：Catalog + Pages + 每页一个 Image XObject
///
/// 结构够简单，不引第三方库反而更可控：页面尺寸按 A4，图片铺满整页。
fn build_pdf(pages: &[(Vec<u8>, u32, u32)], title: &str) -> Vec<u8> {
    let mut objects: Vec<Vec<u8>> = Vec::new();
    let page_count = pages.len();

    // 1: Catalog, 2: Pages, 3+: 每页的 Page 与 Image 与 Contents
    // obj 1 = Catalog，obj 2 = Pages，之后每页占 3 个对象
    let page_id = |i: usize| 3 + i * 3;
    let image_id = |i: usize| 4 + i * 3;
    let content_id = |i: usize| 5 + i * 3;

    let kids: Vec<String> = (0..page_count).map(|i| format!("{} 0 R", page_id(i))).collect();
    objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    objects.push(
        format!(
            "<< /Type /Pages /Count {page_count} /Kids [{}] >>",
            kids.join(" ")
        )
        .into_bytes(),
    );

    for (i, (data, w, h)) in pages.iter().enumerate() {
        // A4：595.28 x 841.89 pt
        let (pw, ph) = (595.28_f64, 841.89_f64);
        // 图片按页宽铺满，高度按比例
        let draw_h = pw * (*h as f64) / (*w as f64);
        let scale = (ph / draw_h).min(1.0);
        let (_dw, _dh) = (pw * scale, draw_h * scale);

        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {pw:.2} {ph:.2}] \
                 /Resources << /XObject << /Im0 {} 0 R >> >> /Contents {} 0 R >>",
                image_id(i),
                content_id(i)
            )
            .into_bytes(),
        );
        objects.push({
            let mut img = format!(
                "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} \
                 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
                data.len()
            )
            .into_bytes();
            img.extend_from_slice(data);
            img.extend_from_slice(b"\nendstream");
            img
        });
        // 内容流：把图片画到页面左下角起、铺满页宽
        // 图片按页宽铺满，垂直居中放置
        let offset = ((ph - draw_h) / 2.0).max(0.0);
        let content = format!("q\n{pw:.2} 0 0 {draw_h:.2} 0 {offset:.2} cm\n/Im0 Do\nQ\n");
        objects.push({
            let mut c = format!(
                "<< /Length {} >>\nstream\n",
                content.len()
            )
            .into_bytes();
            c.extend_from_slice(content.as_bytes());
            c.extend_from_slice(b"endstream");
            c
        });
    }

    // Info 字典
    let info_id = 3 + page_count * 3;
    objects.push(
        format!(
            "<< /Title ({}) /Producer (RedTeam Atlas) >>",
            title.replace('(', "\\(").replace(')', "\\)")
        )
        .into_bytes(),
    );

    // 拼装
    let mut out: Vec<u8> = Vec::with_capacity(4096);
    out.extend_from_slice(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n");
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref_at = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for off in &offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info {info_id} 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// 报告导出 Word
pub fn report_word(
    meta: &ReportMeta,
    vulns: &[Vulnerability],
    images: &ImageStore,
) -> Result<Vec<u8>, AppError> {
    let mut doc = Docx::new();

    doc = doc.add_paragraph(
        Paragraph::new().add_run(Run::new().add_text(&meta.node).size(36).bold()),
    );
    doc = doc.add_paragraph(
        Paragraph::new().add_run(
            Run::new()
                .add_text(&format!("{} / {}", meta.project, kind_text(&meta.kind)))
                .size(20)
                .color("6B7280"),
        ),
    );

    let mut facts: Vec<(&str, String)> = Vec::new();
    let addr = match (meta.ip.is_empty(), meta.port.is_empty()) {
        (false, false) => format!("{}:{}", meta.ip, meta.port),
        (false, true) => meta.ip.clone(),
        (true, false) => meta.port.clone(),
        (true, true) => String::new(),
    };
    if !addr.is_empty() {
        facts.push(("IP / 端口", addr));
    }
    if !meta.domain.is_empty() {
        facts.push(("域名", meta.domain.clone()));
    }
    if !meta.notes.is_empty() {
        facts.push(("备注", meta.notes.clone()));
    }
    facts.push(("漏洞数量", format!("{} 条", vulns.len())));
    facts.push(("报告生成", meta.generated_at.clone()));
    if !meta.author.is_empty() {
        facts.push(("导出人", meta.author.clone()));
    }

    let header = TableRow::new(vec![
        cell("字段", true),
        cell("内容", true),
    ]);
    let rows: Vec<TableRow> = facts
        .iter()
        .map(|(k, v)| TableRow::new(vec![cell(k, false), cell(v, false)]))
        .collect();
    doc = doc.add_table(
        Table::new(rows.into_iter().fold(vec![header], |mut acc, r| {
            acc.push(r);
            acc
        }))
        .width(9706, WidthType::Dxa),
    );

    if vulns.is_empty() {
        doc = doc.add_paragraph(Paragraph::new().add_run(Run::new().add_text("该资产暂无漏洞记录。")));
    }

    for (index, vuln) in vulns.iter().enumerate() {
        doc = doc.add_paragraph(Paragraph::new());
        doc = doc.add_paragraph(
            Paragraph::new().add_run(
                Run::new()
                    .add_text(&format!(
                        "{}. {}  {}",
                        index + 1,
                        if vuln.name.is_empty() { "未命名漏洞" } else { &vuln.name },
                        render::risk_text(&vuln.risk)
                    ))
                    .size(28)
                    .bold(),
            ),
        );

        if !vuln.description.is_empty() {
            doc = doc.add_paragraph(label("漏洞说明"));
            doc = doc.add_paragraph(body(&vuln.description));
        }

        if !vuln.detail.is_empty() {
            doc = doc.add_paragraph(label("渗透测试详情"));
            for block in detail_blocks(&vuln.detail) {
                match block {
                    TextBlock::Line(text) => {
                        doc = doc.add_paragraph(body(&text));
                    }
                    TextBlock::Image(src) => {
                        doc = add_image_paragraph(doc, &src, images, index);
                    }
                }
            }
        }

        if !vuln.remediation.is_empty() {
            doc = doc.add_paragraph(label("处置建议"));
            for line in vuln.remediation.split('\n') {
                if line.trim().is_empty() {
                    continue;
                }
                doc = doc.add_paragraph(body(line.trim()));
            }
        }
    }

    doc = doc.add_paragraph(Paragraph::new());
    doc = doc.add_paragraph(
        Paragraph::new().add_run(
            Run::new()
                .add_text("本报告由 RedTeam Atlas 生成，内容以项目内记录为准。")
                .size(18)
                .color("9CA3AF"),
        ),
    );

    // pack 需要 Read + Seek，Vec 不满足，套一层 Cursor
    let mut cursor = std::io::Cursor::new(Vec::new());
    doc.build()
        .pack(&mut cursor)
        .map_err(|e| AppError::BadRequest(format!("Word 打包失败：{e}")))?;
    Ok(cursor.into_inner())
}

fn kind_text(kind: &str) -> &'static str {
    match kind {
        "attacker" => "攻击者",
        "entry" => "攻击入口",
        "web" => "Web 应用",
        "cloud" => "云资源",
        "host" => "主机服务器",
        "domain" => "域控 / AD",
        "database" => "数据库",
        "workstation" => "工作站",
        "network" => "网络 / 网段",
        "firewall" => "防火墙 / WAF",
        "pivot" => "跳板 / 隧道",
        "credential" => "凭证 / 账号",
        "vuln" => "漏洞利用点",
        "flag" => "战果 / 目标",
        _ => "自定义组件",
    }
}

fn cell(text: &str, head: bool) -> TableCell {
    let mut run = Run::new().add_text(text).size(18);
    if head {
        run = run.bold();
    }
    TableCell::new().add_paragraph(Paragraph::new().add_run(run))
}

fn label(text: &str) -> Paragraph {
    Paragraph::new().add_run(Run::new().add_text(text).size(19).bold().color("374151"))
}

fn body(text: &str) -> Paragraph {
    Paragraph::new().add_run(Run::new().add_text(text).size(21))
}

/// 图片段落：按真实像素比例缩放，读不到的留一行说明
fn add_image_paragraph(
    doc: Docx,
    src: &str,
    images: &ImageStore,
    index: usize,
) -> Docx {
    let Some((bytes, w, h)) = images.load(src) else {
        return doc.add_paragraph(Paragraph::new().add_run(
            Run::new()
                .add_text(&format!(
                    "（漏洞 {} 的图片无法嵌入：地址取不到，或格式不受 Word 支持）",
                    index + 1
                ))
                .size(17)
                .color("A1A1AA"),
        ));
    };
    if w == 0 || h == 0 {
        return doc.add_paragraph(Paragraph::new().add_run(
            Run::new()
                .add_text(&format!("（漏洞 {} 的图片尺寸读不出）", index + 1))
                .size(17)
                .color("A1A1AA"),
        ));
    }
    // 等比缩放到 460 宽、最高 520
    let scale = (SHOT_W as f64 / w as f64).min(SHOT_MAX_H as f64 / h as f64).min(1.0);
    let dw = ((w as f64 * scale).round() as u32).max(1);
    let dh = ((h as f64 * scale).round() as u32).max(1);

    let pic = Pic::new(&bytes).size(dw * EMU, dh * EMU);
    doc.add_paragraph(Paragraph::new().add_run(Run::new().add_image(pic)))
}

/// 报告图片的取用入口
pub struct ImageStore {
    /// 上传目录
    pub root: std::path::PathBuf,
}

impl ImageStore {
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// 从图片地址取本地文件：`/api/pub/uploads/img_x?sig=...` 或完整 URL。
    ///
    /// URL 里的 id 不带扩展名，落盘的文件名带（`img_x.png`），
    /// 所以精确名找不到时按 `id.*` 再找一遍。
    fn resolve(&self, src: &str) -> Option<std::path::PathBuf> {
        let path = src.split(['?', '#']).next().unwrap_or(src);
        let name = path.rsplit('/').next()?;
        if name.is_empty() || name.contains("..") || name.contains('/') || name.contains('\\') {
            return None;
        }
        let exact = self.root.join(name);
        if exact.is_file() {
            return Some(exact);
        }
        // 同名不同扩展名：目录里扫一遍前缀
        let entries = std::fs::read_dir(&self.root).ok()?;
        for entry in entries.flatten() {
            let fname = entry.file_name();
            let fname = fname.to_string_lossy();
            if let Some(stem) = fname.split('.').next() {
                if stem == name && entry.path().is_file() {
                    return Some(entry.path());
                }
            }
        }
        None
    }

    /// 读字节并解析尺寸
    pub fn load(&self, src: &str) -> Option<(Vec<u8>, u32, u32)> {
        if src.starts_with("data:") {
            return decode_data_uri(src);
        }
        let path = self.resolve(src)?;
        let bytes = std::fs::read(path).ok()?;
        let size = image_size(&bytes)?;
        Some((bytes, size.0, size.1))
    }

    /// 转成 data URI，供 SVG 内联
    pub fn data_uri(&self, src: &str) -> Option<String> {
        if src.starts_with("data:") {
            return Some(src.to_string());
        }
        let path = self.resolve(src)?;
        let bytes = std::fs::read(path).ok()?;
        let mime = sniff_mime(&bytes)?;
        Some(format!(
            "data:{mime};base64,{}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes)
        ))
    }
}

fn decode_data_uri(src: &str) -> Option<(Vec<u8>, u32, u32)> {
    let comma = src.find(',')?;
    let head = &src[..comma];
    if !head.contains("base64") {
        return None;
    }
    let raw = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        src[comma + 1..].trim(),
    )
    .ok()?;
    let (w, h) = image_size(&raw)?;
    Some((raw, w, h))
}

fn sniff_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() < 12 {
        return None;
    }
    if bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return Some("image/jpeg");
    }
    if bytes[0] == 0x89 && bytes[1] == 0x50 && bytes[2] == 0x4E && bytes[3] == 0x47 {
        return Some("image/png");
    }
    if bytes[0] == 0x47 && bytes[1] == 0x49 && bytes[2] == 0x46 && bytes[3] == 0x38 {
        return Some("image/gif");
    }
    if bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    None
}

/// 从文件头读尺寸，PNG 看 IHDR，JPEG 逐段找 SOF
fn image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() > 24
        && bytes[0] == 0x89
        && bytes[1] == 0x50
        && bytes[2] == 0x4E
        && bytes[3] == 0x47
        && &bytes[12..16] == b"IHDR"
    {
        let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        return (w > 0 && h > 0).then_some((w, h));
    }
    if bytes.len() > 4 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
        let mut i = 2usize;
        while i + 8 < bytes.len() {
            if bytes[i] != 0xFF {
                i += 1;
                continue;
            }
            let marker = bytes[i + 1];
            let is_sof = (0xC0..=0xCF).contains(&marker)
                && marker != 0xC4
                && marker != 0xC8
                && marker != 0xCC;
            let len = ((bytes[i + 2] as usize) << 8) | bytes[i + 3] as usize;
            if is_sof {
                let h = ((bytes[i + 5] as u32) << 8) | bytes[i + 6] as u32;
                let w = ((bytes[i + 7] as u32) << 8) | bytes[i + 8] as u32;
                return (w > 0 && h > 0).then_some((w, h));
            }
            if marker == 0xD8 || marker == 0xD9 || (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
                i += 2;
                continue;
            }
            if len < 2 {
                return None;
            }
            i += 2 + len;
        }
    }
    // 认不出的交给 image crate 兜底
    image::load_from_memory(bytes)
        .ok()
        .map(|img| (img.width(), img.height()))
}

/// 把 HTML 详情拆成文本行与图片
enum TextBlock {
    Line(String),
    Image(String),
}

fn detail_blocks(html: &str) -> Vec<TextBlock> {
    let mut blocks = Vec::new();
    let mut rest = html;
    loop {
        match rest.find("<img") {
            None => {
                push_lines(&mut blocks, rest);
                break;
            }
            Some(at) => {
                push_lines(&mut blocks, &rest[..at]);
                let tail = &rest[at..];
                let close = tail.find('>').map(|i| i + 1).unwrap_or(tail.len());
                if let Some(src) = extract_attr(tail, "src") {
                    blocks.push(TextBlock::Image(src));
                }
                rest = &tail[close..];
            }
        }
    }
    blocks
}

fn extract_attr(tag: &str, name: &str) -> Option<String> {
    let pat = format!("{name}=");
    let at = tag.find(&pat)? + pat.len();
    let rest = &tag[at..];
    let quote = rest.chars().next()?;
    if quote == '"' || quote == '\'' {
        let body = &rest[1..];
        Some(body[..body.find(quote)?].to_string())
    } else {
        let end = rest
            .find(|c: char| c.is_whitespace() || c == '>')
            .unwrap_or(rest.len());
        Some(rest[..end].to_string())
    }
}

fn push_lines(blocks: &mut Vec<TextBlock>, chunk: &str) {
    if chunk.trim().is_empty() {
        return;
    }
    let spaced = chunk
        .replace("</p>", "\n")
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</li>", "\n")
        .replace("</div>", "\n")
        .replace("</h1>", "\n")
        .replace("</h2>", "\n")
        .replace("</h3>", "\n");
    let stripped = strip_tags(&spaced);
    for raw in stripped.split('\n') {
        let line = decode_entities(raw.trim());
        if !line.is_empty() {
            blocks.push(TextBlock::Line(line));
        }
    }
}

fn strip_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0usize;
    for ch in text.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out
}

fn decode_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}
/// 把导出结果落到 uploads/exports 下，返回可下载的地址
pub async fn publish(
    cfg: &crate::config::Config,
    bytes: &[u8],
    ext: &str,
    stem: &str,
) -> Result<String, AppError> {
    let dir = cfg.upload_dir.join("exports");
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| AppError::BadRequest(format!("创建导出目录失败：{e}")))?;

    // 文件名带上可读的项目名，下载后一眼能认出来；uuid 保证不重名
    let safe: String = stem
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .take(40)
        .collect();
    let name = format!("{safe}-{}.{ext}", &uuid::Uuid::new_v4().simple().to_string()[..12]);

    tokio::fs::write(dir.join(&name), bytes)
        .await
        .map_err(|e| AppError::BadRequest(format!("写入导出文件失败：{e}")))?;

    // 顺带清掉 24 小时前的旧导出件，免得目录一直长
    if let Ok(mut rd) = tokio::fs::read_dir(&dir).await {
        let now = std::time::SystemTime::now();
        while let Ok(Some(entry)) = rd.next_entry().await {
            let Ok(meta) = entry.metadata().await else { continue };
            let Ok(modified) = meta.modified() else { continue };
            if now.duration_since(modified).map(|d| d.as_secs()).unwrap_or(0) > 86_400 {
                let _ = tokio::fs::remove_file(entry.path()).await;
            }
        }
    }

    Ok(format!("/api/pub/exports/{name}"))
}