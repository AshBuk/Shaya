// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Native Markdown renderer using GTK TextBuffer tags
//!
//! Supports:
//! - Code blocks (```...```)
//! - Inline code (`...`)
//! - Bold (**...**)
//! - Italic (*...*)
//! - Strikethrough (~~...~~)
//! - Headers (#, ##, ###, ####, #####, ######)
//! - Bullet lists (-, *, +) with nesting
//! - Numbered lists (1., 2., etc.) with nesting
//! - Blockquotes (> ...)
//! - Horizontal rules (---, ***, ___)
//! - Links [text](url) (rendered as underlined text)
//! - Tables (| col | col |)
//! - Combined formatting (**_bold italic_**)

use gtk4::prelude::*;
use gtk4::{TextBuffer, TextTag, TextTagTable};

/// Theme-dependent colors: (code_bg, code_fg, inline_bg, inline_fg)
fn code_colors(is_dark: bool) -> (&'static str, &'static str, &'static str, &'static str) {
    if is_dark {
        ("#2d2d2d", "#d4d4d4", "#3d3d3d", "#e0e0e0")
    } else {
        ("#eaeaea", "#1e1e1e", "#e0e0e0", "#1e1e1e")
    }
}

/// Update code tag colors when theme changes
pub fn update_theme(table: &TextTagTable, is_dark: bool) {
    let (code_bg, code_fg, inline_bg, inline_fg) = code_colors(is_dark);
    if let Some(tag) = table.lookup("code-block") {
        tag.set_background(Some(code_bg));
        tag.set_foreground(Some(code_fg));
        tag.set_paragraph_background(Some(code_bg));
    }
    if let Some(tag) = table.lookup("code") {
        tag.set_background(Some(inline_bg));
        tag.set_foreground(Some(inline_fg));
    }
}

/// Create tag table with all markdown formatting tags (theme-aware)
pub fn create_tag_table(is_dark: bool) -> TextTagTable {
    let table = TextTagTable::new();

    let (code_bg, code_fg, inline_bg, inline_fg) = code_colors(is_dark);

    let code_block = TextTag::builder()
        .name("code-block")
        .family("monospace")
        .background(code_bg)
        .foreground(code_fg)
        .paragraph_background(code_bg)
        .left_margin(12)
        .right_margin(12)
        .pixels_above_lines(0)
        .pixels_below_lines(0)
        .pixels_inside_wrap(0)
        .build();
    table.add(&code_block);

    let code_inline = TextTag::builder()
        .name("code")
        .family("monospace")
        .background(inline_bg)
        .foreground(inline_fg)
        .build();
    table.add(&code_inline);

    // Bold
    let bold = TextTag::builder().name("bold").weight(700).build();
    table.add(&bold);

    // Italic
    let italic = TextTag::builder()
        .name("italic")
        .style(gtk4::pango::Style::Italic)
        .build();
    table.add(&italic);

    // Strikethrough
    let strikethrough = TextTag::builder()
        .name("strikethrough")
        .strikethrough(true)
        .build();
    table.add(&strikethrough);

    // Link: underlined, colored
    let link = TextTag::builder()
        .name("link")
        .foreground("#58a6ff")
        .underline(gtk4::pango::Underline::Single)
        .build();
    table.add(&link);

    // Blockquote: left border effect via margin + italic + muted color
    let blockquote = TextTag::builder()
        .name("blockquote")
        .left_margin(16)
        .foreground(if is_dark { "#9e9e9e" } else { "#757575" })
        .style(gtk4::pango::Style::Italic)
        .pixels_above_lines(2)
        .pixels_below_lines(2)
        .build();
    table.add(&blockquote);

    // List item: indented
    let list_item = TextTag::builder().name("list-item").left_margin(16).build();
    table.add(&list_item);

    // Horizontal rule: muted color
    let hr = TextTag::builder()
        .name("hr")
        .foreground(if is_dark { "#555555" } else { "#cccccc" })
        .justification(gtk4::Justification::Center)
        .build();
    table.add(&hr);

    // Headers (using scale for relative sizing)
    let h1 = TextTag::builder()
        .name("h1")
        .weight(700)
        .scale(1.4)
        .pixels_above_lines(4)
        .pixels_below_lines(1)
        .build();
    table.add(&h1);

    let h2 = TextTag::builder()
        .name("h2")
        .weight(700)
        .scale(1.25)
        .pixels_above_lines(3)
        .pixels_below_lines(1)
        .build();
    table.add(&h2);

    let h3 = TextTag::builder()
        .name("h3")
        .weight(700)
        .scale(1.1)
        .pixels_above_lines(2)
        .pixels_below_lines(1)
        .build();
    table.add(&h3);

    let h4 = TextTag::builder()
        .name("h4")
        .weight(700)
        .pixels_above_lines(2)
        .pixels_below_lines(1)
        .build();
    table.add(&h4);

    let h5 = TextTag::builder()
        .name("h5")
        .weight(700)
        .pixels_above_lines(2)
        .pixels_below_lines(1)
        .build();
    table.add(&h5);

    let h6 = TextTag::builder()
        .name("h6")
        .weight(700)
        .foreground(if is_dark { "#9e9e9e" } else { "#757575" })
        .pixels_above_lines(2)
        .pixels_below_lines(1)
        .build();
    table.add(&h6);

    // Nested list items
    let list_item_2 = TextTag::builder()
        .name("list-item-2")
        .left_margin(32)
        .build();
    table.add(&list_item_2);

    let list_item_3 = TextTag::builder()
        .name("list-item-3")
        .left_margin(48)
        .build();
    table.add(&list_item_3);

    // Table: monospace for alignment
    let table_tag = TextTag::builder()
        .name("table")
        .family("monospace")
        .left_margin(8)
        .pixels_above_lines(1)
        .pixels_below_lines(1)
        .build();
    table.add(&table_tag);

    table
}

