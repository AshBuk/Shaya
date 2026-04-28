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
//! - Headers (#, ##, ###)
//! - Bullet lists (-, *, +)
//! - Numbered lists (1., 2., etc.)
//! - Blockquotes (> ...)
//! - Horizontal rules (---, ***, ___)
//! - Links [text](url) (rendered as underlined text)

use gtk4::prelude::*;
use gtk4::{TextBuffer, TextTag, TextTagTable};

/// Create tag table with all markdown formatting tags
pub fn create_tag_table() -> TextTagTable {
    let table = TextTagTable::new();

    // Code block: dark background, monospace, compact lines
    let code_block = TextTag::builder()
        .name("code-block")
        .family("monospace")
        .background("#1e1e1e")
        .foreground("#d4d4d4")
        .paragraph_background("#1e1e1e")
        .left_margin(12)
        .right_margin(12)
        .pixels_above_lines(0)
        .pixels_below_lines(0)
        .pixels_inside_wrap(0)
        .build();
    table.add(&code_block);

    // Inline code: subtle background, monospace
    let code_inline = TextTag::builder()
        .name("code")
        .family("monospace")
        .background("#3d3d3d")
        .foreground("#e0e0e0")
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
        .foreground("#9e9e9e")
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
        .foreground("#555555")
        .justification(gtk4::Justification::Center)
        .build();
    table.add(&hr);

    // Headers (using scale for relative sizing)
    let h1 = TextTag::builder()
        .name("h1")
        .weight(700)
        .scale(1.4)
        .pixels_above_lines(8)
        .pixels_below_lines(4)
        .build();
    table.add(&h1);

    let h2 = TextTag::builder()
        .name("h2")
        .weight(700)
        .scale(1.25)
        .pixels_above_lines(6)
        .pixels_below_lines(3)
        .build();
    table.add(&h2);

    let h3 = TextTag::builder()
        .name("h3")
        .weight(700)
        .scale(1.1)
        .pixels_above_lines(4)
        .pixels_below_lines(2)
        .build();
    table.add(&h3);

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
                self.insert_plain("\n");
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
            // Bullet list: - item, * item, + item
            else if let Some(content) = self.parse_bullet_line(line) {
                if i > 0 {
                    self.insert_plain("\n");
                }
                self.insert_plain("• ");
                self.parse_inline_with_block_tag(content, "list-item");
                i += 1;
            }
            // Numbered list: 1. item, 2. item
            else if let Some((num, content)) = self.parse_numbered_line(line) {
                if i > 0 {
                    self.insert_plain("\n");
                }
                self.insert_plain(&format!("{}. ", num));
                self.parse_inline_with_block_tag(content, "list-item");
                i += 1;
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

    /// Parse bullet list line (- item, * item, + item)
    fn parse_bullet_line<'b>(&self, line: &'b str) -> Option<&'b str> {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("+ "))
        {
            Some(rest)
        } else if trimmed.starts_with("* ") && !trimmed.starts_with("**") {
            Some(&trimmed[2..])
        } else {
            None
        }
    }

    /// Parse numbered list line (1. item, 2. item)
    fn parse_numbered_line<'b>(&self, line: &'b str) -> Option<(&'b str, &'b str)> {
        let trimmed = line.trim_start();
        let dot_pos = trimmed.find(". ")?;
        let num = &trimmed[..dot_pos];
        if num.chars().all(|c| c.is_ascii_digit()) && !num.is_empty() {
            Some((num, &trimmed[dot_pos + 2..]))
        } else {
            None
        }
    }

    /// Parse code block, returns next line index
    fn parse_code_block(&mut self, lines: &[&str], start: usize) -> usize {
        // Skip opening ``` (possibly with language)
        let mut i = start + 1;
        let mut code_lines = Vec::new();

        // Collect lines until closing ```
        while i < lines.len() {
            if lines[i].starts_with("```") {
                break;
            }
            code_lines.push(lines[i]);
            i += 1;
        }

        // Insert code block
        if start > 0 {
            self.insert_plain("\n");
        }

        let code = code_lines.join("\n");
        if !code.is_empty() {
            self.insert_with_tag(&code, "code-block");
        }

        // Return index after closing ```
        if i < lines.len() && lines[i].starts_with("```") {
            i + 1
        } else {
            i
        }
    }

    /// Parse header line, returns (text, tag) if header found
    fn parse_header_line(&self, line: &str) -> Option<Header> {
        // Check in order: ### before ## before # (longest prefix first)
        [("### ", "h3"), ("## ", "h2"), ("# ", "h1")]
            .iter()
            .find_map(|(prefix, tag)| {
                line.strip_prefix(prefix).map(|text| Header {
                    text: text.to_string(),
                    tag,
                })
            })
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

                if let Some((st_text, end)) = self.extract_double_delimited(&chars, i + 2, '~') {
                    self.insert_with_tag(&st_text, "strikethrough");
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

                if let Some((bold_text, end)) = self.extract_double_delimited(&chars, i + 2, '*') {
                    self.insert_with_tag(&bold_text, "bold");
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

                if let Some((italic_text, end)) = self.extract_single_delimited(&chars, i + 1, '*')
                {
                    self.insert_with_tag(&italic_text, "italic");
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

    /// Extract text between double delimiter: **bold**
    fn extract_double_delimited(
        &self,
        chars: &[char],
        start: usize,
        delim: char,
    ) -> Option<(String, usize)> {
        let mut i = start;
        while i + 1 < chars.len() {
            if chars[i] == delim && chars[i + 1] == delim {
                let text: String = chars[start..i].iter().collect();
                return Some((text, i));
            }
            i += 1;
        }
        None
    }

    /// Extract text between single delimiter for italic (not double)
    fn extract_single_delimited(
        &self,
        chars: &[char],
        start: usize,
        delim: char,
    ) -> Option<(String, usize)> {
        let mut i = start;
        while i < chars.len() {
            // Found single delimiter (not double)
            if chars[i] == delim && (i + 1 >= chars.len() || chars[i + 1] != delim) {
                let text: String = chars[start..i].iter().collect();
                // Don't match empty content
                if !text.is_empty() {
                    return Some((text, i));
                }
            }
            i += 1;
        }
        None
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
