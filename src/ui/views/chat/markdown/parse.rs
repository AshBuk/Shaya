// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Markdown to tagged spans, without GTK. Tag names match the tag table in `mod.rs`.

use pulldown_cmark::{
    Alignment, CodeBlockKind, Event, Options, Parser, Tag, TagEnd, TextMergeStream,
};
use std::mem;

/// A run of text with the tags it carries and, for a link, its target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub tags: Vec<&'static str>,
    pub url: Option<String>,
}

/// A part of a message: code blocks get their own surface in the chat.
#[derive(Debug, PartialEq, Eq)]
pub enum Block {
    Text(String),
    Code { lang: String, code: String },
}

const RULE: &str = "────────────────────────────────";

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
}

/// Split markdown into text and top-level fenced code blocks.
pub fn split_blocks(text: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut text_start = 0;
    let mut depth = 0usize;
    let mut code: Option<(String, String)> = None;

    for (event, range) in Parser::new_ext(text, options()).into_offset_iter() {
        match &event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) if depth == 0 => {
                push_text_block(&mut blocks, &text[text_start..range.start]);
                let lang = info.split_whitespace().next().unwrap_or_default();
                code = Some((lang.to_string(), String::new()));
                text_start = range.end;
            }
            Event::Text(t) => {
                if let Some((_, body)) = &mut code {
                    body.push_str(t);
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((lang, body)) = code.take() {
                    let body = body.trim_end_matches('\n');
                    if !body.is_empty() {
                        blocks.push(Block::Code {
                            lang,
                            code: body.to_string(),
                        });
                    }
                }
            }
            _ => {}
        }
        match event {
            Event::Start(_) => depth += 1,
            Event::End(_) => depth -= 1,
            _ => {}
        }
    }
    push_text_block(&mut blocks, &text[text_start..]);
    blocks
}

fn push_text_block(blocks: &mut Vec<Block>, text: &str) {
    let text = text.trim_matches('\n');
    if !text.trim().is_empty() {
        blocks.push(Block::Text(text.to_string()));
    }
}

/// Turn markdown into spans. Blocks are separated by a short blank line (`gap`).
pub fn spans(text: &str) -> Vec<Span> {
    let mut r = Renderer {
        line_start: true,
        ..Renderer::default()
    };
    for event in TextMergeStream::new(Parser::new_ext(text, options())) {
        r.event(event);
    }
    r.out
}

/// What separates the next span from the previous one.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Break {
    #[default]
    None,
    Line,
    Gap,
}

#[derive(Default)]
struct Table {
    aligns: Vec<Alignment>,
    rows: Vec<Vec<Vec<Span>>>,
    head: bool,
}

#[derive(Default)]
struct Renderer {
    out: Vec<Span>,
    blocks: Vec<&'static str>,
    inline: Vec<&'static str>,
    /// Target of the current link, if it may be opened.
    link: Option<String>,
    /// Target of the current image until its alt text shows.
    image: Option<String>,
    /// Open lists, each with the number of its next item.
    lists: Vec<Option<u64>>,
    /// Marker of the current list item, written before its first text.
    marker: Option<String>,
    quotes: usize,
    in_code: bool,
    table: Option<Table>,
    pending: Break,
    line_start: bool,
}