/// Render markdown text into TextBuffer with formatting tags
pub fn render(buffer: &TextBuffer, text: &str) {
    buffer.set_text("");

    let mut parser = Parser::new(buffer);
    parser.parse(text);
}

/// Render markdown, appending to existing buffer content.
/// Returns the char offset where this chunk started.
pub fn render_append(buffer: &TextBuffer, text: &str) -> i32 {
    let start_offset = buffer.end_iter().offset();
    let mut parser = Parser::new(buffer);
    parser.parse(text);
    start_offset
}

/// Parser state machine for markdown
struct Parser<'a> {
    buffer: &'a TextBuffer,
}

impl<'a> Parser<'a> {
    fn new(buffer: &'a TextBuffer) -> Self {
        Self { buffer }
    }

    /// Main parse entry point
    fn parse(&mut self, text: &str) {
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            // Check for code block start
            if line.starts_with("```") {
                i = self.parse_code_block(&lines, i);
            }
            // Horizontal rule: ---, ***, ___
            else if self.is_horizontal_rule(line) {
                if i > 0 {
                    self.insert_plain("\n");
                }
                self.insert_with_tag("────────────────────────────────", "hr");
                self.insert_plain("\n");
                i += 1;
            }
            // Check for header
            else if let Some(header) = self.parse_header_line(line) {
                if i > 0 {
                    self.insert_plain("\n");
                }
                self.parse_inline_with_block_tag(&header.text, header.tag);
                i += 1;
            }
            // Blockquote: > text
            else if let Some(content) =
                line.strip_prefix("> ")
                    .or_else(|| if line == ">" { Some("") } else { None })
            {
                if i > 0 {
                    self.insert_plain("\n");
                }
                if content.is_empty() {
                    self.insert_with_tag(" ", "blockquote");
                } else {
                    self.parse_inline_with_block_tag(content, "blockquote");
                }
                i += 1;
            }
            // Bullet list: - item, * item, + item (with nesting)
            else if let Some((level, content)) = self.parse_bullet_line(line) {
                if i > 0 {
                    self.insert_plain("\n");
                }
                let (bullet, tag) = match level {
                    0 => ("• ", "list-item"),
                    1 => ("◦ ", "list-item-2"),
                    _ => ("▪ ", "list-item-3"),
                };
                self.insert_plain(bullet);
                self.parse_inline_with_block_tag(content, tag);
                i += 1;
            }
            // Numbered list: 1. item, 2. item (with nesting)
            else if let Some((level, num, content)) = self.parse_numbered_line(line) {
                if i > 0 {
                    self.insert_plain("\n");
                }
                let tag = match level {
                    0 => "list-item",
                    1 => "list-item-2",
                    _ => "list-item-3",
                };
                self.insert_plain(&format!("{}. ", num));
                self.parse_inline_with_block_tag(content, tag);
                i += 1;
            }
            // Table: | col | col |
            else if self.is_table_line(line) {
                if i > 0 {
                    self.insert_plain("\n");
                }
                i = self.parse_table(&lines, i);
            }
            // Regular line with inline formatting
            else {
                if i > 0 {
                    self.insert_plain("\n");
                }
                self.parse_inline(line);
                i += 1;
            }
        }
    }

    /// Check if line is a horizontal rule
    fn is_horizontal_rule(&self, line: &str) -> bool {
        is_horizontal_rule(line)
    }

    /// Parse bullet list line (- item, * item, + item)
    fn parse_bullet_line<'b>(&self, line: &'b str) -> Option<(usize, &'b str)> {
        parse_bullet_line(line)
    }

    /// Parse numbered list line (1. item, 2. item)
    fn parse_numbered_line<'b>(&self, line: &'b str) -> Option<(usize, &'b str, &'b str)> {
        parse_numbered_line(line)
    }

    /// Parse code block, returns next line index
    fn parse_code_block(&mut self, lines: &[&str], start: usize) -> usize {
        // Insert code block
        if start > 0 {
            self.insert_plain("\n");
        }

        let (code, next) = collect_code_block(lines, start);
        if !code.is_empty() {
            self.insert_with_tag(&code, "code-block");
        }

        next
    }

    /// Parse header line, returns (text, tag) if header found
    fn parse_header_line(&self, line: &str) -> Option<Header> {
        parse_header_line(line)
    }

    /// Parse inline formatting: bold, italic, inline code, strikethrough, links
    fn parse_inline(&mut self, text: &str) {
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let mut i = 0;
        let mut plain_start = 0;

        while i < len {
            // Inline code: `...`
            if chars[i] == '`' {
                if i > plain_start {
                    self.insert_plain(&chars[plain_start..i].iter().collect::<String>());
                }

                if let Some((code_text, end)) = self.extract_delimited(&chars, i + 1, '`') {
                    self.insert_with_tag(&code_text, "code");
                    i = end + 1;
                    plain_start = i;
                    continue;
                }
            }
            // Strikethrough: ~~...~~
            else if i + 1 < len && chars[i] == '~' && chars[i + 1] == '~' {
                if i > plain_start {
                    self.insert_plain(&chars[plain_start..i].iter().collect::<String>());
                }

                if let Some((st_text, end)) =
                    self.extract_double_delimited(&chars, i + 2, '~', false)
                {
                    self.parse_inline_with_block_tag(&st_text, "strikethrough");
                    i = end + 2;
                    plain_start = i;
                    continue;
                }
            }
            // Link: [text](url) — render text with link style
            else if chars[i] == '[' {
                if let Some((link_text, end)) = self.parse_link(&chars, i) {
                    if i > plain_start {
                        self.insert_plain(&chars[plain_start..i].iter().collect::<String>());
                    }
                    self.insert_with_tag(&link_text, "link");
                    i = end;
                    plain_start = i;
                    continue;
                }
            }
            // Bold: **...**
            else if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
                if i > plain_start {
                    self.insert_plain(&chars[plain_start..i].iter().collect::<String>());
                }

                if let Some((bold_text, end)) =
                    self.extract_double_delimited(&chars, i + 2, '*', false)
                {
                    self.parse_inline_with_block_tag(&bold_text, "bold");
                    i = end + 2;
                    plain_start = i;
                    continue;
                }
            }
            // Italic: *...* (single asterisk, not followed by another)
            else if chars[i] == '*' && (i + 1 >= len || chars[i + 1] != '*') {
                // Check it's not preceded by another * (would be end of bold)
                if i > 0 && chars[i - 1] == '*' {
                    i += 1;
                    continue;
                }

                if i > plain_start {
                    self.insert_plain(&chars[plain_start..i].iter().collect::<String>());
                }

                if let Some((italic_text, end)) =
                    self.extract_single_delimited(&chars, i + 1, '*', false)
                {
                    self.parse_inline_with_block_tag(&italic_text, "italic");
                    i = end + 1;
                    plain_start = i;
                    continue;
                }
            }
            // Bold: __...__
            else if i + 1 < len
                && chars[i] == '_'
                && chars[i + 1] == '_'
                && (i == 0 || !chars[i - 1].is_alphanumeric())
            {
                if i > plain_start {
                    self.insert_plain(&chars[plain_start..i].iter().collect::<String>());
                }

                if let Some((bold_text, end)) =
                    self.extract_double_delimited(&chars, i + 2, '_', true)
                {
                    self.parse_inline_with_block_tag(&bold_text, "bold");
                    i = end + 2;
                    plain_start = i;
                    continue;
                }
            }
            // Italic: _..._
            else if chars[i] == '_'
                && (i + 1 >= len || chars[i + 1] != '_')
                && (i == 0 || !chars[i - 1].is_alphanumeric())
            {
                if i > 0 && chars[i - 1] == '_' {
                    i += 1;
                    continue;
                }

                if i > plain_start {
                    self.insert_plain(&chars[plain_start..i].iter().collect::<String>());
                }

                if let Some((italic_text, end)) =
                    self.extract_single_delimited(&chars, i + 1, '_', true)
                {
                    self.parse_inline_with_block_tag(&italic_text, "italic");
                    i = end + 1;
                    plain_start = i;
                    continue;
                }
            }

            i += 1;
        }

        // Flush remaining plain text
        if plain_start < len {
            self.insert_plain(&chars[plain_start..].iter().collect::<String>());
        }
    }

    /// Parse [text](url) link, returns (display_text, end_index)
    fn parse_link(&self, chars: &[char], start: usize) -> Option<(String, usize)> {
        // Find closing ]
        let mut i = start + 1;
        while i < chars.len() && chars[i] != ']' {
            i += 1;
        }
        if i >= chars.len() {
            return None;
        }
        let text: String = chars[start + 1..i].iter().collect();
        if text.is_empty() {
            return None;
        }

        // Expect ( immediately after ]
        i += 1;
        if i >= chars.len() || chars[i] != '(' {
            return None;
        }

        // Find closing )
        while i < chars.len() && chars[i] != ')' {
            i += 1;
        }
        if i >= chars.len() {
            return None;
        }

        Some((text, i + 1))
    }

    /// Extract text between single delimiter: `code`
    fn extract_delimited(
        &self,
        chars: &[char],
        start: usize,
        delim: char,
    ) -> Option<(String, usize)> {
        let mut i = start;
        while i < chars.len() {
            if chars[i] == delim {
                let text: String = chars[start..i].iter().collect();
                return Some((text, i));
            }
            i += 1;
        }
        None
    }

    /// Extract text between double delimiter: **bold**, ~~strike~~
    /// When `boundary` is true, the char after closing delimiter must not be alphanumeric.
    fn extract_double_delimited(
        &self,
        chars: &[char],
        start: usize,
        delim: char,
        boundary: bool,
    ) -> Option<(String, usize)> {
        let mut i = start;
        while i + 1 < chars.len() {
            if chars[i] == delim
                && chars[i + 1] == delim
                && (!boundary || i + 2 >= chars.len() || !chars[i + 2].is_alphanumeric())
            {
                let text: String = chars[start..i].iter().collect();
                if !text.is_empty() {
                    return Some((text, i));
                }
            }
            i += 1;
        }
        None
    }

    /// Extract text between single delimiter for italic: *text*, _text_
    /// When `boundary` is true, the char after closing delimiter must not be alphanumeric.
    fn extract_single_delimited(
        &self,
        chars: &[char],
        start: usize,
        delim: char,
        boundary: bool,
    ) -> Option<(String, usize)> {
        let mut i = start;
        while i < chars.len() {
            if chars[i] == delim
                && (i + 1 >= chars.len() || chars[i + 1] != delim)
                && (!boundary || i + 1 >= chars.len() || !chars[i + 1].is_alphanumeric())
            {
                let text: String = chars[start..i].iter().collect();
                if !text.is_empty() {
                    return Some((text, i));
                }
            }
            i += 1;
        }
        None
    }

    /// Check if line is a table row
    fn is_table_line(&self, line: &str) -> bool {
        is_table_line(line)
    }

    /// Parse table block, returns next line index
    fn parse_table(&mut self, lines: &[&str], start: usize) -> usize {
        let mut i = start;
        let mut raw_rows = Vec::new();

        while i < lines.len() && self.is_table_line(lines[i]) {
            raw_rows.push(lines[i]);
            i += 1;
        }

        let formatted_rows = format_table_rows(&raw_rows);
        if formatted_rows.is_empty() {
            return i;
        }

        for (ri, row) in formatted_rows.iter().enumerate() {
            if ri > 0 || start > 0 {
                self.insert_plain("\n");
            }
            self.insert_with_tag(row, "table");
        }

        i
    }

    /// Parse inline formatting and wrap the entire result with a block-level tag
    fn parse_inline_with_block_tag(&mut self, text: &str, block_tag: &str) {
        let start_offset = self.buffer.end_iter().offset();
        self.parse_inline(text);
        let start = self.buffer.iter_at_offset(start_offset);
        let end = self.buffer.end_iter();
        self.buffer.apply_tag_by_name(block_tag, &start, &end);
    }

    /// Insert plain text without formatting
    fn insert_plain(&self, text: &str) {
        let mut end = self.buffer.end_iter();
        self.buffer.insert(&mut end, text);
    }

    /// Insert text with a specific tag
    fn insert_with_tag(&self, text: &str, tag_name: &str) {
        let start_offset = self.buffer.end_iter().offset();
        let mut end = self.buffer.end_iter();
        self.buffer.insert(&mut end, text);

        let start = self.buffer.iter_at_offset(start_offset);
        let end = self.buffer.end_iter();
        self.buffer.apply_tag_by_name(tag_name, &start, &end);
    }
}

