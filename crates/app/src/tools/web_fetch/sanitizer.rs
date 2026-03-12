#[cfg(feature = "tool-webfetch")]
const DANGEROUS_TAGS: &[&str] = &[
    "script", "style", "noscript", "iframe", "object", "embed", "svg", "canvas", "template",
    "meta", "link",
];

#[cfg(feature = "tool-webfetch")]
pub(crate) fn sanitize_html(html: &str) -> String {
    let without_dangerous = remove_dangerous_elements(html);
    let without_hidden = remove_hidden_elements(&without_dangerous);
    strip_invisible_unicode(&without_hidden)
}

#[cfg(feature = "tool-webfetch")]
pub(crate) fn remove_dangerous_elements(html: &str) -> String {
    let mut output = html.to_owned();
    for tag in DANGEROUS_TAGS {
        output = remove_tag_block(&output, tag);
    }
    output
}

#[cfg(feature = "tool-webfetch")]
pub(crate) fn remove_hidden_elements(html: &str) -> String {
    let mut output = String::with_capacity(html.len());

    for line in html.lines() {
        let lowered = line.to_ascii_lowercase();
        if lowered.contains("aria-hidden=\"true\"")
            || lowered.contains("aria-hidden='true'")
            || lowered.contains(" hidden")
            || lowered.contains("display:none")
            || lowered.contains("visibility:hidden")
            || lowered.contains("opacity:0")
        {
            continue;
        }
        output.push_str(line);
        output.push('\n');
    }

    output
}

#[cfg(feature = "tool-webfetch")]
pub(crate) fn strip_invisible_unicode(input: &str) -> String {
    input
        .chars()
        .filter(|ch| {
            !matches!(
                *ch as u32,
                0x200B | 0x200C | 0x200D | 0x2060 | 0xFEFF | 0x202A..=0x202E | 0x2066..=0x2069
            )
        })
        .collect()
}

#[cfg(feature = "tool-webfetch")]
fn remove_tag_block(input: &str, tag: &str) -> String {
    let mut source = input.to_owned();
    loop {
        let lower = source.to_ascii_lowercase();
        let open_pat = format!("<{tag}");
        let close_pat = format!("</{tag}>");

        let Some(open_idx) = lower.find(&open_pat) else {
            break;
        };
        let search_from = open_idx + open_pat.len();
        let Some(open_end_rel) = lower[search_from..].find('>') else {
            source.replace_range(open_idx..source.len(), "");
            break;
        };
        let open_end = search_from + open_end_rel + 1;

        let remove_end = if let Some(close_rel) = lower[open_end..].find(&close_pat) {
            open_end + close_rel + close_pat.len()
        } else {
            open_end
        };
        source.replace_range(open_idx..remove_end, "");
    }
    source
}

#[cfg(all(test, feature = "tool-webfetch"))]
mod tests {
    use super::{
        remove_dangerous_elements, remove_hidden_elements, sanitize_html, strip_invisible_unicode,
    };

    #[test]
    fn remove_dangerous_elements_strips_script_and_style_blocks() {
        let html = r#"<div>ok</div><script>alert(1)</script><style>p{}</style><p>done</p>"#;
        let cleaned = remove_dangerous_elements(html);
        assert!(cleaned.contains("<div>ok</div>"));
        assert!(cleaned.contains("<p>done</p>"));
        assert!(!cleaned.contains("<script"));
        assert!(!cleaned.contains("<style"));
    }

    #[test]
    fn remove_hidden_elements_drops_hidden_lines() {
        let html = "<p>visible</p>\n<div aria-hidden=\"true\">secret</div>\n<span style=\"display:none\">x</span>\n";
        let cleaned = remove_hidden_elements(html);
        assert!(cleaned.contains("visible"));
        assert!(!cleaned.contains("secret"));
        assert!(!cleaned.contains("display:none"));
    }

    #[test]
    fn strip_invisible_unicode_removes_zero_width_chars() {
        let input = "a\u{200B}b\u{2066}c\u{2069}d\u{FEFF}e";
        assert_eq!(strip_invisible_unicode(input), "abcde");
    }

    #[test]
    fn sanitize_html_applies_all_stages() {
        let html = "<p>hi</p>\n<script>boom</script>\n<div aria-hidden=\"true\">x</div>\n<p>z\u{200B}</p>\n";
        let cleaned = sanitize_html(html);
        assert!(cleaned.contains("<p>hi</p>"));
        assert!(cleaned.contains("<p>z</p>"));
        assert!(!cleaned.contains("script"));
        assert!(!cleaned.contains("aria-hidden"));
        assert!(!cleaned.contains('\u{200B}'));
    }
}
