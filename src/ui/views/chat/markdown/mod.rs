// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Markdown in a GTK TextView. `parse` turns text into tagged spans,
//! this module inserts them and opens links on click.

mod parse;

pub use parse::{split_blocks, Block};

use gtk4::prelude::*;
use gtk4::{gdk, glib, pango, TextBuffer, TextTag, TextTagTable, TextView, TextWindowType};
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;

/// Link targets of a rendered buffer, by character offset.
type Links = Rc<RefCell<Vec<Link>>>;

struct Link {
    start: i32,
    end: i32,
    url: String,
}

/// Theme colors, matched to the palette in `ui/theme.rs`.
struct Colors {
    code_block_bg: &'static str,
    code_bg: &'static str,
    accent: &'static str,
    muted: &'static str,
    rule: &'static str,
}

fn colors(is_dark: bool) -> Colors {
    if is_dark {
        Colors {
            code_block_bg: "rgba(0, 0, 0, 0.28)",
            code_bg: "rgba(255, 255, 255, 0.09)",
            accent: "#e7bc7c",
            muted: "#beb19f",
            rule: "rgba(255, 255, 255, 0.22)",
        }
    } else {
        Colors {
            code_block_bg: "rgba(0, 0, 0, 0.05)",
            code_bg: "rgba(0, 0, 0, 0.07)",
            accent: "#7a4b18",
            muted: "#6e5e4a",
            rule: "rgba(0, 0, 0, 0.25)",
        }
    }
}

fn update_theme(table: &TextTagTable, is_dark: bool) {
    let c = colors(is_dark);
    let tag = |name: &str| table.lookup(name);
    if let Some(t) = tag("code-block") {
        t.set_background(Some(c.code_block_bg));
        t.set_paragraph_background(Some(c.code_block_bg));
    }
    if let Some(t) = tag("code") {
        t.set_background(Some(c.code_bg));
    }
    for name in ["link", "list-marker", "quote-bar"] {
        if let Some(t) = tag(name) {
            t.set_foreground(Some(c.accent));
        }
    }
    for name in ["blockquote", "h6"] {
        if let Some(t) = tag(name) {
            t.set_foreground(Some(c.muted));
        }
    }
    for name in ["hr", "table-rule"] {
        if let Some(t) = tag(name) {
            t.set_foreground(Some(c.rule));
        }
    }
}

/// Create the tag table for markdown. Its colors follow the system style while it lives.
/// Tags added later win when they overlap.
pub fn create_tag_table() -> TextTagTable {
    let table = TextTagTable::new();
    let heading = |name: &str, scale: f64, above: i32| {
        TextTag::builder()
            .name(name)
            .weight(700)
            .scale(scale)
            .pixels_above_lines(above)
            .pixels_below_lines(2)
            .build()
    };
    let margin = |name: &str, px: i32| TextTag::builder().name(name).left_margin(px).build();

    for tag in [
        TextTag::builder().name("gap").scale(0.5).build(),
        TextTag::builder()
            .name("code-block")
            .family("monospace")
            .scale(0.93)
            .left_margin(12)
            .right_margin(12)
            .build(),
        TextTag::builder()
            .name("code")
            .family("monospace")
            .scale(0.93)
            .build(),
        TextTag::builder().name("bold").weight(700).build(),
        TextTag::builder()
            .name("italic")
            .style(pango::Style::Italic)
            .build(),
        TextTag::builder()
            .name("strikethrough")
            .strikethrough(true)
            .build(),
        TextTag::builder()
            .name("link")
            .underline(pango::Underline::Single)
            .build(),
        margin("blockquote", 4),
        TextTag::builder().name("quote-bar").weight(700).build(),
        margin("list-item", 12),
        margin("list-item-2", 30),
        margin("list-item-3", 48),
        TextTag::builder().name("list-marker").weight(700).build(),
        TextTag::builder()
            .name("hr")
            .justification(gtk4::Justification::Center)
            .build(),
        heading("h1", 1.4, 6),
        heading("h2", 1.25, 5),
        heading("h3", 1.1, 4),
        heading("h4", 1.0, 3),
        heading("h5", 1.0, 3),
        heading("h6", 1.0, 3),
        TextTag::builder()
            .name("table")
            .family("monospace")
            .scale(0.93)
            .left_margin(8)
            .build(),
        TextTag::builder().name("table-rule").build(),
    ] {
        table.add(&tag);
    }

    let style = adw::StyleManager::default();
    update_theme(&table, style.is_dark());
    // `watch` disconnects the handler once the table is gone.
    style.connect_closure(
        "notify::dark",
        false,
        glib::closure_local!(
            #[watch]
            table,
            move |style: adw::StyleManager, _: glib::ParamSpec| {
                update_theme(table, style.is_dark());
            }
        ),
    );
    table
}

