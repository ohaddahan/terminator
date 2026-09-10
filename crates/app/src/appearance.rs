use eframe::egui::{self, Color32, FontFamily, FontId, TextStyle};
use terminator_core::appearance::{AppearanceConfig, rgb};

/// Keep small navigation/action glyphs legible independently of secondary text.
pub const ICON_COLOR: Color32 = Color32::from_rgb(242, 244, 248);

pub fn color(value: &str) -> Color32 {
    let [r, g, b] = rgb(value).unwrap_or([209, 211, 217]);
    Color32::from_rgb(r, g, b)
}
pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for (name, bytes, family) in [
        (
            "Inter",
            include_bytes!("../assets/fonts/Inter-Regular.ttf").as_slice(),
            FontFamily::Proportional,
        ),
        (
            "Inter Semibold",
            include_bytes!("../assets/fonts/Inter-SemiBold.ttf").as_slice(),
            FontFamily::Name("Semibold".into()),
        ),
        (
            "JetBrains Mono",
            include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf").as_slice(),
            FontFamily::Monospace,
        ),
        (
            "JetBrains Mono Bold",
            include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf").as_slice(),
            FontFamily::Name("Terminal Bold".into()),
        ),
    ] {
        fonts
            .font_data
            .insert(name.into(), egui::FontData::from_static(bytes).into());
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, name.into());
    }
    ctx.set_fonts(fonts);
    egui_extras::install_image_loaders(ctx);
    apply(ctx, &AppearanceConfig::default());
}
pub fn apply(ctx: &egui::Context, theme: &AppearanceConfig) {
    let mut style = egui::Style {
        visuals: egui::Visuals::dark(),
        ..Default::default()
    };
    for role in [TextStyle::Body, TextStyle::Button] {
        style.text_styles.insert(role, FontId::proportional(13.0));
    }
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(12.0));
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(16.0, FontFamily::Name("Semibold".into())),
    );
    style.text_styles.insert(
        TextStyle::Name("Section".into()),
        FontId::new(13.0, FontFamily::Name("Semibold".into())),
    );
    style
        .text_styles
        .insert(TextStyle::Monospace, FontId::monospace(13.0));
    style.visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
    style.visuals.panel_fill = color(&theme.surface);
    style.visuals.window_fill = color(&theme.window);
    style.visuals.extreme_bg_color = color(&theme.surface);
    style.visuals.faint_bg_color = color(&theme.hover);
    style.visuals.override_text_color = Some(color(&theme.text));
    style.visuals.error_fg_color = color(&theme.status_failed);
    style.visuals.weak_text_color = Some(color(&theme.secondary));
    style.visuals.selection.bg_fill = color(&theme.selection);
    style.visuals.selection.stroke = egui::Stroke::new(1.0, color(&theme.accent));
    style.visuals.window_stroke = egui::Stroke::new(theme.border_width, color(&theme.border));
    style.visuals.window_corner_radius = egui::CornerRadius::same(3);
    for widget in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.bg_fill = color(&theme.surface);
        widget.weak_bg_fill = color(&theme.surface);
        widget.bg_stroke = egui::Stroke::new(theme.border_width, color(&theme.border));
        widget.fg_stroke = egui::Stroke::new(1.0, color(&theme.text));
        widget.corner_radius = egui::CornerRadius::same(3);
    }
    style.visuals.widgets.hovered.bg_fill = color(&theme.hover);
    style.visuals.widgets.hovered.weak_bg_fill = color(&theme.hover);
    style.visuals.widgets.active.bg_stroke.color = color(&theme.accent);
    style.visuals.window_shadow = egui::epaint::Shadow::NONE;
    style.visuals.popup_shadow = egui::epaint::Shadow::NONE;
    style.visuals.indent_has_left_vline = false;
    style.visuals.menu_corner_radius = egui::CornerRadius::same(4);
    // Shared by every app-owned ScrollArea: a slim, rounded overlay handle.
    style.spacing.scroll = egui::style::ScrollStyle {
        floating: true,
        bar_width: 8.0,
        floating_width: 4.0,
        floating_allocated_width: 4.0,
        handle_min_length: 28.0,
        bar_inner_margin: 3.0,
        bar_outer_margin: 2.0,
        foreground_color: true,
        dormant_background_opacity: 0.0,
        active_background_opacity: 0.0,
        interact_background_opacity: 0.08,
        dormant_handle_opacity: 0.18,
        active_handle_opacity: 0.42,
        interact_handle_opacity: 0.70,
        ..Default::default()
    };
    style.spacing.button_padding = egui::vec2(8.0, 5.0);
    style.spacing.text_edit_width = 220.0;
    style.spacing.indent = 14.0;
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.interact_size.y = 28.0;
    ctx.set_style_of(egui::Theme::Dark, style);
    ctx.set_theme(egui::Theme::Dark);
}

