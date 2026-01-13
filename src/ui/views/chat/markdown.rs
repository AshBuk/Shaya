// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/ShyQuiet

//! Native Markdown renderer using GTK TextBuffer tags
//!
//! Supports:
//! - Code blocks (```...```)
//! - Inline code (`...`)
//! - Bold (**...**)
//! - Italic (*...*)
//! - Headers (#, ##, ###)

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
            // Check for header
            else if let Some(header) = self.parse_header_line(line) {
                if i > 0 {
                    self.insert_plain("\n");
                }
                self.insert_with_tag(&header.text, header.tag);
                self.insert_plain("\n");
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

    /// Parse inline formatting: bold, italic, inline code
    fn parse_inline(&mut self, text: &str) {
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let mut i = 0;
        let mut plain_start = 0;

        while i < len {
            // Inline code: `...`
            if chars[i] == '`' {
                // Flush plain text before this
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
            // Bold: **...**
            else if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
                // Flush plain text before this
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

                // Flush plain text before this
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

#[cfg(test)]
mod tests {
    use super::*;

    fn render_to_string(markdown: &str) -> String {
        let tag_table = create_tag_table();
        let buffer = TextBuffer::new(Some(&tag_table));
        render(&buffer, markdown);
        buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string()
    }

    #[test]
    fn test_plain_text() {
        let result = render_to_string("Hello world");
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_code_block() {
        let result = render_to_string("```\nlet x = 1;\n```");
        assert_eq!(result, "let x = 1;");
    }

    #[test]
    fn test_inline_code() {
        let result = render_to_string("Use `println!` macro");
        assert_eq!(result, "Use println! macro");
    }
}