/// A read-only TextView that shows markdown with working links.
#[derive(Clone)]
pub struct MarkdownView {
    view: TextView,
    links: Links,
}

impl MarkdownView {
    /// The view's buffer must use the table from `create_tag_table`.
    pub fn new(view: &TextView) -> Self {
        Self {
            view: view.clone(),
            links: connect_links(view),
        }
    }

    /// Replace the view's text with rendered markdown.
    pub fn render(&self, text: &str) {
        *self.links.borrow_mut() = render(&self.view.buffer(), text);
    }
}

fn render(buffer: &TextBuffer, text: &str) -> Vec<Link> {
    buffer.set_text("");
    let mut links = Vec::new();
    let mut end = buffer.end_iter();
    for span in parse::spans(text) {
        let start = end.offset();
        buffer.insert_with_tags_by_name(&mut end, &span.text, &span.tags);
        if let Some(url) = span.url {
            links.push(Link {
                start,
                end: end.offset(),
                url,
            });
        }
    }
    links
}

/// Open links in the view on click, with a pointer cursor and the URL as tooltip.
fn connect_links(view: &TextView) -> Links {
    let links = Links::default();

    let click = gtk4::GestureClick::new();
    click.set_button(gdk::BUTTON_PRIMARY);
    let l = links.clone();
    click.connect_released(move |gesture, _, x, y| {
        let Some(view) = gesture.widget().and_downcast::<TextView>() else {
            return;
        };
        // Releasing a drag selection over a link must not open it.
        if view.buffer().has_selection() {
            return;
        }
        if let Some(url) = link_at(&view, &l.borrow(), x, y) {
            open(&view, &url);
        }
    });
    view.add_controller(click);

    let motion = gtk4::EventControllerMotion::new();
    let l = links.clone();
    motion.connect_motion(move |motion, x, y| {
        if let Some(view) = motion.widget().and_downcast::<TextView>() {
            let over = link_at(&view, &l.borrow(), x, y).is_some();
            view.set_cursor_from_name(Some(if over { "pointer" } else { "text" }));
        }
    });
    view.add_controller(motion);

    let l = links.clone();
    view.set_has_tooltip(true);
    view.connect_query_tooltip(move |view, x, y, keyboard, tooltip| {
        if keyboard {
            return false;
        }
        let Some(url) = link_at(view, &l.borrow(), x.into(), y.into()) else {
            return false;
        };
        tooltip.set_text(Some(&url));
        true
    });

    links
}

fn link_at(view: &TextView, links: &[Link], x: f64, y: f64) -> Option<String> {
    let (bx, by) = view.window_to_buffer_coords(TextWindowType::Widget, x as i32, y as i32);
    let offset = view.iter_at_location(bx, by)?.offset();
    links
        .iter()
        .find(|link| (link.start..link.end).contains(&offset))
        .map(|link| link.url.clone())
}

/// `parse` already drops other schemes; this guards the launcher itself.
fn open(view: &TextView, url: &str) {
    if !parse::is_openable(url) {
        tracing::warn!("Not opening link with unsupported scheme: {url}");
        return;
    }
    let window = view.root().and_downcast::<gtk4::Window>();
    gtk4::show_uri(window.as_ref(), url, gdk::CURRENT_TIME);
}