pub fn tool_button(
    ui: &mut egui::Ui,
    tool: crate::preferences::SidebarTool,
    label: &str,
    active: bool,
) -> egui::Response {
    use crate::preferences::SidebarTool;
    let response = ui
        .add_sized([36.0, 32.0], egui::Button::new("").frame(false))
        .on_hover_text(label);
    if response.hovered() {
        ui.painter()
            .rect_filled(response.rect, 2, ui.visuals().widgets.hovered.bg_fill);
    }
    let tint = ICON_COLOR;
    let icon = match tool {
        SidebarTool::Explorer => "Files",
        SidebarTool::Agents => "PanelsTopLeft",
        SidebarTool::Git => "GitBranch",
        SidebarTool::History => "History",
    };
    egui::Image::new(crate::icons::source(icon))
        .tint(tint)
        .paint_at(
            ui,
            egui::Rect::from_center_size(response.rect.center(), egui::vec2(16.0, 16.0)),
        );
    if active {
        ui.painter().line_segment(
            [
                response.rect.left_bottom() + egui::vec2(8.0, -1.0),
                response.rect.right_bottom() + egui::vec2(-8.0, -1.0),
            ],
            egui::Stroke::new(2.0, tint),
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, active, label)
    });
    response
}
/// Flat full-width action row with a fixed icon column and optional shortcut.
pub fn menu_item(ui: &mut egui::Ui, label: &str, icon: &str, shortcut: &str) -> egui::Response {
    ui.set_min_width(220.0);
    ui.spacing_mut().item_spacing.y = 2.0;
    let destructive =
        label.starts_with("Close ") || label.starts_with("Remove ") || label.starts_with("Clear ");
    let previous = ui.visuals().override_text_color;
    let tint = if destructive {
        ui.visuals().error_fg_color
    } else {
        ui.visuals().weak_text_color()
    };
    if destructive {
        ui.visuals_mut().override_text_color = Some(tint);
    }
    let response = row(ui, label, icon, false, 26.0, shortcut, tint);
    ui.visuals_mut().override_text_color = previous;
    #[cfg(feature = "test-support")]
    crate::diagnostics::record(ui.ctx(), label, response.rect);
    response
}
pub fn target_header(ui: &mut egui::Ui, target: &str) {
    ui.set_width(300.0);
    ui.add(egui::Label::new(egui::RichText::new(target).small().weak()).truncate())
        .on_hover_text(target);
    ui.separator();
}
/// Git tint applies to the filename and badge; the icon stays bright.
pub fn file_row(
    ui: &mut egui::Ui,
    label: &str,
    icon: &str,
    selected: bool,
    height: f32,
    trailing: &str,
    tint: Color32,
) -> egui::Response {
    ui.scope(|ui| {
        ui.visuals_mut().override_text_color = Some(tint);
        row(ui, label, icon, selected, height, trailing, tint)
    })
    .inner
}
pub fn project_row(
    ui: &mut egui::Ui,
    label: &str,
    icon: &str,
    selected: bool,
    height: f32,
    trailing: &str,
    tint: Color32,
) -> egui::Response {
    ui.scope(|ui| {
        ui.style_mut()
            .text_styles
            .insert(TextStyle::Body, FontId::proportional(14.0));
        row(ui, label, icon, selected, height, trailing, tint)
    })
    .inner
}
pub fn session_row(
    ui: &mut egui::Ui,
    label: &str,
    icon: &str,
    selected: bool,
    height: f32,
    trailing: &str,
    tint: Color32,
) -> egui::Response {
    ui.scope(|ui| {
        if !selected {
            ui.visuals_mut().override_text_color = Some(ui.visuals().weak_text_color());
        }
        row(ui, label, icon, selected, height, trailing, tint)
    })
    .inner
}
/// Consistent full-width native sidebar row with fixed icon and status columns.
pub fn row(
    ui: &mut egui::Ui,
    label: &str,
    icon: &str,
    selected: bool,
    height: f32,
    trailing: &str,
    tint: Color32,
) -> egui::Response {
    let response = ui.add_sized(
        [ui.available_width(), height],
        egui::Button::new("").frame(false),
    );
    if response.hovered() || selected {
        ui.painter().rect_filled(
            response.rect,
            4,
            if selected {
                ui.visuals().selection.bg_fill
            } else {
                ui.visuals().widgets.hovered.bg_fill
            },
        );
    }
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(response.rect.left() + 12.0, response.rect.center().y),
        egui::vec2(16.0, 16.0),
    );
    egui::Image::new(crate::icons::source(icon))
        .tint(ICON_COLOR)
        .paint_at(ui, icon_rect);
    let trailing_width = if trailing.is_empty() {
        4.0
    } else {
        ui.painter()
            .layout_no_wrap(trailing.into(), FontId::proportional(12.0), tint)
            .size()
            .x
            + 12.0
    };
    let right = response.rect.right() - trailing_width;
    let rect = egui::Rect::from_min_max(
        egui::pos2(response.rect.left() + 26.0, response.rect.top()),
        egui::pos2(
            right.max(response.rect.left() + 26.0),
            response.rect.bottom(),
        ),
    );
    // Paint the label rather than overlaying a selectable Label widget: the row
    // must own clicks on its text as well as its icon and empty space.
    let mut job = egui::text::LayoutJob::simple(
        label.to_owned(),
        TextStyle::Body.resolve(ui.style()),
        ui.visuals().text_color(),
        rect.width(),
    );
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.painter().layout_job(job);
    let position = egui::pos2(rect.left(), rect.center().y - galley.size().y * 0.5);
    ui.painter()
        .with_clip_rect(rect)
        .galley(position, galley, ui.visuals().text_color());
    ui.painter().text(
        egui::pos2(response.rect.right() - 5.0, response.rect.center().y),
        egui::Align2::RIGHT_CENTER,
        trailing,
        FontId::proportional(12.0),
        if trailing.contains(' ') {
            ui.visuals().weak_text_color()
        } else {
            tint
        },
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, label)
    });
    response
}

