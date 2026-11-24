//! 把拓扑和报告画成 SVG。
//!
//! SVG 是唯一的内容表示：PNG 用 resvg 光栅化，PDF 把同样的页面光栅化后封装，
//! Word 走自己的结构化生成。三处共用这一份排版，改动只在这里做。
//!
//! 画布配色沿用前端的深色底 + 资产类型色条；报告页用白底黑字，适合打印。
//!
//! 书写约定：颜色常量自带 `#`，模板里写 `fill="{CANVAS_BG}"`；
//! raw string 一律用 `r##"…"##` 定界，避免内容里的 `"#` 提前闭合。

use crate::model::graph::{Graph, NodeData, Vulnerability};

/// 编辑区画布背景
const CANVAS_BG: &str = "#0a0e14";
const CARD_BG: &str = "#151a21";
const CARD_RULE: &str = "#2a3341";
const INK: &str = "#e8ebf0";
const MUTED: &str = "#7d8797";
const EDGE: &str = "#3f4a5a";

/// 报告页的白底与文字色
const PAPER: &str = "#ffffff";
const REPORT_INK: &str = "#111827";
const REPORT_BODY: &str = "#374151";
const REPORT_MUTED: &str = "#6b7280";
const REPORT_FAINT: &str = "#9ca3af";
const REPORT_RULE: &str = "#e5e7eb";
const REPORT_BAND: &str = "#f9fafb";
const REPORT_SLOT: &str = "#f3f4f6";

/// 节点卡片宽度，和前端 `.asset` 保持一致
const CARD_W: f64 = 248.0;
const CARD_HEADER: f64 = 54.0;
const LINE_H: f64 = 13.0;

/// 报告页尺寸：A4 在 96dpi 下的像素
pub const PAGE_W: f64 = 794.0;
pub const PAGE_H: f64 = 1123.0;
pub const SCALE: f64 = 2.0;

/// 资产类型配色，与 `asset_service::builtin_meta` 对齐
fn kind_color(kind: &str) -> &'static str {
    match kind {
        "attacker" => "#c96f7a",
        "entry" => "#d9a441",
        "web" => "#7fb0d8",
        "cloud" => "#8fb8c9",
        "host" => "#9fb3c8",
        "domain" => "#b49bd8",
        "database" => "#87c6a8",
        "workstation" => "#9aa8b8",
        "network" => "#8fa3b8",
        "firewall" => "#c98b7f",
        "pivot" => "#c9a87f",
        "credential" => "#d8b46a",
        "vuln" => "#d97f7f",
        "flag" => "#d9a441",
        _ => "#8892a0",
    }
}

fn kind_label(kind: &str) -> &'static str {
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

/// XML 转义，文本里出现 < & " 时不至于把结构冲垮
fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

/// 估算一段文字的显示宽度（px）：中日韩按 1em，ASCII 按 0.52em
fn text_width(text: &str, size: f64) -> f64 {
    text.chars()
        .map(|ch| if (ch as u32) < 0x2E80 { 0.52 } else { 1.0 })
        .sum::<f64>()
        * size
}

/// 按可用宽度折行；超出的用省略号收尾，绝不让文字压出卡片
fn wrap(text: &str, size: f64, max_w: f64, max_lines: usize) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch == '\n' {
            lines.push(current.clone());
            current.clear();
            if lines.len() >= max_lines {
                break;
            }
            continue;
        }
        let probe = format!("{current}{ch}");
        if text_width(&probe, size) > max_w && !current.is_empty() {
            lines.push(current.clone());
            current.clear();
            current.push(ch);
            if lines.len() >= max_lines {
                break;
            }
        } else {
            current.push(ch);
        }
    }
    if lines.len() < max_lines && !current.is_empty() {
        lines.push(current);
    }
    if lines.len() == max_lines {
        let shown: f64 = lines.iter().map(|l| text_width(l, size)).sum();
        if shown + size < text_width(text, size) {
            if let Some(last) = lines.last_mut() {
                while text_width(last, size) > max_w - size && last.chars().count() > 1 {
                    last.pop();
                }
                last.push('…');
            }
        }
    }
    lines
}