/// Header parsing result
struct Header {
    text: String,
    tag: &'static str,
}

fn is_horizontal_rule(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 3 {
        return false;
    }
    let chars: Vec<char> = trimmed.chars().collect();
    let first = chars[0];
    (first == '-' || first == '*' || first == '_')
        && chars.iter().all(|&c| c == first || c == ' ')
        && chars.iter().filter(|&&c| c == first).count() >= 3
}

fn parse_bullet_line(line: &str) -> Option<(usize, &str)> {
    let indent = line.len() - line.trim_start().len();
    let trimmed = line.trim_start();
    let content = if let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("+ "))
    {
        rest
    } else if trimmed.starts_with("* ") && !trimmed.starts_with("**") {
        &trimmed[2..]
    } else {
        return None;
    };
    Some(((indent / 2).min(2), content))
}

fn parse_numbered_line(line: &str) -> Option<(usize, &str, &str)> {
    let indent = line.len() - line.trim_start().len();
    let trimmed = line.trim_start();
    let dot_pos = trimmed.find(". ")?;
    let num = &trimmed[..dot_pos];
    if num.chars().all(|c| c.is_ascii_digit()) && !num.is_empty() {
        Some(((indent / 2).min(2), num, &trimmed[dot_pos + 2..]))
    } else {
        None
    }
}

