//! 轻量富文本转换：把 Agent 常用的 Markdown 写法转成详情区支持的 HTML。
//!
//! 不引第三方依赖——只需覆盖编辑器的工具栏能力：粗体、斜体、有序/无序列表、
//! 图片、段落与行内代码。已经是 HTML 的输入原样返回。

/// 判断一段文本是否已经是 HTML
fn looks_like_html(text: &str) -> bool {
    let head = text.trim_start();
    head.starts_with('<')
}

/// 行内格式：**粗体**、*斜体*、`代码`、![alt](url)
fn inline(line: &str) -> String {
    let mut out = String::with_capacity(line.len() + 16);
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // 图片：![alt](url)
        if chars[i] == '!' && i + 1 < chars.len() && chars[i + 1] == '[' {
            if let Some((alt, url, next)) = parse_link(&chars, i + 1) {
                out.push_str(&format!("<img src=\"{url}\" alt=\"{alt}\">"));
                i = next;
                continue;
            }
        }
        // 链接：[text](url) —— 详情区不放链接，退化成纯文本
        if chars[i] == '[' {
            if let Some((label, _url, next)) = parse_link(&chars, i) {
                out.push_str(&escape(&label));
                i = next;
                continue;
            }
        }
        // 行内代码 `code`
        if chars[i] == '`' {
            if let Some(end) = chars[i + 1..].iter().position(|c| *c == '`') {
                let code: String = chars[i + 1..i + 1 + end].iter().collect();
                out.push_str(&format!("<code>{}</code>", escape(&code)));
                i = i + end + 2;
                continue;
            }
        }
        // 粗体 **text**
        if chars[i] == '*' && i + 1 < chars.len() && chars[i + 1] == '*' {
            if let Some(end) = find(&chars, i + 2, "**") {
                let text: String = chars[i + 2..end].iter().collect();
                out.push_str(&format!("<b>{}</b>", inline(&text)));
                i = end + 2;
                continue;
            }
        }
        // 斜体 *text*
        if chars[i] == '*' {
            if let Some(end) = chars[i + 1..].iter().position(|c| *c == '*') {
                let text: String = chars[i + 1..i + 1 + end].iter().collect();
                if !text.trim().is_empty() {
                    out.push_str(&format!("<i>{}</i>", inline(&text)));
                    i = i + end + 2;
                    continue;
                }
            }
        }
        out.push_str(&escape(&chars[i].to_string()));
        i += 1;
    }
    out
}

/// 解析 `[text](url)`，返回 (text, url, 结束位置)
fn parse_link(chars: &[char], start: usize) -> Option<(String, String, usize)> {
    if chars.get(start)? != &'[' {
        return None;
    }
    let close = chars[start..].iter().position(|c| *c == ']')? + start;
    if chars.get(close + 1)? != &'(' {
        return None;
    }
    let paren = chars[close..].iter().position(|c| *c == ')')? + close;
    let text: String = chars[start + 1..close].iter().collect();
    let url: String = chars[close + 2..paren].iter().collect();
    Some((text, url, paren + 1))
}

/// 在 chars 里从 from 开始找子串
fn find(chars: &[char], from: usize, needle: &str) -> Option<usize> {
    let target: Vec<char> = needle.chars().collect();
    if target.is_empty() || from >= chars.len() {
        return None;
    }
    (from..=chars.len().saturating_sub(target.len()))
        .find(|&i| chars[i..i + target.len()] == target[..])
}

/// 转义 HTML 特殊字符
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Markdown → 详情区 HTML。
///
/// 支持：`**粗体**`、`*斜体*`、`` `代码` ``、`- 无序列`、`1. 有序列`、
/// `![说明](图片地址)`、空行分段。已经像 HTML 的输入原样返回。
pub fn markdown_to_html(input: &str) -> String {
    if input.trim().is_empty() || looks_like_html(input) {
        return input.to_string();
    }

    let mut out: Vec<String> = Vec::new();
    let mut list: Option<(&'static str, Vec<String>)> = None;

    let flush = |list: &mut Option<(&'static str, Vec<String>)>, out: &mut Vec<String>| {
        if let Some((tag, items)) = list.take() {
            let body = items.join("");
            out.push(format!("<{tag}>{body}</{tag}>"));
        }
    };

    for raw in input.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim_start();

        if trimmed.is_empty() {
            flush(&mut list, &mut out);
            continue;
        }

        // 无序列表：- * + 开头
        if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("+ "))
        {
            match &mut list {
                Some((tag, items)) if *tag == "ul" => items.push(format!("<li>{}</li>", inline(rest))),
                _ => {
                    flush(&mut list, &mut out);
                    list = Some(("ul", vec![format!("<li>{}</li>", inline(rest))]));
                }
            }
            continue;
        }

        // 有序列表：1. 2. 3.
        let ordered = {
            let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() && trimmed[digits.len()..].starts_with(". ") {
                Some(trimmed[digits.len() + 2..].to_string())
            } else {
                None
            }
        };
        if let Some(rest) = ordered {
            match &mut list {
                Some((tag, items)) if *tag == "ol" => items.push(format!("<li>{}</li>", inline(&rest))),
                _ => {
                    flush(&mut list, &mut out);
                    list = Some(("ol", vec![format!("<li>{}</li>", inline(&rest))]));
                }
            }
            continue;
        }

        // 单独一行的图片不包段落
        if trimmed.starts_with("![") && trimmed.ends_with(')') {
            flush(&mut list, &mut out);
            out.push(inline(trimmed));
            continue;
        }

        flush(&mut list, &mut out);
        out.push(format!("<p>{}</p>", inline(trimmed)));
    }
    flush(&mut list, &mut out);

    out.join("")
}

#[cfg(test)]
mod tests {
    use super::markdown_to_html;

    #[test]
    fn 已经_是_html_原样返回() {
        let html = "<p>已有内容</p><img src=\"/api/pub/uploads/x\">";
        assert_eq!(markdown_to_html(html), html);
    }

    #[test]
    fn 行内格式转成标签() {
        let out = markdown_to_html("这是 **粗体** 与 *斜体*，还有 `代码`。");
        assert!(out.contains("<b>粗体</b>"));
        assert!(out.contains("<i>斜体</i>"));
        assert!(out.contains("<code>代码</code>"));
        assert!(out.starts_with("<p>"));
    }

    #[test]
    fn 列表与编号() {
        let out = markdown_to_html("- 第一项\n- 第二项\n\n1. 步骤一\n2. 步骤二");
        assert!(out.contains("<ul><li>第一项</li><li>第二项</li></ul>"));
        assert!(out.contains("<ol><li>步骤一</li><li>步骤二</li></ol>"));
    }

    #[test]
    fn 图片单独成行不包段落() {
        let out = markdown_to_html("![复现截图](/api/pub/uploads/img_1?sig=abc)");
        assert_eq!(out, "<img src=\"/api/pub/uploads/img_1?sig=abc\" alt=\"复现截图\">");
    }

    #[test]
    fn 尖括号被转义() {
        let out = markdown_to_html("注入 <script>alert(1)</script> 试试");
        assert!(out.contains("&lt;script&gt;"));
        assert!(!out.contains("<script>"));
    }
}