// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Cheatsheets View - Quick-access markdown notes

use super::chat;
use crate::core::state::AppState;
use adw::prelude::*;
use gtk4::{Box as GtkBox, Button, Orientation, PolicyType, ScrolledWindow, TextView};
use libadwaita as adw;
use libadwaita::prelude::MessageDialogExt;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Build the cheatsheets view
pub fn build(state: Rc<RefCell<AppState>>) -> GtkBox {
    let vbox = GtkBox::new(Orientation::Vertical, 0);
    vbox.set_vexpand(true);

    let content = GtkBox::new(Orientation::Vertical, 0);
    content.set_vexpand(true);
    vbox.append(&content);

    let paned_pos = Rc::new(Cell::new(200i32));
    rebuild_content(&content, &state, &paned_pos);

    vbox
}

fn rebuild_content(container: &GtkBox, state: &Rc<RefCell<AppState>>, paned_pos: &Rc<Cell<i32>>) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }

    let state_ref = state.borrow();

    if state_ref.cheatsheets.is_empty() {
        let empty_box = GtkBox::new(Orientation::Vertical, 12);
        empty_box.set_vexpand(true);

        let status = adw::StatusPage::new();
        status.set_icon_name(Some("accessories-text-editor-symbolic"));
        status.set_title("No Cheatsheets Yet");
        status.set_description(Some("Add quick-access markdown notes"));
        empty_box.append(&status);

        let add_btn = Button::with_label("+ Add Cheatsheet");
        add_btn.add_css_class("suggested-action");
        add_btn.set_halign(gtk4::Align::Center);
        add_btn.set_margin_bottom(16);

        let state_c = state.clone();
        let container_c = container.clone();
        let paned_pos_c = paned_pos.clone();
        add_btn.connect_clicked(move |_| {
            state_c
                .borrow_mut()
                .add_cheatsheet("New Note".into(), "# My Note\n\nContent here...".into());
            let _ = state_c.borrow().save();
            rebuild_content(&container_c, &state_c, &paned_pos_c);
        });
        empty_box.append(&add_btn);

        let scroll = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .vexpand(true)
            .child(&empty_box)
            .build();
        container.append(&scroll);
        return;
    }

    drop(state_ref);

    // Paned: list on top, markdown preview on bottom
    let paned = gtk4::Paned::new(Orientation::Vertical);
    paned.set_vexpand(true);
    paned.set_shrink_start_child(false);
    paned.set_shrink_end_child(false);

    // --- Top: list of cheatsheets ---
    let list_box = GtkBox::new(Orientation::Vertical, 0);

    let list = gtk4::ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::Single);
    list.add_css_class("boxed-list");
    list.set_margin_top(12);
    list.set_margin_start(12);
    list.set_margin_end(12);

    // Preview area (shared, updated on selection)
    let tag_table = chat::markdown::create_tag_table();
    let preview_buffer = gtk4::TextBuffer::new(Some(&tag_table));
    let preview_view = TextView::builder()
        .buffer(&preview_buffer)
        .editable(false)
        .cursor_visible(false)
        .wrap_mode(gtk4::WrapMode::Word)
        .top_margin(12)
        .bottom_margin(12)
        .left_margin(12)
        .right_margin(12)
        .vexpand(true)
        .build();

    let state_ref = state.borrow();
    for (idx, sheet) in state_ref.cheatsheets.iter().enumerate() {
        let row = adw::ActionRow::new();
        row.set_title(&sheet.name);

        let preview: String = sheet.content.chars().take(50).collect();
        let subtitle = if sheet.content.len() > 50 {
            format!("{}...", preview)
        } else {
            preview
        };
        row.set_subtitle(&subtitle);
        row.set_activatable(true);

        // Edit button
        let edit_btn = Button::from_icon_name("document-edit-symbolic");
        edit_btn.set_valign(gtk4::Align::Center);
        edit_btn.add_css_class("flat");
        edit_btn.set_tooltip_text(Some("Edit cheatsheet"));

        let state_c = state.clone();
        let container_c = container.clone();
        let paned_pos_c = paned_pos.clone();
        let sheet_id = sheet.id.clone();
        let sheet_name = sheet.name.clone();
        let sheet_content = sheet.content.clone();

        edit_btn.connect_clicked(move |btn| {
            show_edit_dialog(
                btn,
                &state_c,
                &container_c,
                &paned_pos_c,
                &sheet_id,
                &sheet_name,
                &sheet_content,
            );
        });
        row.add_suffix(&edit_btn);

        // Delete button
        let delete_btn = Button::from_icon_name("user-trash-symbolic");
        delete_btn.set_valign(gtk4::Align::Center);
        delete_btn.add_css_class("flat");
        delete_btn.set_tooltip_text(Some("Delete cheatsheet"));

        let state_c = state.clone();
        let container_c = container.clone();
        let paned_pos_c = paned_pos.clone();
        let sheet_id = sheet.id.clone();

        delete_btn.connect_clicked(move |btn| {
            let dialog = adw::MessageDialog::new(
                btn.root()
                    .and_then(|r| r.downcast::<gtk4::Window>().ok())
                    .as_ref(),
                Some("Delete cheatsheet?"),
                Some("This cannot be undone."),
            );
            dialog.add_response("cancel", "Cancel");
            dialog.add_response("delete", "Delete");
            dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
            dialog.set_default_response(Some("cancel"));

            let state_cc = state_c.clone();
            let container_cc = container_c.clone();
            let paned_pos_cc = paned_pos_c.clone();
            let id = sheet_id.clone();

            dialog.connect_response(None, move |d, response| {
                if response == "delete" {
                    state_cc.borrow_mut().remove_cheatsheet(&id);
                    let _ = state_cc.borrow().save();
                    rebuild_content(&container_cc, &state_cc, &paned_pos_cc);
                }
                d.close();
            });

            dialog.present();
        });
        row.add_suffix(&delete_btn);

        // Select first cheatsheet by default for preview
        if idx == 0 {
            chat::markdown::render(&preview_buffer, &sheet.content);
        }

        list.append(&row);
    }
    drop(state_ref);

    // Selection change → update markdown preview
    let state_c = state.clone();
    let preview_buffer_c = preview_buffer.clone();
    list.connect_row_selected(move |_, row| {
        if let Some(row) = row {
            let idx = row.index() as usize;
            let state_ref = state_c.borrow();
            if let Some(sheet) = state_ref.cheatsheets.get(idx) {
                chat::markdown::render(&preview_buffer_c, &sheet.content);
            }
        }
    });

    // Add new cheatsheet button
    let add_btn = Button::with_label("+ Add Cheatsheet");
    add_btn.add_css_class("suggested-action");
    add_btn.set_margin_top(8);
    add_btn.set_margin_start(12);
    add_btn.set_margin_end(12);
    add_btn.set_margin_bottom(8);

    let state_c = state.clone();
    let container_c = container.clone();
    let paned_pos_c = paned_pos.clone();
    add_btn.connect_clicked(move |_| {
        state_c
            .borrow_mut()
            .add_cheatsheet("New Note".into(), "# My Note\n\nContent here...".into());
        let _ = state_c.borrow().save();
        rebuild_content(&container_c, &state_c, &paned_pos_c);
    });

    list_box.append(&list);
    list_box.append(&add_btn);

    let list_scroll = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .child(&list_box)
        .build();

    // --- Bottom: markdown preview ---
    let preview_scroll = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .child(&preview_view)
        .build();

    paned.set_start_child(Some(&list_scroll));
    paned.set_end_child(Some(&preview_scroll));
    paned.set_position(paned_pos.get());

    // Persist paned position across rebuilds (session-local)
    let paned_pos_c = paned_pos.clone();
    paned.connect_position_notify(move |p| {
        paned_pos_c.set(p.position());
    });

    container.append(&paned);

    // Auto-select first row
    if let Some(first) = list.row_at_index(0) {
        list.select_row(Some(&first));
    }
}