/// A compact caption inside a pane border, without tab or split controls.
pub fn pane_caption(
    ui: &mut egui::Ui,
    title: &str,
    active: bool,
    closeable: bool,
) -> (egui::Response, Option<egui::Response>) {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 18.0), egui::Sense::click());
    let tint = if active {
        ui.visuals().selection.stroke.color
    } else {
        ui.visuals().weak_text_color()
    };
    let mut job = egui::text::LayoutJob::simple(
        title.into(),
        FontId::proportional(12.0),
        tint,
        (rect.width() - if closeable { 40.0 } else { 16.0 }).max(0.0),
    );
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.painter().layout_job(job);
    let position = egui::pos2(rect.left() + 8.0, rect.center().y - galley.size().y * 0.5);
    ui.painter()
        .with_clip_rect(rect)
        .galley(position, galley, tint);
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            active,
            title,
        )
    });
    let close = closeable.then(|| {
        let close_rect = egui::Rect::from_center_size(
            egui::pos2(rect.right() - 10.0, rect.center().y),
            egui::vec2(18.0, 18.0),
        );
        let response = ui
            .interact(
                close_rect,
                response.id.with("close-pane"),
                egui::Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Close pane");
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "Close pane")
        });
        if response.hovered() {
            ui.painter()
                .rect_filled(close_rect, 2, ui.visuals().widgets.hovered.bg_fill);
        }
        egui::Image::new(crate::icons::source("X"))
            .tint(if response.hovered() {
                ui.visuals().error_fg_color
            } else {
                ICON_COLOR
            })
            .paint_at(
                ui,
                egui::Rect::from_center_size(close_rect.center(), egui::vec2(12.0, 12.0)),
            );
        response
    });
    (
        response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(title),
        close,
    )
}