/// 节点卡片的高度：按实际要画的行数算，避免留白或溢出
fn card_height(node: &NodeData) -> f64 {
    let mut rows = 0usize;
    if !node.ip.is_empty() {
        rows += 1;
    }
    if !node.domain.is_empty() {
        rows += 1;
    }
    if !node.os.is_empty() {
        rows += 1;
    }
    if !node.notes.is_empty() {
        rows += 1;
    }
    if !node.fields.is_empty() {
        rows += 1;
    }
    let footer = if node.vulns.is_empty() { 0.0 } else { 22.0 };
    CARD_HEADER + rows as f64 * LINE_H + 14.0 + footer
}

/// 画布 SVG：节点卡片 + 贝塞尔连线 + 箭头
pub fn canvas_svg(graph: &Graph, title: &str) -> (String, u32, u32) {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    for node in &graph.nodes {
        min_x = min_x.min(node.position.x);
        min_y = min_y.min(node.position.y);
        max_x = max_x.max(node.position.x + CARD_W);
        max_y = max_y.max(node.position.y + card_height(&node.data));
    }
    if graph.nodes.is_empty() {
        min_x = 0.0;
        min_y = 0.0;
        max_x = 640.0;
        max_y = 360.0;
    }

    let pad = 48.0;
    let head = 56.0;
    let w = (max_x - min_x + pad * 2.0).max(640.0);
    let h = (max_y - min_y + pad * 2.0 + head).max(360.0);
    let dx = pad - min_x;
    let dy = pad + head - min_y;

    let mut svg = String::with_capacity(8192);
    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.0}" height="{h:.0}" viewBox="0 0 {w:.0} {h:.0}">