/// Show edit dialog for a cheatsheet
#[allow(clippy::too_many_arguments)]
fn show_edit_dialog(
    btn: &Button,
    state: &Rc<RefCell<AppState>>,
    container: &GtkBox,
    paned_pos: &Rc<Cell<i32>>,
    sheet_id: &str,
    name: &str,
    content: &str,
) {
    let dialog = adw::Dialog::new();
    dialog.set_title("Edit Cheatsheet");
    dialog.set_content_width(450);
    dialog.set_content_height(350);

    let toolbar = adw::ToolbarView::new();

    let header = adw::HeaderBar::new();
    let save_btn = Button::with_label("Save");
    save_btn.add_css_class("suggested-action");
    header.pack_end(&save_btn);
    toolbar.add_top_bar(&header);

    let vbox = GtkBox::new(Orientation::Vertical, 12);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);

    // Name entry
    let name_entry = adw::EntryRow::new();
    name_entry.set_title("Name");
    name_entry.set_text(name);

    let name_list = gtk4::ListBox::new();
    name_list.add_css_class("boxed-list");
    name_list.append(&name_entry);
    vbox.append(&name_list);

    // Content editor (raw markdown)
    let text_view = TextView::new();
    text_view.set_wrap_mode(gtk4::WrapMode::Word);
    text_view.add_css_class("card");
    text_view.set_top_margin(8);
    text_view.set_bottom_margin(8);
    text_view.set_left_margin(8);
    text_view.set_right_margin(8);
    text_view.set_monospace(true);
    text_view.buffer().set_text(content);

    let scroll = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand(true)
        .min_content_height(150)
        .child(&text_view)
        .build();
    vbox.append(&scroll);

    toolbar.set_content(Some(&vbox));
    dialog.set_child(Some(&toolbar));

    let state_c = state.clone();
    let container_c = container.clone();
    let paned_pos_c = paned_pos.clone();
    let dialog_c = dialog.clone();
    let id = sheet_id.to_string();

    save_btn.connect_clicked(move |_| {
        let new_name = name_entry.text().to_string();
        let buffer = text_view.buffer();
        let (start, end) = buffer.bounds();
        let new_content = buffer.text(&start, &end, false).to_string();

        {
            let mut state_mut = state_c.borrow_mut();
            if let Some(sheet) = state_mut.cheatsheets.iter_mut().find(|c| c.id == id) {
                sheet.name = new_name;
                sheet.content = new_content;
            }
            let _ = state_mut.save();
        }

        dialog_c.close();
        rebuild_content(&container_c, &state_c, &paned_pos_c);
    });

    if let Some(window) = btn.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) {
        dialog.present(Some(&window));
    }
}