/// Brief focus emphasis, fading to a thin, translucent steady-state outline.
pub fn focus_stroke(accent: Color32, elapsed: std::time::Duration) -> egui::Stroke {
    let progress = ((elapsed.as_secs_f32() - 0.2) / 1.0).clamp(0.0, 1.0);
    let strength = (1.0 - progress).powi(2);
    let alpha = (70.0 + 185.0 * strength).round() as u8;
    egui::Stroke::new(
        1.0 + strength,
        Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), alpha),
    )
}

/// Cover custom click controls while preserving text, resize and drag cursors.
pub fn click_cursor(ctx: &egui::Context) {
    if ctx.output(|output| output.cursor_icon) != egui::CursorIcon::Default {
        return;
    }
    let hovered =
        ctx.interaction_snapshot(|snapshot| snapshot.hovered.iter().copied().collect::<Vec<_>>());
    if hovered
        .into_iter()
        .filter_map(|id| ctx.read_response(id))
        .any(|response| {
            response.enabled()
                && response.hovered()
                && response.sense.senses_click()
                && !response.sense.senses_drag()
        })
    {
        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

#[cfg(test)]
mod row_tests {
    use super::*;
    fn draw(ctx: &egui::Context, events: Vec<egui::Event>) -> egui::Response {
        let mut response = None;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(260.0, 80.0),
            )),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            response = Some(row(
                ui,
                "Terminal 1",
                "Terminal",
                false,
                24.0,
                "",
                Color32::GRAY,
            ));
        });
        output.textures_delta.clear();
        response.unwrap()
    }
    #[test]
    fn navigation_row_and_close_icons_render_bright_without_hover() {
        for surface in ["tool", "row", "close"] {
            let ctx = egui::Context::default();
            install(&ctx);
            let draw = || {
                let mut output = ctx.run_ui(egui::RawInput::default(), |ui| match surface {
                    "tool" => {
                        tool_button(ui, crate::preferences::SidebarTool::Git, "Git", false);
                    }
                    "row" => {
                        row(ui, "File", "FileCode", false, 24.0, "", Color32::GRAY);
                    }
                    _ => {
                        pane_caption(ui, "Terminal", false, true);
                    }
                });
                output.textures_delta.clear();
                output
            };
            // Allow the image loader to finish before inspecting painted images.
            draw();
            let output = draw();
            let icons = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect) if rect.brush.is_some() => Some(rect),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert!(!icons.is_empty(), "Missing rendered {surface} icon");
            assert!(
                icons.iter().all(|icon| icon.fill == ICON_COLOR),
                "The {surface} icon must remain bright even when its text is muted"
            );
        }
    }

    #[test]
    fn focus_border_fades_and_stays_soft() {
        let accent = Color32::from_rgb(56, 113, 225);
        let strong = focus_stroke(accent, std::time::Duration::ZERO);
        let fading = focus_stroke(accent, std::time::Duration::from_millis(700));
        let steady = focus_stroke(accent, std::time::Duration::from_secs(2));
        assert_eq!(strong.width, 2.0);
        assert_eq!(strong.color.a(), 255);
        assert!(fading.width < strong.width && fading.width > steady.width);
        assert!(fading.color.a() < strong.color.a() && fading.color.a() > steady.color.a());
        assert_eq!(steady.width, 1.0);
        assert_eq!(steady.color.a(), 70);
        assert_eq!(
            steady,
            focus_stroke(accent, std::time::Duration::from_secs(60))
        );
    }
    #[test]
    fn row_click_works_on_icon_label_and_empty_space() {
        for x in [12.0, 55.0, 230.0] {
            let ctx = egui::Context::default();
            install(&ctx);
            let rect = draw(&ctx, vec![]).rect;
            let pos = egui::pos2(rect.left() + x, rect.center().y);
            draw(&ctx, vec![egui::Event::PointerMoved(pos)]);
            draw(
                &ctx,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                }],
            );
            assert!(
                draw(
                    &ctx,
                    vec![egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: Default::default()
                    }]
                )
                .clicked(),
                "Row should receive the click at x={x}"
            );
        }
    }
}
