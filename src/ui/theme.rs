// Copyright 2025 Asher Buk
//
// SPDX-License-Identifier: Apache-2.0
//
// https://github.com/AshBuk/Shaya

//! Application stylesheet and color palette

use crate::core::state::Settings;
use gtk4::{gdk, CssProvider};
use std::cell::RefCell;

const STYLE: &str = include_str!("style.css");

thread_local! {
    static PROVIDER: RefCell<Option<CssProvider>> = const { RefCell::new(None) };
}

/// Register the stylesheet for the display and apply the saved appearance.
pub fn init(settings: &Settings) {
    let provider = CssProvider::new();
    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    PROVIDER.with(|p| *p.borrow_mut() = Some(provider));
    apply(settings.dark_mode, settings.window_opacity);
}

/// Reload the stylesheet. Opacity applies to backgrounds, text stays fully opaque.
pub fn apply(is_dark: bool, opacity: f64) {
    PROVIDER.with(|p| {
        if let Some(provider) = p.borrow().as_ref() {
            provider.load_from_data(&format!("{}{STYLE}", palette(is_dark, opacity)));
        }
    });
}

fn palette(is_dark: bool, opacity: f64) -> String {
    // Colors are RGB triples, so backgrounds can take the opacity setting.
    let (window, card, popover, bubble, code, line, accent_bg, accent, error) = if is_dark {
        (
            "28, 26, 24",
            "39, 35, 31",
            "46, 40, 34",
            "54, 45, 34",
            "22, 20, 18",
            "255, 255, 255",
            "139, 91, 40",
            "231, 188, 124",
            "255, 123, 114",
        )
    } else {
        (
            "246, 244, 239",
            "255, 255, 255",
            "255, 255, 255",
            "240, 229, 210",
            "237, 235, 230",
            "0, 0, 0",
            "139, 88, 32",
            "122, 75, 24",
            "192, 28, 40",
        )
    };

    let (foreground, muted) = if is_dark {
        ("242, 235, 225", "190, 177, 159")
    } else {
        ("47, 39, 30", "110, 94, 74")
    };
    let mut css = format!(
        "@define-color shaya_fg rgb({foreground});
         @define-color shaya_muted rgb({muted});
         @define-color window_fg_color rgb({foreground});
         @define-color view_fg_color rgb({foreground});
         @define-color card_fg_color rgb({foreground});
         @define-color popover_fg_color rgb({foreground});
         @define-color headerbar_fg_color rgb({foreground});
         @define-color headerbar_bg_color rgb({window});
         @define-color dialog_bg_color rgb({popover});
         @define-color dialog_fg_color rgb({foreground});
         @define-color accent_fg_color white;
         @define-color shaya_window rgba({window}, {opacity:.2});
         @define-color shaya_card rgba({card}, {opacity:.2});
         @define-color shaya_bubble rgb({bubble});
         @define-color shaya_code rgb({code});
         @define-color shaya_border rgba({line}, 0.09);
         @define-color shaya_accent rgb({accent});
         @define-color shaya_error rgb({error});
         @define-color window_bg_color rgb({window});
         @define-color view_bg_color rgba({window}, {opacity:.2});
         @define-color card_bg_color rgba({card}, {opacity:.2});
         @define-color popover_bg_color rgb({popover});
         @define-color accent_bg_color rgb({accent_bg});
         @define-color accent_color rgb({accent});\n"
    );

    // libadwaita 1.6 reads CSS variables, which GTK before 4.16 cannot parse.
    if gtk4::check_version(4, 16, 0).is_none() {
        css.push_str(&format!(
            ":root {{
                --window-bg-color: rgb({window});
                --window-fg-color: rgb({foreground});
                --view-fg-color: rgb({foreground});
                --card-fg-color: rgb({foreground});
                --popover-fg-color: rgb({foreground});
                --headerbar-bg-color: rgb({window});
                --headerbar-fg-color: rgb({foreground});
                --dialog-bg-color: rgb({popover});
                --dialog-fg-color: rgb({foreground});
                --accent-fg-color: white;
                --view-bg-color: rgba({window}, {opacity:.2});
                --card-bg-color: rgba({card}, {opacity:.2});
                --popover-bg-color: rgb({popover});
                --accent-bg-color: rgb({accent_bg});
                --accent-color: rgb({accent});
            }}\n"
        ));
    }
    css
}