<rect width="100%" height="100%" fill="{CANVAS_BG}"/>
<text x="{pad:.0}" y="38" font-family="sans-serif" font-size="17" fill="{INK}" font-weight="600">{}</text>
"##,
        esc(title)
    ));

    // 连线画在卡片之下
    for edge in &graph.edges {
        let from = graph.nodes.iter().find(|n| n.id == edge.source);
        let to = graph.nodes.iter().find(|n| n.id == edge.target);
        let (Some(from), Some(to)) = (from, to) else {
            continue;
        };
        let from_h = card_height(&from.data);
        let to_h = card_height(&to.data);
        // 谁更近就走哪条边：横向排列接左右边，纵向排列接上下边
        let ddx = (to.position.x - from.position.x).abs();
        let ddy = (to.position.y - from.position.y).abs();
        let (fx, fyy, tx, tyy) = if ddx >= ddy {
            let fy = from.position.y + from_h / 2.0;
            let ty = to.position.y + to_h / 2.0;
            let x1 = if to.position.x >= from.position.x {
                from.position.x + CARD_W
            } else {
                from.position.x
            };
            let x2 = if to.position.x >= from.position.x {
                to.position.x
            } else {
                to.position.x + CARD_W
            };
            (x1 + dx, fy + dy, x2 + dx, ty + dy)
        } else {
            let fx = from.position.x + CARD_W / 2.0;
            let tx = to.position.x + CARD_W / 2.0;
            let y1 = if to.position.y >= from.position.y {
                from.position.y + from_h
            } else {
                from.position.y
            };
            let y2 = if to.position.y >= from.position.y {
                to.position.y
            } else {
                to.position.y + to_h
            };
            (fx + dx, y1 + dy, tx + dx, y2 + dy)
        };
        // 控制点沿主轴偏移，纵向排列时弯曲方向跟着换
        let (c1x, c1y, c2x, c2y) = if ddx >= ddy {
            let bend = ((tx - fx).abs() * 0.4).max(40.0);
            (fx + bend, fyy, tx - bend, tyy)
        } else {
            let bend = ((tyy - fyy).abs() * 0.4).max(32.0);
            (fx, fyy + bend, tx, tyy - bend)
        };
        svg.push_str(&format!(
            r##"<path d="M {fx:.1} {fyy:.1} C {c1x:.1} {c1y:.1}, {c2x:.1} {c2y:.1}, {tx:.1} {tyy:.1}" fill="none" stroke="{EDGE}" stroke-width="1.6"/>
<circle cx="{tx:.1}" cy="{tyy:.1}" r="3.2" fill="{EDGE}"/>
"##
        ));
        if !edge.label.is_empty() {
            let mx = (fx + tx) / 2.0;
            let my = (fyy + tyy) / 2.0;
            let label = wrap(&edge.label, 10.0, 140.0, 1);
            if let Some(text) = label.first() {
                let tw = text_width(text, 10.0);
                svg.push_str(&format!(
                    r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="16" rx="3" fill="{CANVAS_BG}" fill-opacity="0.92"/>
<text x="{:.1}" y="{:.1}" font-family="sans-serif" font-size="10" fill="{MUTED}" text-anchor="middle">{}</text>
"##,
                    mx - tw / 2.0 - 5.0,
                    my - 8.0,
                    tw + 10.0,
                    mx,
                    my + 4.0,
                    esc(text)
                ));
            }
        }
    }

    for node in &graph.nodes {
        let x = node.position.x + dx;
        let y = node.position.y + dy;
        let h_card = card_height(&node.data);
        let accent = kind_color(&node.data.kind);

        svg.push_str(&format!(
            r##"<g>
<rect x="{x:.1}" y="{y:.1}" width="{CARD_W}" height="{h_card:.1}" rx="5" fill="{CARD_BG}" stroke="{CARD_RULE}" stroke-width="1"/>
<rect x="{x:.1}" y="{y:.1}" width="3" height="{h_card:.1}" rx="1.5" fill="{accent}"/>
"##
        ));

        let title_lines = wrap(&node.data.label, 13.0, CARD_W - 26.0, 2);
        let mut ty = y + 20.0;
        for line in &title_lines {
            svg.push_str(&format!(
                r##"<text x="{:.1}" y="{ty:.1}" font-family="sans-serif" font-size="13" font-weight="600" fill="{INK}">{}</text>
"##,
                x + 14.0,
                esc(line)
            ));
            ty += 16.0;
        }
        svg.push_str(&format!(
            r##"<text x="{:.1}" y="{ty:.1}" font-family="sans-serif" font-size="10" fill="{MUTED}">{}</text>
"##,
            x + 14.0,
            esc(kind_label(&node.data.kind))
        ));

        let mut ry = y + CARD_HEADER + 8.0;
        let mut rows: Vec<(String, String)> = Vec::new();
        if !node.data.ip.is_empty() {
            rows.push(("IP".into(), node.data.ip.clone()));
        }
        if !node.data.domain.is_empty() {
            rows.push(("域名".into(), node.data.domain.clone()));
        }
        if !node.data.os.is_empty() {
            rows.push(("系统".into(), node.data.os.clone()));
        }
        if !node.data.notes.is_empty() {
            rows.push(("备注".into(), node.data.notes.clone()));
        }
        for field in &node.data.fields {
            if !field.key.is_empty() {
                rows.push((field.key.clone(), field.value.clone()));
            }
        }

        for (key, value) in rows {
            let text = if value.is_empty() {
                key.clone()
            } else {
                format!("{key}  {value}")
            };
            let line = wrap(&text, 10.0, CARD_W - 28.0, 1);
            if let Some(shown) = line.first() {
                svg.push_str(&format!(
                    r##"<text x="{:.1}" y="{ry:.1}" font-family="sans-serif" font-size="10" fill="{MUTED}">{}</text>
"##,
                    x + 14.0,
                    esc(shown)
                ));
            }
            ry += LINE_H;
        }

        if !node.data.vulns.is_empty() {
            let label = format!("{} 条漏洞", node.data.vulns.len());
            svg.push_str(&format!(
                r##"<text x="{:.1}" y="{:.1}" font-family="sans-serif" font-size="10" fill="{accent}" text-anchor="end">{}</text>
"##,
                x + CARD_W - 12.0,
                y + h_card - 8.0,
                esc(&label)
            ));
        }

        svg.push_str("</g>\n");
    }

    svg.push_str("</svg>");
    (svg, w.ceil() as u32, h.ceil() as u32)
}

/// 报告的一节内容
#[derive(Clone)]
pub struct ReportMeta {
    pub project: String,
    pub node: String,
    pub kind: String,
    pub ip: String,
    pub port: String,
    pub domain: String,
    pub status: String,
    pub notes: String,
    pub generated_at: String,
    pub author: String,
}

fn risk_label(risk: &str) -> &'static str {
    match risk {
        "critical" => "严重",
        "high" => "高",
        "medium" => "中",
        "low" => "低",
        _ => "提示",
    }
}