fn collect_code_block(lines: &[&str], start: usize) -> (String, usize) {
    let mut i = start + 1;
    let mut code_lines = Vec::new();

    while i < lines.len() {
        if lines[i].starts_with("```") {
            break;
        }
        code_lines.push(lines[i]);
        i += 1;
    }

    let next = if i < lines.len() && lines[i].starts_with("```") {
        i + 1
    } else {
        i
    };

    (code_lines.join("\n"), next)
}

fn parse_header_line(line: &str) -> Option<Header> {
    [
        ("###### ", "h6"),
        ("##### ", "h5"),
        ("#### ", "h4"),
        ("### ", "h3"),
        ("## ", "h2"),
        ("# ", "h1"),
    ]
    .iter()
    .find_map(|(prefix, tag)| {
        line.strip_prefix(prefix).map(|text| Header {
            text: text.to_string(),
            tag,
        })
    })
}

fn is_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.matches('|').count() >= 3
}

fn is_table_separator(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|')
        && trimmed.contains('-')
        && trimmed
            .chars()
            .all(|c| c == '|' || c == '-' || c == ':' || c == ' ')
}

fn format_table_rows(raw_rows: &[&str]) -> Vec<String> {
    let rows: Vec<Vec<String>> = raw_rows
        .iter()
        .filter(|line| !is_table_separator(line))
        .map(|line| {
            line.split('|')
                .map(|cell| cell.trim().to_string())
                .filter(|cell| !cell.is_empty())
                .collect()
        })
        .collect();

    if rows.is_empty() {
        return Vec::new();
    }

    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut col_widths = vec![0usize; num_cols];
    for row in &rows {
        for (j, cell) in row.iter().enumerate() {
            if j < num_cols {
                col_widths[j] = col_widths[j].max(cell.chars().count());
            }
        }
    }

    let mut formatted = Vec::new();
    for (ri, row) in rows.iter().enumerate() {
        formatted.push(
            (0..num_cols)
                .map(|j| {
                    let cell = row.get(j).map(|s| s.as_str()).unwrap_or("");
                    let pad = col_widths[j].saturating_sub(cell.chars().count());
                    format!("{}{}", cell, " ".repeat(pad))
                })
                .collect::<Vec<_>>()
                .join(" │ "),
        );

        if ri == 0 && rows.len() > 1 {
            formatted.push(
                col_widths
                    .iter()
                    .map(|&w| "─".repeat(w))
                    .collect::<Vec<_>>()
                    .join("─┼─"),
            );
        }
    }

    formatted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_markdown_block_markers() {
        let header = parse_header_line("#### Title").map(|h| (h.text, h.tag));

        assert_eq!(header, Some(("Title".to_string(), "h4")));
        assert!(is_horizontal_rule("---"));
        assert!(!is_horizontal_rule("--"));
    }

    #[test]
    fn parses_nested_lists() {
        assert_eq!(parse_bullet_line("- one"), Some((0, "one")));
        assert_eq!(parse_bullet_line("  - two"), Some((1, "two")));
        assert_eq!(parse_numbered_line("    1. deep"), Some((2, "1", "deep")));
    }

    #[test]
    fn collects_code_blocks_without_fence_markers() {
        let lines = ["```rust", "fn main() {}", "```", "After"];
        let (code, next) = collect_code_block(&lines, 0);

        assert_eq!(code, "fn main() {}");
        assert_eq!(next, 3);
    }

    #[test]
    fn formats_tables_as_aligned_text() {
        let rows = format_table_rows(&["| Name | Value |", "| --- | --- |", "| A | 1 |"]);

        assert_eq!(rows, vec!["Name │ Value", "─────┼──────", "A    │ 1    "]);
    }
}
