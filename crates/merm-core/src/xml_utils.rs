/// Sanitizes and escapes arbitrary text to be valid inside SVG XML text and attribute nodes.
pub fn escape_xml(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 16);
    for c in input.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Splits label on HTML line breaks (<br>, <br/>, <br />) and returns escaped lines.
pub fn split_and_escape_lines(input: &str) -> Vec<String> {
    let normalized = input
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("<br>", "\n")
        .replace("\\n", "\n");

    normalized
        .lines()
        .map(|l| escape_xml(l.trim()))
        .filter(|l| !l.is_empty())
        .collect()
}