impl Renderer {
    fn event(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(t) if self.in_code => self.push_code(&t),
            Event::Text(t) if self.link.is_some() => self.push(&t, &[]),
            Event::Text(t) => self.push_linkified(&t),
            Event::Code(t) => self.push(&t, &["code"]),
            Event::SoftBreak | Event::HardBreak => self.line_break(),
            Event::InlineHtml(h) if is_br(&h) => self.line_break(),
            Event::Html(h) => {
                self.push(h.trim_end_matches('\n'), &[]);
                self.brk(Break::Line);
            }
            Event::InlineHtml(h) => self.push(&h, &[]),
            Event::Rule => {
                self.brk(Break::Gap);
                self.push(RULE, &["hr"]);
                self.brk(Break::Gap);
            }
            Event::TaskListMarker(done) => {
                let check = if done { "☑ " } else { "☐ " };
                self.marker = Some(match self.lists.last() {
                    Some(Some(_)) => format!("{}{check}", self.marker.take().unwrap_or_default()),
                    _ => check.to_string(),
                });
            }
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Heading { level, .. } => {
                self.brk(Break::Gap);
                self.blocks.push(match level as u8 {
                    1 => "h1",
                    2 => "h2",
                    3 => "h3",
                    4 => "h4",
                    5 => "h5",
                    _ => "h6",
                });
            }
            Tag::BlockQuote(_) => {
                self.brk(Break::Gap);
                self.quotes += 1;
                self.blocks.push("blockquote");
            }
            Tag::CodeBlock(_) => {
                self.brk(Break::Gap);
                self.in_code = true;
                self.blocks.push("code-block");
            }
            Tag::List(first) => {
                self.brk(Break::Line);
                self.lists.push(first);
            }
            Tag::Item => {
                self.flush_marker();
                self.brk(Break::Line);
                let depth = self.lists.len();
                self.blocks.push(match depth {
                    0 | 1 => "list-item",
                    2 => "list-item-2",
                    _ => "list-item-3",
                });
                self.marker = Some(match self.lists.last_mut() {
                    Some(Some(n)) => {
                        *n += 1;
                        format!("{}. ", *n - 1)
                    }
                    _ => match depth {
                        0 | 1 => "• ",
                        2 => "◦ ",
                        _ => "▪ ",
                    }
                    .to_string(),
                });
            }
            Tag::Table(aligns) => {
                self.brk(Break::Gap);
                self.blocks.push("table");
                self.table = Some(Table {
                    aligns,
                    ..Table::default()
                });
            }
            Tag::TableHead => {
                self.inline.push("bold");
                if let Some(table) = &mut self.table {
                    table.head = true;
                    table.rows.push(Vec::new());
                }
            }
            Tag::TableRow => {
                if let Some(table) = &mut self.table {
                    table.rows.push(Vec::new());
                }
            }
            Tag::TableCell => {
                if let Some(row) = self.table.as_mut().and_then(|t| t.rows.last_mut()) {
                    row.push(Vec::new());
                }
            }
            Tag::Emphasis => self.inline.push("italic"),
            Tag::Strong => self.inline.push("bold"),
            Tag::Strikethrough => self.inline.push("strikethrough"),
            Tag::Link { dest_url, .. } => self.link = openable(dest_url.into_string()),
            Tag::Image { dest_url, .. } => {
                self.link = openable(dest_url.to_string());
                self.image = Some(dest_url.into_string());
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.brk(if self.lists.is_empty() {
                Break::Gap
            } else {
                Break::Line
            }),
            TagEnd::Heading(_) | TagEnd::CodeBlock => {
                self.in_code = false;
                self.blocks.pop();
                self.brk(Break::Gap);
            }
            TagEnd::BlockQuote(_) => {
                self.quotes -= 1;
                self.blocks.pop();
                self.brk(Break::Gap);
            }
            TagEnd::HtmlBlock => self.brk(Break::Gap),
            TagEnd::List(_) => {
                self.lists.pop();
                self.brk(if self.lists.is_empty() {
                    Break::Gap
                } else {
                    Break::Line
                });
            }
            TagEnd::Item => {
                self.flush_marker();
                self.blocks.pop();
                self.brk(Break::Line);
            }
            TagEnd::Table => {
                if let Some(table) = self.table.take() {
                    self.layout_table(table);
                }
                self.blocks.pop();
                self.brk(Break::Gap);
            }
            TagEnd::TableHead => {
                self.inline.pop();
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.inline.pop();
            }
            TagEnd::Link => self.link = None,
            TagEnd::Image => {
                // Without alt text, the address is all there is to show.
                if let Some(url) = self.image.take() {
                    self.push(&url, &[]);
                }
                self.link = None;
            }
            _ => {}
        }
    }

    fn brk(&mut self, b: Break) {
        self.pending = self.pending.max(b);
    }

    fn line_break(&mut self) {
        if self.table.is_some() {
            self.push(" ", &[]);
        } else {
            self.brk(Break::Line);
        }
    }

