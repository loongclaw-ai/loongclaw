#[cfg(feature = "tool-webfetch")]
use scraper::{Html, Selector};

#[cfg(feature = "tool-webfetch")]
pub(crate) fn extract_title(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("title").ok()?;
    let raw = document
        .select(&selector)
        .next()?
        .text()
        .collect::<String>();
    let normalized = normalize_whitespace(&raw);
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

#[cfg(feature = "tool-webfetch")]
pub(crate) fn extract_main_content(html: &str) -> String {
    let document = Html::parse_document(html);
    let article_selector = Selector::parse("article, main").ok();
    if let Some(selector) = article_selector
        && let Some(node) = document.select(&selector).next()
    {
        let text = node.text().collect::<Vec<_>>().join(" ");
        let normalized = normalize_whitespace(&text);
        if !normalized.is_empty() {
            return normalized;
        }
    }

    let body_selector = Selector::parse("body").ok();
    if let Some(selector) = body_selector
        && let Some(node) = document.select(&selector).next()
    {
        let text = node.text().collect::<Vec<_>>().join(" ");
        let normalized = normalize_whitespace(&text);
        if !normalized.is_empty() {
            return normalized;
        }
    }

    normalize_whitespace(&document.root_element().text().collect::<Vec<_>>().join(" "))
}

#[cfg(feature = "tool-webfetch")]
pub(crate) fn html_to_markdown(html: &str) -> String {
    let document = Html::parse_document(html);
    let mut lines = Vec::new();

    if let Some(title) = extract_title(html) {
        lines.push(format!("# {title}"));
        lines.push(String::new());
    }

    let selector = Selector::parse("article, main, p, h1, h2, h3, h4, h5, h6, li").ok();
    if let Some(selector) = selector {
        for node in document.select(&selector) {
            let tag = node.value().name();
            let text = normalize_whitespace(&node.text().collect::<Vec<_>>().join(" "));
            if text.is_empty() {
                continue;
            }
            match tag {
                "h1" => lines.push(format!("# {text}")),
                "h2" => lines.push(format!("## {text}")),
                "h3" => lines.push(format!("### {text}")),
                "h4" => lines.push(format!("#### {text}")),
                "h5" => lines.push(format!("##### {text}")),
                "h6" => lines.push(format!("###### {text}")),
                "li" => lines.push(format!("- {text}")),
                _ => lines.push(text),
            }
            if matches!(tag, "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
                lines.push(String::new());
            }
        }
    }

    let markdown = lines.join("\n");
    let collapsed = collapse_blank_lines(&markdown);
    if collapsed.trim().is_empty() {
        extract_main_content(html)
    } else {
        collapsed.trim().to_owned()
    }
}

#[cfg(feature = "tool-webfetch")]
pub(crate) fn markdown_to_text(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        let cleaned = trimmed
            .trim_start_matches('#')
            .trim_start_matches('-')
            .trim_start_matches('*')
            .trim_start_matches(|ch: char| ch.is_ascii_digit() || ch == '.')
            .trim();
        if cleaned.is_empty() {
            out.push('\n');
        } else {
            out.push_str(cleaned);
            out.push('\n');
        }
    }
    normalize_whitespace_lines(&out)
}

#[cfg(feature = "tool-webfetch")]
fn normalize_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(feature = "tool-webfetch")]
fn normalize_whitespace_lines(input: &str) -> String {
    let mut lines = Vec::new();
    for line in input.lines() {
        let normalized = normalize_whitespace(line);
        if !normalized.is_empty() {
            lines.push(normalized);
        }
    }
    lines.join("\n")
}

#[cfg(feature = "tool-webfetch")]
fn collapse_blank_lines(input: &str) -> String {
    let mut output = String::new();
    let mut previous_blank = false;
    for line in input.lines() {
        let is_blank = line.trim().is_empty();
        if is_blank {
            if !previous_blank {
                output.push('\n');
            }
            previous_blank = true;
            continue;
        }
        output.push_str(line);
        output.push('\n');
        previous_blank = false;
    }
    output
}

#[cfg(all(test, feature = "tool-webfetch"))]
mod tests {
    use super::{extract_main_content, extract_title, html_to_markdown, markdown_to_text};

    #[test]
    fn extract_title_reads_document_title() {
        let html = "<html><head><title>Example Title</title></head><body></body></html>";
        assert_eq!(extract_title(html).as_deref(), Some("Example Title"));
    }

    #[test]
    fn extract_main_content_prefers_article() {
        let html = r#"
            <html><body>
                <header>Navigation</header>
                <article><h1>Hello</h1><p>World</p></article>
            </body></html>
        "#;
        assert_eq!(extract_main_content(html), "Hello World");
    }

    #[test]
    fn html_to_markdown_generates_headings_and_paragraphs() {
        let html = r#"
            <html>
                <head><title>Doc</title></head>
                <body><main><h2>Section</h2><p>Paragraph text</p><ul><li>One</li></ul></main></body>
            </html>
        "#;
        let markdown = html_to_markdown(html);
        assert!(markdown.contains("# Doc"));
        assert!(markdown.contains("## Section"));
        assert!(markdown.contains("Paragraph text"));
        assert!(markdown.contains("- One"));
    }

    #[test]
    fn markdown_to_text_strips_basic_markers() {
        let markdown = "# Title\n\n- item\n## Sub\nText";
        let text = markdown_to_text(markdown);
        assert_eq!(text, "Title\nitem\nSub\nText");
    }
}
