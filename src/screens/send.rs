use freya::{
    animation::*,
    clipboard::Clipboard,
    icons::lucide,
    prelude::*,
    router::RouterContext,
};

use crate::{
    app::Route,
    mock,
    ui::*,
};

const VIEWFINDER: f32 = 260.;

fn open_invoice(raw: &str) {
    let invoice: String = raw
        .trim()
        .trim_start_matches("lightning:")
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    if !invoice.is_empty() {
        let _ = RouterContext::get().push(Route::Pay { invoice });
    }
}

#[derive(PartialEq)]
pub struct SendScreen;

impl Component for SendScreen {
    fn render(&self) -> impl IntoElement {
        let manual = use_state(String::new);
        let colors = palette();
        let has_manual = !manual.read().trim().is_empty();

        ScrollView::new()
            .width(Size::fill())
            .height(Size::fill())
            .child(page_header("Send", true))
            .child(
                rect()
                    .width(Size::fill())
                    .padding((4., 16., 24., 16.))
                    .spacing(16.)
                    .cross_align(Alignment::center())
                    .child(Scanner)
                    .child(
                        label()
                            .font_size(13.)
                            .color(colors.text_secondary)
                            .text("Camera not wired up yet. Tap the viewfinder to fake a scan."),
                    )
                    .child(
                        Button::new()
                            .expanded()
                            .width(Size::fill())
                            .filled()
                            .on_press(|_| {
                                let text = Clipboard::get()
                                    .ok()
                                    .filter(|text| !text.trim().is_empty())
                                    .unwrap_or_else(mock::mock_clipboard_invoice);
                                open_invoice(&text);
                            })
                            .child(
                                rect()
                                    .horizontal()
                                    .spacing(8.)
                                    .cross_align(Alignment::center())
                                    .child(icon(lucide::clipboard_paste(), 18., Color::WHITE))
                                    .child("Paste invoice"),
                            ),
                    )
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .content(Content::flex())
                            .cross_align(Alignment::center())
                            .spacing(10.)
                            .child(rect().width(Size::flex(1.)).child(divider()))
                            .child(
                                label()
                                    .font_size(12.)
                                    .color(colors.text_secondary)
                                    .text("or enter manually"),
                            )
                            .child(rect().width(Size::flex(1.)).child(divider())),
                    )
                    .child(
                        Input::new(manual)
                            .width(Size::fill())
                            .placeholder("lnbc1… / bc1q…")
                            .on_submit(|text: String| open_invoice(&text)),
                    )
                    .child(
                        Button::new()
                            .expanded()
                            .width(Size::fill())
                            .outline()
                            .enabled(has_manual)
                            .on_press(move |_| open_invoice(&manual.read()))
                            .child("Continue"),
                    ),
            )
    }
}

#[derive(PartialEq)]
struct Scanner;

impl Component for Scanner {
    fn render(&self) -> impl IntoElement {
        let scan_line = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            conf.on_finish(OnFinish::reverse());
            AnimNum::new(0., 1.)
                .time(1600)
                .ease(Ease::InOut)
                .function(Function::Sine)
        });
        let progress = scan_line.get().value();
        let inner = VIEWFINDER - 48.;

        rect()
            .width(Size::px(VIEWFINDER))
            .height(Size::px(VIEWFINDER))
            .corner_radius(28.)
            .overflow(Overflow::Clip)
            .background(
                RadialGradient::new()
                    .stop(((40, 40, 48), 0.))
                    .stop(((8, 8, 10), 100.)),
            )
            .center()
            .on_press(|_| open_invoice(&mock::mock_clipboard_invoice()))
            .child(icon(lucide::qr_code(), 72., Color::from_af32rgb(0.15, 255, 255, 255)))
            .child(corner(true, true))
            .child(corner(true, false))
            .child(corner(false, true))
            .child(corner(false, false))
            .child(
                rect()
                    .position(Position::new_absolute().top(24. + progress * inner).left(24.))
                    .width(Size::px(inner))
                    .height(Size::px(2.))
                    .background(ACCENT)
                    .shadow(Shadow::new().blur(12.).color(ACCENT)),
            )
    }
}

fn corner(top: bool, left: bool) -> Rect {
    const LEN: f32 = 34.;
    const THICK: f32 = 4.;
    const INSET: f32 = 18.;

    let position = Position::new_absolute();
    let position = if top {
        position.top(INSET)
    } else {
        position.bottom(INSET)
    };
    let position = if left {
        position.left(INSET)
    } else {
        position.right(INSET)
    };

    let bar = |x: f32, y: f32, w: f32, h: f32| {
        rect()
            .position(Position::new_absolute().left(x).top(y))
            .width(Size::px(w))
            .height(Size::px(h))
            .corner_radius(2.)
            .background(ACCENT)
    };

    rect()
        .position(position)
        .width(Size::px(LEN))
        .height(Size::px(LEN))
        .child(bar(0., if top { 0. } else { LEN - THICK }, LEN, THICK))
        .child(bar(if left { 0. } else { LEN - THICK }, 0., THICK, LEN))
}