    fn span(&self, text: &str, extra: &[&'static str]) -> Span {
        Span {
            text: text.to_string(),
            tags: self
                .blocks
                .iter()
                .chain(&self.inline)
                .chain(extra)
                .copied()
                .chain(self.link.as_ref().map(|_| "link"))
                .collect(),
            url: self.link.clone(),
        }
    }

    fn push(&mut self, text: &str, extra: &[&'static str]) {
        if !text.is_empty() {
            self.image = None;
            let span = self.span(text, extra);
            self.emit(span);
        }
    }

    /// Bare URLs in text become links.
    fn push_linkified(&mut self, text: &str) {
        let mut rest = text;
        while let Some((start, len)) = find_url(rest) {
            self.push(&rest[..start], &[]);
            let url = &rest[start..start + len];
            self.link = Some(url.to_string());
            self.push(url, &[]);
            self.link = None;
            rest = &rest[start + len..];
        }
        self.push(rest, &[]);
    }

    fn emit(&mut self, span: Span) {
        if let Some(table) = &mut self.table {
            if let Some(cell) = table.rows.last_mut().and_then(|r| r.last_mut()) {
                cell.push(span);
            }
            return;
        }
        self.flush_break();
        if let Some(marker) = self.marker.take() {
            let mut tags = self.blocks.clone();
            tags.push("list-marker");
            self.out.push(Span {
                text: marker,
                tags,
                url: None,
            });
        }
        self.out.push(span);
    }

    /// Write the pending separator, then the quote bars of a new line.
    fn flush_break(&mut self) {
        let pending = mem::take(&mut self.pending);
        if !self.out.is_empty() && pending != Break::None {
            self.out.push(bare("\n", &[]));
            if pending == Break::Gap {
                self.out.push(bare("\n", &["gap"]));
            }
            self.line_start = true;
        }
        if mem::take(&mut self.line_start) {
            for _ in 0..self.quotes {
                let mut tags = self.blocks.clone();
                tags.push("quote-bar");
                self.out.push(Span {
                    text: "▍ ".to_string(),
                    tags,
                    url: None,
                });
            }
        }
    }

    /// Code is one span, so inside a quote it is split to give each line its bar.
    fn push_code(&mut self, code: &str) {
        let code = code.strip_suffix('\n').unwrap_or(code);
        if self.quotes == 0 {
            return self.push(code, &[]);
        }
        for (i, line) in code.split('\n').enumerate() {
            if i > 0 {
                self.out.push(bare("\n", &[]));
                self.line_start = true;
            }
            self.flush_break();
            self.push(line, &[]);
        }
    }

    /// An item with no text of its own still shows its marker.
    fn flush_marker(&mut self) {
        if self.marker.is_some() {
            self.push(" ", &[]);
        }
    }

    /// Pad cells to column width, so the monospace table lines up.
    fn layout_table(&mut self, table: Table) {
        let width = |cell: &[Span]| cell.iter().map(|s| s.text.chars().count()).sum::<usize>();
        let cols = table.rows.iter().map(Vec::len).max().unwrap_or(0);
        let mut widths = vec![0; cols];
        for row in &table.rows {
            for (w, cell) in widths.iter_mut().zip(row) {
                *w = (*w).max(width(cell));
            }
        }

        let rows = table.rows.len();
        for (i, row) in table.rows.into_iter().enumerate() {
            self.brk(Break::Line);
            let mut cells = row.into_iter();
            for (j, &w) in widths.iter().enumerate() {
                if j > 0 {
                    self.push(" │ ", &["table-rule"]);
                }
                let cell = cells.next().unwrap_or_default();
                let pad = w - width(&cell);
                let (left, right) = match table.aligns.get(j) {
                    Some(Alignment::Right) => (pad, 0),
                    Some(Alignment::Center) => (pad / 2, pad - pad / 2),
                    _ => (0, pad),
                };
                self.push(&" ".repeat(left), &[]);
                cell.into_iter().for_each(|s| self.emit(s));
                if j + 1 < cols {
                    self.push(&" ".repeat(right), &[]);
                }
            }
            if i == 0 && table.head && rows > 1 {
                self.brk(Break::Line);
                let rule: Vec<_> = widths.iter().map(|&w| "─".repeat(w)).collect();
                self.push(&rule.join("─┼─"), &["table-rule"]);
            }
        }
    }
}

fn bare(text: &str, tags: &[&'static str]) -> Span {
    Span {
        text: text.to_string(),
        tags: tags.to_vec(),
        url: None,
    }
}

/// Model output decides link targets, so only web and mail links open.
pub(super) fn is_openable(url: &str) -> bool {
    ["https://", "http://", "mailto:"].iter().any(|scheme| {
        url.get(..scheme.len())
            .is_some_and(|s| s.eq_ignore_ascii_case(scheme))
    })
}

fn openable(url: String) -> Option<String> {
    is_openable(&url).then_some(url)
}

fn is_br(html: &str) -> bool {
    let tag: String = html.chars().filter(|c| !c.is_whitespace()).collect();
    matches!(tag.to_ascii_lowercase().as_str(), "<br>" | "<br/>")
}

/// Find a bare http(s) URL: returns its byte start and length.
fn find_url(text: &str) -> Option<(usize, usize)> {
    text.match_indices("http").find_map(|(start, _)| {
        let tail = &text[start..];
        let scheme = ["https://", "http://"]
            .into_iter()
            .find(|s| tail.starts_with(s))?;
        let boundary = text[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let len = url_len(tail);
        (boundary && len > scheme.len()).then_some((start, len))
    })
}

/// Length of the URL at the start of `text`, without trailing punctuation.
fn url_len(text: &str) -> usize {
    let end = text
        .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '`'))
        .unwrap_or(text.len());
    let mut url = &text[..end];
    loop {
        let unbalanced = url.ends_with(')') && url.matches('(').count() < url.matches(')').count();
        match url.chars().next_back() {
            Some('.' | ',' | ';' | ':' | '!' | '?' | '\'' | '*' | '_') => {}
            Some(')') if unbalanced => {}
            _ => return url.len(),
        }
        url = &url[..url.len() - 1];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(spans: &[Span]) -> String {
        spans.iter().map(|s| s.text.as_str()).collect()
    }

    /// Spans that carry a tag, as text.
    fn tagged(spans: &[Span], tag: &str) -> Vec<String> {
        spans
            .iter()
            .filter(|s| s.tags.contains(&tag))
            .map(|s| s.text.clone())
            .collect()
    }

    fn links(spans: &[Span]) -> Vec<(String, String)> {
        spans
            .iter()
            .filter_map(|s| Some((s.text.clone(), s.url.clone()?)))
            .collect()
    }

    #[test]
    fn headings_and_rules() {
        let s = spans("#### Title\n\n---\n\nAfter");

        assert_eq!(tagged(&s, "h4"), ["Title"]);
        assert_eq!(tagged(&s, "hr"), [RULE]);
        assert_eq!(text(&s), format!("Title\n\n{RULE}\n\nAfter"));
    }

    #[test]
    fn soft_break_is_a_line_break() {
        assert_eq!(text(&spans("one\ntwo")), "one\ntwo");
    }

    #[test]
    fn escapes_are_not_formatting() {
        let s = spans(r"\*not italic\*");

        assert_eq!(text(&s), "*not italic*");
        assert!(tagged(&s, "italic").is_empty());
    }

    #[test]
    fn nested_formatting() {
        let s = spans("**bold _and italic_**");

        assert_eq!(tagged(&s, "bold"), ["bold ", "and italic"]);
        assert_eq!(tagged(&s, "italic"), ["and italic"]);
    }

    #[test]
    fn links_keep_parentheses_in_url() {
        let s = spans("[text](https://a.b/x_(y)) and `code`");

        assert_eq!(links(&s), [("text".into(), "https://a.b/x_(y)".into())]);
        assert_eq!(tagged(&s, "link"), ["text"]);
        assert_eq!(tagged(&s, "code"), ["code"]);
    }

    #[test]
    fn only_web_and_mail_links_open() {
        for src in [
            "[x](file:///etc/passwd)",
            "[x](docs/a.md)",
            "[x](javascript:alert)",
        ] {
            let s = spans(src);
            assert!(links(&s).is_empty(), "{src}");
            assert!(tagged(&s, "link").is_empty(), "{src}");
            assert_eq!(text(&s), "x");
        }
        assert_eq!(
            links(&spans("[mail](MAILTO:a@b.c)")),
            [("mail".into(), "MAILTO:a@b.c".into())]
        );
    }

    #[test]
    fn images_show_alt_or_address() {
        let url = "https://a.b/i.png";

        assert_eq!(
            links(&spans(&format!("![]({url})"))),
            [(url.into(), url.into())]
        );
        assert_eq!(
            links(&spans(&format!("![cat]({url})"))),
            [("cat".into(), url.into())]
        );
        assert_eq!(text(&spans("![](file:///x.png)")), "file:///x.png");
    }

    #[test]
    fn bare_urls_become_links() {
        let s = spans("See https://example.com/a_(b), or (http://x.io/p).");

        assert_eq!(
            links(&s),
            [
                (
                    "https://example.com/a_(b)".into(),
                    "https://example.com/a_(b)".into()
                ),
                ("http://x.io/p".into(), "http://x.io/p".into()),
            ]
        );
        assert!(links(&spans("`https://in.code`")).is_empty());
        assert!(links(&spans("nothttps://x.y")).is_empty());
    }

    #[test]
    fn nested_lists() {
        let s = spans("- one\n  - two\n    - three\n      - four\n- five");

        assert_eq!(text(&s), "• one\n◦ two\n▪ three\n▪ four\n• five");
        assert_eq!(tagged(&s, "list-item-3"), ["▪ ", "three", "▪ ", "four"]);
    }

    #[test]
    fn numbering_starts_from_source() {
        let s = spans("Intro\n\n3. three\n4. four\n\nAfter");

        assert_eq!(text(&s), "Intro\n\n3. three\n4. four\n\nAfter");
        assert_eq!(tagged(&s, "gap"), ["\n", "\n"]);
    }

    #[test]
    fn task_lists() {
        assert_eq!(text(&spans("- [ ] todo\n- [x] done")), "☐ todo\n☑ done");
    }

    #[test]
    fn blockquotes_get_a_bar_per_line() {
        let s = spans("> one\n> two");

        assert_eq!(text(&s), "▍ one\n▍ two");
        assert_eq!(tagged(&s, "blockquote"), ["▍ ", "one", "▍ ", "two"]);
    }

    #[test]
    fn code_in_quotes_has_a_bar_per_line() {
        let s = spans("> ```\n> a\n>\n> b\n> ```");

        assert_eq!(text(&s), "▍ a\n▍ \n▍ b");
        assert_eq!(tagged(&s, "quote-bar").len(), 3);
    }

    #[test]
    fn indented_code_keeps_lines() {
        let s = spans("Code:\n\n    let a = 1;\n    let b = 2;\n");

        assert_eq!(tagged(&s, "code-block"), ["let a = 1;\nlet b = 2;"]);
    }

    #[test]
    fn tables_are_aligned_text() {
        let s = spans("Name | Value\n--- | ---:\n**A** | 1\nLonger | 22");

        assert_eq!(
            text(&s),
            "Name   │ Value\n───────┼──────\nA      │     1\nLonger │    22"
        );
        assert_eq!(tagged(&s, "bold"), ["Name", "Value", "A"]);
        assert!(s
            .iter()
            .filter(|span| span.text != "\n")
            .all(|span| span.tags.contains(&"table")));
    }

    #[test]
    fn splits_messages_around_code_blocks() {
        let blocks = split_blocks("Before\n\n```rust title\nfn main() {}\n```\n\nAfter\n```\nopen");

        assert_eq!(
            blocks,
            vec![
                Block::Text("Before".to_string()),
                Block::Code {
                    lang: "rust".to_string(),
                    code: "fn main() {}".to_string(),
                },
                Block::Text("After".to_string()),
                Block::Code {
                    lang: String::new(),
                    code: "open".to_string(),
                },
            ]
        );
    }

    #[test]
    fn code_inside_lists_stays_in_text() {
        let src = "1. Run:\n   ```sh\n   make\n   ```\n2. Done";

        assert_eq!(split_blocks(src), [Block::Text(src.to_string())]);
    }
}
