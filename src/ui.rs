use freya::{
    icons::lucide,
    prelude::*,
    router::RouterContext,
};

use crate::state::Settings;

pub const ACCENT: Color = Color::from_rgb(247, 147, 26);
pub const POSITIVE: Color = Color::from_rgb(76, 200, 120);
pub const NEGATIVE: Color = Color::from_rgb(235, 95, 95);

pub fn palette() -> ColorsSheet {
    get_theme_or_default().read().colors.clone()
}

pub fn use_settings() -> State<Settings> {
    use_consume::<State<Settings>>()
}

pub fn icon(bytes: Bytes, size: f32, color: Color) -> SvgViewer {
    SvgViewer::new(bytes)
        .stroke(color)
        .width(Size::px(size))
        .height(Size::px(size))
}

/// Round icon badge used for list rows and actions.
pub fn icon_badge(bytes: Bytes, size: f32, color: Color, background: Color) -> Rect {
    rect()
        .width(Size::px(size))
        .height(Size::px(size))
        .corner_radius(size / 2.)
        .background(background)
        .center()
        .child(icon(bytes, size * 0.5, color))
}

/// Screen title row, with an optional back button.
pub fn page_header(title: &'static str, back: bool) -> Rect {
    let colors = palette();
    rect()
        .horizontal()
        .width(Size::fill())
        .padding((14., 16., 10., 16.))
        .spacing(8.)
        .cross_align(Alignment::center())
        .maybe(back, |el| {
            el.child(
                rect()
                    .padding(6.)
                    .corner_radius(99.)
                    .background(colors.surface_secondary)
                    .on_press(|_| RouterContext::get().go_back())
                    .child(icon(lucide::chevron_left(), 22., colors.text_primary)),
            )
        })
        .child(
            label()
                .font_size(24.)
                .font_weight(FontWeight::BOLD)
                .text(title),
        )
}

/// A titled group of content on a rounded surface.
pub fn section(title: &'static str, content: impl Into<Element>) -> Rect {
    let colors = palette();
    rect()
        .width(Size::fill())
        .spacing(8.)
        .child(
            label()
                .font_size(13.)
                .font_weight(FontWeight::BOLD)
                .color(colors.text_secondary)
                .text(title.to_uppercase())
                .margin((0., 4.)),
        )
        .child(
            rect()
                .width(Size::fill())
                .background(colors.surface_secondary)
                .corner_radius(16.)
                .padding((4., 14.))
                .child(content),
        )
}

pub fn kv_row(key: impl Into<String>, value: impl Into<String>) -> Rect {
    let colors = palette();
    rect()
        .horizontal()
        .width(Size::fill())
        .padding((10., 0.))
        .spacing(12.)
        .cross_align(Alignment::center())
        .child(
            label()
                .color(colors.text_secondary)
                .text(key.into()),
        )
        .child(
            label()
                .width(Size::flex(1.))
                .text_align(TextAlign::End)
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .text(value.into()),
        )
}

pub fn divider() -> Rect {
    rect()
        .width(Size::fill())
        .height(Size::px(1.))
        .background(palette().border)
}

pub fn pill(text: impl Into<String>, color: Color) -> Rect {
    rect()
        .padding((3., 8.))
        .corner_radius(99.)
        .background(color.with_a(40))
        .child(
            label()
                .font_size(11.)
                .font_weight(FontWeight::BOLD)
                .color(color)
                .text(text.into()),
        )
}
