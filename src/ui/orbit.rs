//! Compact launcher and radial section navigation.

use super::header;
use super::layer_shell::{self, Visible};
use crate::core::state::AppState;
use gtk4::prelude::*;
use gtk4::{
    gio, Align, ApplicationWindow, Box as GtkBox, Button, Fixed, Image, Label, Orientation,
};
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Side of the ring area, and of the orb button at its centre
const RING: i32 = 300;
const ORB: i32 = 88;

pub fn install(
    window: &ApplicationWindow,
    panel: &impl IsA<gtk4::Widget>,
    stack: &adw::ViewStack,
    state: Rc<RefCell<AppState>>,
) {
    let panel = panel.as_ref().clone();
    let root = gtk4::Overlay::new();
    window.set_size_request(300, 300);
    let orbit = Fixed::new();
    orbit.set_halign(Align::Center);
    orbit.set_valign(Align::Center);
    orbit.add_css_class("orbit");
    orbit.set_size_request(RING, RING);
    orbit.set_visible(false);
    root.set_child(Some(&panel));
    root.add_overlay(&orbit);
    window.set_child(Some(&root));

    let links = gtk4::DrawingArea::new();
    links.set_content_width(300);
    links.set_content_height(300);
    links.set_can_target(false);
    links.set_visible(false);
    links.set_draw_func(|_, context, _, _| {
        context.set_source_rgba(0.72, 0.49, 0.23, 0.35);
        context.set_line_width(1.0);
        for index in 0..6 {
            let angle = (index as f64 * 60.0 - 90.0).to_radians();
            context.move_to(150.0 + 46.0 * angle.cos(), 150.0 + 46.0 * angle.sin());
            context.line_to(150.0 + 72.0 * angle.cos(), 150.0 + 72.0 * angle.sin());
        }
        let _ = context.stroke();
    });
    orbit.put(&links, 0.0, 0.0);

    let center = Button::new();
    center.add_css_class("orbit-center");
    center.set_size_request(ORB, ORB);
    center.set_tooltip_text(Some("Open sections · drag to move"));
    if let Ok(texture) = gtk4::gdk::Texture::from_bytes(&gtk4::glib::Bytes::from_static(
        include_bytes!("../../icons/shaya.png"),
    )) {
        let logo = Image::from_paintable(Some(&texture));
        logo.set_pixel_size(50);
        center.set_child(Some(&logo));
    } else {
        center.set_label("Shaya");
    }
    orbit.put(&center, 106.0, 106.0);
    header::setup_drag_gesture(&center, window, state);

    let expanded = Rc::new(Cell::new(false));
    let mut nodes = Vec::new();
    for (index, page) in header::sections(stack).into_iter().enumerate() {
        let (Some(name), Some(title)) = (page.name(), page.title()) else {
            continue;
        };
        let content = GtkBox::new(Orientation::Vertical, 4);
        content.set_halign(Align::Center);
        content.set_valign(Align::Center);
        content.append(&Image::from_icon_name(
            &page.icon_name().unwrap_or_default(),
        ));
        content.append(&Label::new(Some(&title)));
        let button = Button::builder()
            .child(&content)
            .tooltip_text(title.as_str())
            .build();
        button.add_css_class("orbit-node");
        button.set_size_request(64, 64);
        button.set_visible(false);
        let angle = (index as f64 * 60.0 - 90.0).to_radians();
        orbit.put(
            &button,
            118.0 + 104.0 * angle.cos(),
            118.0 + 104.0 * angle.sin(),
        );
        let (window, panel, orbit, stack) =
            (window.clone(), panel.clone(), orbit.clone(), stack.clone());
        button.connect_clicked(move |_| {
            orbit.set_visible(false);
            panel.set_opacity(1.0);
            panel.set_sensitive(true);
            stack.set_visible_child_name(&name);
            layer_shell::fit(&window, Visible::Window);
        });
        nodes.push(button);
    }

    // Run after GTK layout, before painting commits the native surface.
    // after-paint is too late: the input region would wait for another frame.
    let input_handler = Rc::new(RefCell::new(None));
    let handler = input_handler.clone();
    let orbit_c = orbit.clone();
    let buttons = std::iter::once(center.clone())
        .chain(nodes.iter().cloned())
        .collect::<Vec<_>>();
    window.connect_map(move |window| {
        let Some(clock) = window.frame_clock() else {
            return;
        };
        let window = window.downgrade();
        let orbit = orbit_c.clone();
        let buttons = buttons.clone();
        let id = clock.connect_local("layout", true, move |_| {
            if let Some(window) = window.upgrade() {
                update_input_region(&window, &orbit, &buttons);
            }
            None
        });
        *handler.borrow_mut() = Some((clock, id));
    });
    window.connect_unmap(move |_| {
        if let Some((clock, id)) = input_handler.borrow_mut().take() {
            clock.disconnect(id);
        }
    });

    let toggle = {
        let (window, expanded, nodes) = (window.clone(), expanded.clone(), nodes.clone());
        move |open: bool| {
            expanded.set(open);
            links.set_visible(open);
            for node in &nodes {
                node.set_visible(open);
            }
            layer_shell::fit(&window, Visible::Centered(if open { RING } else { ORB }));
        }
    };
    let toggle = Rc::new(toggle);
    let toggle_c = toggle.clone();
    let expanded_c = expanded.clone();
    center.connect_clicked(move |_| toggle_c(!expanded_c.get()));

    let actions = gio::SimpleActionGroup::new();
    let collapse = gio::SimpleAction::new("collapse", None);
    collapse.connect_activate(move |_, _| {
        // Retain the panel allocation so switching modes never resizes or moves
        // the native surface, including on compositors with geometry animations.
        panel.set_opacity(0.0);
        panel.set_sensitive(false);
        orbit.set_visible(true);
        toggle(false);
        center.grab_focus();
    });
    actions.add_action(&collapse);
    window.insert_action_group("orbit", Some(&actions));
}

fn update_input_region(window: &ApplicationWindow, orbit: &Fixed, buttons: &[Button]) {
    let Some(surface) = window.surface() else {
        return;
    };
    let region = gtk4::cairo::Region::create();
    if orbit.is_visible() {
        let (surface_x, surface_y) = window.surface_transform();
        for button in buttons.iter().filter(|button| button.is_visible()) {
            let Some((x, y)) = button.translate_coordinates(window, 0.0, 0.0) else {
                continue;
            };
            let radius = button.width().min(button.height()) / 2;
            add_circle(
                &region,
                (x - surface_x).round() as i32 + button.width() / 2,
                (y - surface_y).round() as i32 + button.height() / 2,
                radius,
            );
        }
    } else {
        let _ = region.union_rectangle(&gtk4::cairo::RectangleInt::new(
            0,
            0,
            surface.width(),
            surface.height(),
        ));
    }
    surface.set_input_region(&region);
}

fn add_circle(region: &gtk4::cairo::Region, x: i32, y: i32, radius: i32) {
    for dy in -radius..=radius {
        let dx = ((radius * radius - dy * dy) as f64).sqrt() as i32;
        let _ = region.union_rectangle(&gtk4::cairo::RectangleInt::new(
            x - dx,
            y + dy,
            dx * 2 + 1,
            1,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapsed_orb_leaves_surrounding_desktop_clickable() {
        let region = gtk4::cairo::Region::create();
        add_circle(&region, 150, 150, 46);
        assert!(region.contains_point(150, 150));
        assert!(region.contains_point(195, 150));
        assert!(!region.contains_point(195, 195));
        assert!(!region.contains_point(150, 46));
    }
}
