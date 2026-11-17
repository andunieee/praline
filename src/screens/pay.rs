use freya::{
    animation::*,
    clipboard::Clipboard,
    icons::lucide,
    prelude::*,
    router::RouterContext,
};

use crate::{
    app::Route,
    fmt,
    state::{
        FlowRequest,
        PendingFlow,
        PendingInvoice,
        SelectedMint,
    },
    ui::*,
};

const VIEWFINDER: f32 = 220.;

/// What a scanned or pasted text is for.
enum Scanned {
    /// A cashu token to redeem.
    Token(String),
    /// A Lightning invoice to pay out of a mint.
    Lightning(String),
    /// Anything else: an order (`cop://…` or a web address).
    Order(String),
}

fn classify(raw: &str) -> Scanned {
    let text = raw.trim();
    let lower = text.to_ascii_lowercase();
    let bare = lower.strip_prefix("lightning:").unwrap_or(&lower);
    if lower.starts_with("cashu") {
        Scanned::Token(text.to_owned())
    } else if ["lnbc", "lntb", "lnbcrt", "lntbs"].iter().any(|p| bare.starts_with(p)) {
        let skip = if lower.starts_with("lightning:") { "lightning:".len() } else { 0 };
        Scanned::Lightning(text[skip..].to_owned())
    } else {
        Scanned::Order(text.to_owned())
    }
}

/// Where payments start: scan or paste an order, an invoice or a token. Orders opened before are
/// listed to be reopened.
#[derive(PartialEq)]
pub struct PayScreen;