/// 把 HTML 详情拆成纯文本行与图片引用
fn detail_blocks(html: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut rest = html;
    loop {
        match rest.find("<img") {
            None => {
                push_text(&mut blocks, rest);
                break;
            }
            Some(at) => {
                push_text(&mut blocks, &rest[..at]);
                let tail = &rest[at..];
                let close = tail.find('>').map(|i| i + 1).unwrap_or(tail.len());
                if let Some(src) = attr(tail, "src") {
                    blocks.push(Block::Image(src));
                }
                rest = &tail[close..];
            }
        }
    }
    blocks
}

fn attr(tag: &str, name: &str) -> Option<String> {
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

enum Block {
    Line(String),
    Image(String),
}

fn push_text(blocks: &mut Vec<Block>, chunk: &str) {
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
            blocks.push(Block::Line(line));
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

/// 报告页 SVG，按页切成多张；每张就是 PDF 的一页
pub fn report_pages_svg(meta: &ReportMeta, vulns: &[Vulnerability]) -> Vec<String> {
    let mut ops: Vec<Op> = Vec::new();
    ops.push(Op::Space(24.0));
    ops.push(Op::H1(meta.node.clone()));
    ops.push(Op::Meta(format!(
        "{} / {}",
        meta.project,
        kind_label(&meta.kind)
    )));
    ops.push(Op::Space(16.0));

    let mut facts: Vec<(String, String)> = Vec::new();
    let addr = match (meta.ip.is_empty(), meta.port.is_empty()) {
        (false, false) => format!("{}:{}", meta.ip, meta.port),
        (false, true) => meta.ip.clone(),
        (true, false) => meta.port.clone(),
        (true, true) => String::new(),
    };
    if !addr.is_empty() {
        facts.push(("IP / 端口".into(), addr));
    }
    if !meta.domain.is_empty() {
        facts.push(("域名".into(), meta.domain.clone()));
    }
    if !meta.notes.is_empty() {
        facts.push(("备注".into(), meta.notes.clone()));
    }
    facts.push(("漏洞数量".into(), format!("{} 条", vulns.len())));
    let top = vulns
        .iter()
        .map(|v| v.risk.as_str())
        .max_by_key(|r| match *r {
            "critical" => 4,
            "high" => 3,
            "medium" => 2,
            "low" => 1,
            _ => 0,
        })
        .unwrap_or("");
    if !top.is_empty() {
        facts.push(("最高风险".into(), risk_label(top).into()));
    }
    facts.push(("报告生成".into(), meta.generated_at.clone()));
    if !meta.author.is_empty() {
        facts.push(("导出人".into(), meta.author.clone()));
    }
    ops.push(Op::Facts(facts));
    ops.push(Op::Space(14.0));

    if vulns.is_empty() {
        ops.push(Op::Body("该资产暂无漏洞记录。".into()));
    }

    for (index, vuln) in vulns.iter().enumerate() {
        ops.push(Op::Space(8.0));
        ops.push(Op::H2(format!(
            "{}. {}  {}",
            index + 1,
            if vuln.name.is_empty() {
                "未命名漏洞"
            } else {
                &vuln.name
            },
            risk_label(&vuln.risk)
        )));
        if !vuln.description.is_empty() {
            ops.push(Op::Label("漏洞说明".into()));
            for line in wrap(&vuln.description, 10.5, 660.0, 40) {
                ops.push(Op::Body(line));
            }
        }
        if !vuln.detail.is_empty() {
            ops.push(Op::Label("渗透测试详情".into()));
            for block in detail_blocks(&vuln.detail) {
                match block {
                    Block::Line(line) => {
                        for part in wrap(&line, 10.5, 660.0, 40) {
                            ops.push(Op::Body(part));
                        }
                    }
                    Block::Image(src) => ops.push(Op::Image(src)),
                }
            }
        }
        if !vuln.remediation.is_empty() {
            ops.push(Op::Label("处置建议".into()));
            for line in vuln.remediation.split('\n') {
                if line.trim().is_empty() {
                    continue;
                }
                for part in wrap(line.trim(), 10.5, 660.0, 40) {
                    ops.push(Op::Body(part));
                }
            }
        }
    }

    ops.push(Op::Space(16.0));
    ops.push(Op::Footer(
        "本报告由 RedTeam Atlas 生成，内容以项目内记录为准。".into(),
    ));

    paginate(&ops)
}

enum Op {
    Space(f64),
    H1(String),
    H2(String),
    Meta(String),
    Label(String),
    Body(String),
    Facts(Vec<(String, String)>),
    Image(String),
    Footer(String),
}

/// 把指令流按 A4 高度切页；图片放不下就整块挪到下一页
fn paginate(ops: &[Op]) -> Vec<String> {
    let margin = 64.0;
    let content_w = PAGE_W - margin * 2.0;
    let mut pages: Vec<String> = Vec::new();
    let mut body = String::new();
    let mut y = margin;

    fn flush(body: &mut String, pages: &mut Vec<String>) {
        pages.push(format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{PAGE_W}" height="{PAGE_H}" viewBox="0 0 {PAGE_W} {PAGE_H}">
<rect width="100%" height="100%" fill="{PAPER}"/>{body}</svg>"##
        ));
        body.clear();
    }

    for op in ops {
        let need = match op {
            Op::Space(h) => {
                y += h;
                continue;
            }
            Op::H1(_) => 34.0,
            Op::H2(_) => 28.0,
            Op::Meta(_) => 18.0,
            Op::Label(_) => 22.0,
            Op::Body(_) => 17.0,
            Op::Facts(rows) => rows.len() as f64 * 22.0 + 8.0,
            Op::Image(_) => 220.0,
            Op::Footer(_) => 30.0,
        };
        if y + need > PAGE_H - margin {
            flush(&mut body, &mut pages);
            y = margin;
        }
        match op {
            Op::H1(text) => {
                for line in wrap(text, 22.0, content_w, 2) {
                    body.push_str(&format!(
                        r##"<text x="{margin:.0}" y="{:.0}" font-family="sans-serif" font-size="22" font-weight="700" fill="{REPORT_INK}">{}</text>"##,
                        y + 22.0,
                        esc(&line)
                    ));
                    y += 28.0;
                }
                y += 6.0;
            }
            Op::Meta(text) => {
                body.push_str(&format!(
                    r##"<text x="{margin:.0}" y="{:.0}" font-family="sans-serif" font-size="11" fill="{REPORT_MUTED}">{}</text>"##,
                    y + 12.0,
                    esc(text)
                ));
                y += 18.0;
            }
            Op::H2(text) => {
                body.push_str(&format!(
                    r##"<text x="{margin:.0}" y="{:.0}" font-family="sans-serif" font-size="14" font-weight="600" fill="{REPORT_INK}">{}</text>"##,
                    y + 16.0,
                    esc(text)
                ));
                y += 22.0;
                body.push_str(&format!(
                    r##"<line x1="{margin:.0}" y1="{y:.0}" x2="{:.0}" y2="{y:.0}" stroke="{REPORT_RULE}" stroke-width="1"/>"##,
                    PAGE_W - margin
                ));
                y += 10.0;
            }
            Op::Label(text) => {
                body.push_str(&format!(
                    r##"<text x="{margin:.0}" y="{:.0}" font-family="sans-serif" font-size="11" font-weight="600" fill="{REPORT_BODY}">{}</text>"##,
                    y + 13.0,
                    esc(text)
                ));
                y += 20.0;
            }
            Op::Body(text) => {
                body.push_str(&format!(
                    r##"<text x="{margin:.0}" y="{:.0}" font-family="sans-serif" font-size="10.5" fill="{REPORT_BODY}">{}</text>"##,
                    y + 12.0,
                    esc(text)
                ));
                y += 17.0;
            }
            Op::Facts(rows) => {
                let box_h = rows.len() as f64 * 22.0 + 8.0;
                body.push_str(&format!(
                    r##"<rect x="{margin:.0}" y="{y:.0}" width="{content_w:.0}" height="{box_h:.0}" rx="3" fill="{REPORT_BAND}" stroke="{REPORT_RULE}" stroke-width="1"/>"##
                ));
                let mut ry = y + 20.0;
                for (key, value) in rows {
                    body.push_str(&format!(
                        r##"<text x="{:.0}" y="{ry:.0}" font-family="sans-serif" font-size="10" fill="{REPORT_FAINT}">{}</text><text x="{:.0}" y="{ry:.0}" font-family="sans-serif" font-size="10.5" fill="{REPORT_INK}">{}</text>"##,
                        margin + 14.0,
                        esc(key),
                        margin + 110.0,
                        esc(value)
                    ));
                    ry += 22.0;
                }
                y += box_h;
            }
            Op::Image(src) => {
                body.push_str(&format!(
                    r##"<rect x="{margin:.0}" y="{y:.0}" width="{content_w:.0}" height="200" rx="3" fill="{REPORT_SLOT}" stroke="{REPORT_RULE}" stroke-width="1"/><image x="{margin:.0}" y="{y:.0}" width="{content_w:.0}" height="200" preserveAspectRatio="xMidYMid meet" href="{}"/>"##,
                    esc(src)
                ));
                y += 214.0;
            }
            Op::Footer(text) => {
                body.push_str(&format!(
                    r##"<line x1="{margin:.0}" y1="{y:.0}" x2="{:.0}" y2="{y:.0}" stroke="{REPORT_RULE}" stroke-width="1"/><text x="{margin:.0}" y="{:.0}" font-family="sans-serif" font-size="9" fill="{REPORT_FAINT}">{}</text>"##,
                    PAGE_W - margin,
                    y + 16.0,
                    esc(text)
                ));
                y += 30.0;
            }
            Op::Space(_) => {}
        }
    }
    if !body.is_empty() {
        flush(&mut body, &mut pages);
    }
    if pages.is_empty() {
        flush(&mut body, &mut pages);
    }
    pages
}

/// 风险等级供外部复用
pub fn risk_text(risk: &str) -> &'static str {
    risk_label(risk)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::graph::{EdgeData, GraphEdge, GraphNode, NodeData, Position};

    fn sample() -> Graph {
        Graph {
            nodes: vec![
                GraphNode {
                    id: "a".into(),
                    position: Position { x: 0.0, y: 0.0 },
                    node_type: "asset".into(),
                    data: NodeData {
                        label: "外网门户".into(),
                        kind: "web".into(),
                        ip: "203.0.113.10".into(),
                        port: "443".into(),
                        ..Default::default()
                    },
                },
                GraphNode {
                    id: "b".into(),
                    position: Position { x: 360.0, y: 20.0 },
                    node_type: "asset".into(),
                    data: NodeData {
                        label: "数据库".into(),
                        kind: "database".into(),
                        ip: "10.0.3.50".into(),
                        ..Default::default()
                    },
                },
            ],
            edges: vec![GraphEdge {
                id: "e1".into(),
                source: "a".into(),
                target: "b".into(),
                label: "内网横向".into(),
                data: EdgeData::default(),
                marker_end: String::new(),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn canvas_svg_has_nodes_edges_and_single_hash_colors() {
        let (svg, w, h) = canvas_svg(&sample(), "测试");
        println!("--- SVG ({w}x{h}) ---\n{svg}");
        assert!(svg.contains("外网门户"), "节点名丢了");
        assert!(svg.contains("数据库"), "节点名丢了");
        assert!(svg.contains("<g>"), "节点分组丢了");
        assert!(svg.contains("<path"), "连线丢了");
        assert!(svg.contains("内网横向"), "边标签丢了");
        // 颜色必须只有一个井号，否则 SVG 解析会把整层丢掉
        assert!(!svg.contains("##"), "出现双井号颜色，SVG 会失效");
        assert!(svg.contains("fill=\"#0a0e14\""), "画布底色不对");
    }

    #[test]
    fn report_pages_split_and_carry_meta() {
        let meta = ReportMeta {
            project: "测试项目".into(),
            node: "外网门户".into(),
            kind: "web".into(),
            ip: "203.0.113.10".into(),
            port: "443".into(),
            domain: String::new(),
            status: "owned".into(),
            notes: String::new(),
            generated_at: "2026-11-24 12:00".into(),
            author: "管理员".into(),
        };
        let vulns = vec![Vulnerability {
            name: "任意文件上传".into(),
            risk: "critical".into(),
            description: "上传接口未校验后缀".into(),
            detail: "<p>步骤一</p><p><img src=\"/api/pub/uploads/x.png\"></p>".into(),
            remediation: "白名单校验".into(),
            ..Default::default()
        }];
        let pages = report_pages_svg(&meta, &vulns);
        assert!(!pages.is_empty());
        let joined = pages.join("");
        assert!(joined.contains("外网门户"));
        assert!(joined.contains("任意文件上传"));
        assert!(joined.contains("<image"), "详情里的图没留占位");
        assert!(!joined.contains("##"), "出现双井号颜色");
    }
}