impl Component for PayScreen {
    fn render(&self) -> impl IntoElement {
        let engine = use_engine();
        let refresh = use_refresh();
        let mut pending_flow = use_consume::<PendingFlow>().0;
        let mut selected = use_consume::<SelectedMint>().0;
        let mut pending_invoice = use_consume::<PendingInvoice>().0;
        let mut input = use_state(String::new);
        let mut busy = use_state(|| false);
        let mut message = use_state(|| None::<Result<String, String>>);

        let open = {
            let engine = engine.clone();
            move |raw: String| {
                if raw.trim().is_empty() || busy() {
                    return;
                }
                message.set(None);
                match classify(&raw) {
                    Scanned::Token(token) => {
                        busy.set(true);
                        let engine = engine.clone();
                        spawn(async move {
                            let outcome = engine.receive_token(&token).await;
                            if outcome.is_ok() {
                                bump(refresh);
                            }
                            message.set(Some(outcome.map(|(_, sats)| {
                                format!("Received {}", fmt::sats(sats))
                            })));
                            busy.set(false);
                        });
                    }
                    Scanned::Lightning(invoice) => {
                        busy.set(true);
                        let engine = engine.clone();
                        spawn(async move {
                            match engine.mints().await {
                                Ok(mints) => {
                                    match mints.into_iter().max_by_key(|m| m.balance_sat) {
                                        Some(mint) => {
                                            selected.set(Some(mint.url));
                                            pending_invoice.set(Some(invoice));
                                            let _ = RouterContext::get().push(Route::Withdraw);
                                        }
                                        None => message.set(Some(Err("Add a mint first.".into()))),
                                    }
                                }
                                Err(e) => message.set(Some(Err(e))),
                            }
                            busy.set(false);
                        });
                    }
                    Scanned::Order(text) => {
                        busy.set(true);
                        let engine = engine.clone();
                        spawn(async move {
                            let resolved = engine
                                .run(async move { freya_cop::resolve(&text).await.map_err(|e| e.to_string()) })
                                .await;
                            match resolved {
                                Ok(uri) => {
                                    pending_flow.set(Some(FlowRequest { uri, session: None }));
                                    input.set(String::new());
                                    let _ = RouterContext::get().push(Route::Order);
                                }
                                Err(e) => message.set(Some(Err(e))),
                            }
                            busy.set(false);
                        });
                    }
                }
            }
        };

        #[cfg(target_os = "android")]
        {
            let mut scan = crate::scanner::use_scan_result();
            let open = open.clone();
            use_side_effect(move || {
                let scanned = scan.read().clone();
                if let Some(contents) = scanned {
                    scan.set(None);
                    open(contents);
                }
            });
        }

        let recent = use_future({
            let engine = engine.clone();
            move || {
                let _ = refresh();
                let engine = engine.clone();
                async move {
                    let mut flows = engine.db().flows().all().map_err(|e| e.to_string())?;
                    flows.sort_by_key(|f| std::cmp::Reverse(f.last_opened));
                    flows.truncate(8);
                    Ok::<_, String>(flows)
                }
            }
        });

        let recent_rows: Vec<Element> = match &*recent.state() {
            FutureState::Fulfilled(Ok(flows)) => flows
                .iter()
                .filter_map(|flow| {
                    let uri = freya_cop::CopUri::parse(&flow.uri).ok()?;
                    let session = flow.session.0.clone();
                    let title = flow.name.clone().unwrap_or_else(|| fmt::short(&uri.endpoint));
                    let colors = palette();
                    Some(
                        rect()
                            .key(session.clone())
                            .horizontal()
                            .width(Size::fill())
                            .content(Content::flex())
                            .padding((10., 0.))
                            .spacing(12.)
                            .cross_align(Alignment::center())
                            .on_press(move |_| {
                                pending_flow.set(Some(FlowRequest {
                                    uri: uri.clone(),
                                    session: Some(session.clone()),
                                }));
                                let _ = RouterContext::get().push(Route::Order);
                            })
                            .child(icon_badge(lucide::store(), 38., ACCENT, ACCENT.with_a(36)))
                            .child(
                                rect()
                                    .width(Size::flex(1.))
                                    .child(label().font_size(15.).max_lines(1).text(title))
                                    .child(
                                        label()
                                            .font_size(12.)
                                            .color(colors.text_secondary)
                                            .text(fmt::when(flow.last_opened)),
                                    ),
                            )
                            .into_element(),
                    )
                })
                .collect(),
            _ => Vec::new(),
        };

        let outcome: Option<Rect> = message.read().clone().map(|m| match m {
            Ok(text) => notice(text, POSITIVE),
            Err(e) => notice(e, NEGATIVE),
        });

        ScrollView::new()
            .width(Size::fill())
            .height(Size::fill())
            .child(page_header("Pay", false))
            .child(
                rect()
                    .width(Size::fill())
                    .padding((4., 16., 24., 16.))
                    .spacing(16.)
                    .cross_align(Alignment::center())
                    .child(Scanner)
                    .child(
                        label()
                            .color(palette().text_secondary)
                            .text_align(TextAlign::Center)
                            .text("Scan an order, or paste a cop:// address, a web address, a Lightning invoice or a cashu token."),
                    )
                    .child(
                        Input::new(input)
                            .width(Size::fill())
                            .placeholder("cop://… · example.com/order · lnbc… · cashu…")
                            .on_submit({
                                let mut open = open.clone();
                                move |text: String| open(text)
                            }),
                    )
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .content(Content::flex())
                            .spacing(8.)
                            .child(
                                Button::new()
                                    .width(Size::flex(1.))
                                    .outline()
                                    .on_press(move |_| {
                                        if let Ok(text) = Clipboard::get() {
                                            input.set(text.trim().to_owned());
                                        }
                                    })
                                    .child("Paste"),
                            )
                            .child(
                                Button::new()
                                    .width(Size::flex(2.))
                                    .filled()
                                    .enabled(!input.read().trim().is_empty() && !busy())
                                    .on_press({
                                        let mut open = open.clone();
                                        move |_| open(input.read().clone())
                                    })
                                    .child(if busy() { "Opening…" } else { "Open" }),
                            ),
                    )
                    .maybe_child(outcome)
                    .maybe(!recent_rows.is_empty(), |el| {
                        el.child(section(
                            "Recent orders",
                            rect().width(Size::fill()).children(recent_rows),
                        ))
                    }),
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
            .on_press(|_| {
                #[cfg(target_os = "android")]
                crate::scanner::start();
            })
            .child(icon(lucide::qr_code(), 64., Color::from_af32rgb(0.15, 255, 255, 255)))
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
