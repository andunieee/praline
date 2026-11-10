use freya::{
    icons::lucide,
    prelude::*,
};

use crate::{
    app::wallet_theme,
    state::{
        Currency,
        Network,
        Unit,
    },
    ui::*,
};

const SEED_WORDS: [&str; 12] = [
    "orbit", "velvet", "canyon", "lunar", "pepper", "hollow", "ticket", "amber", "dragon",
    "silent", "meadow", "fossil",
];

#[derive(Clone, Copy, PartialEq)]
enum Dialog {
    Seed,
    Wipe,
}

#[derive(PartialEq)]
pub struct SettingsScreen;

impl Component for SettingsScreen {
    fn render(&self) -> impl IntoElement {
        let mut settings = use_settings();
        let mut theme = use_theme();
        let mut dialog = use_state(|| None::<Dialog>);
        let s = settings.read().clone();

        let server = use_state(|| s.server.clone());
        let lsp = use_state(|| s.lsp.clone());

        rect()
            .expanded()
            .child(
                ScrollView::new()
                    .width(Size::fill())
                    .height(Size::fill())
                    .child(page_header("Settings", false))
                    .child(
                        rect()
                            .width(Size::fill())
                            .padding((4., 16., 24., 16.))
                            .spacing(20.)
                            .child(section(
                                "Display",
                                rect()
                                    .width(Size::fill())
                                    .child(setting_row(
                                        "Fiat currency",
                                        "Used for conversions",
                                        Select::new()
                                            .selected_item(s.currency.to_string())
                                            .children(Currency::ALL.map(|currency| {
                                                MenuItem::new()
                                                    .selected(s.currency == currency)
                                                    .on_press(move |_| {
                                                        settings.write().currency = currency
                                                    })
                                                    .child(currency.to_string())
                                            })),
                                    ))
                                    .child(divider())
                                    .child(setting_row(
                                        "Bitcoin unit",
                                        "How amounts are shown",
                                        SegmentedButton::new()
                                            .child(
                                                ButtonSegment::new()
                                                    .selected(s.unit == Unit::Sats)
                                                    .on_press(move |_| {
                                                        settings.write().unit = Unit::Sats
                                                    })
                                                    .child("sat"),
                                            )
                                            .child(
                                                ButtonSegment::new()
                                                    .selected(s.unit == Unit::Btc)
                                                    .on_press(move |_| {
                                                        settings.write().unit = Unit::Btc
                                                    })
                                                    .child("BTC"),
                                            ),
                                    ))
                                    .child(divider())
                                    .child(setting_row(
                                        "Dark mode",
                                        "",
                                        Switch::new(WritableSetter::new(s.dark_mode, move |dark| {
                                            settings.write().dark_mode = dark;
                                            theme.set(wallet_theme(dark));
                                        })),
                                    ))
                                    .child(divider())
                                    .child(setting_row(
                                        "Hide balance",
                                        "Tap the balance card to toggle",
                                        Switch::new(WritableSetter::new(
                                            s.hide_balance,
                                            move |hide| settings.write().hide_balance = hide,
                                        )),
                                    )),
                            ))
                            .child(section(
                                "Connection",
                                rect()
                                    .width(Size::fill())
                                    .child(setting_row(
                                        "Network",
                                        "",
                                        Select::new()
                                            .selected_item(s.network.to_string())
                                            .children(Network::ALL.map(|network| {
                                                MenuItem::new()
                                                    .selected(s.network == network)
                                                    .on_press(move |_| {
                                                        settings.write().network = network
                                                    })
                                                    .child(network.to_string())
                                            })),
                                    ))
                                    .child(divider())
                                    .child(text_setting(
                                        "Electrum server",
                                        server,
                                        move |value| settings.write().server = value,
                                    ))
                                    .child(divider())
                                    .child(text_setting(
                                        "Lightning service provider",
                                        lsp,
                                        move |value| settings.write().lsp = value,
                                    ))
                                    .child(divider())
                                    .child(setting_row(
                                        "Route through Tor",
                                        "Slower, more private",
                                        Switch::new(WritableSetter::new(s.use_tor, move |tor| {
                                            settings.write().use_tor = tor
                                        })),
                                    )),
                            ))
                            .child(section(
                                "Security",
                                rect()
                                    .width(Size::fill())
                                    .child(action_row(
                                        lucide::key_round(),
                                        "Show recovery phrase",
                                        palette().text_primary,
                                        move || dialog.set(Some(Dialog::Seed)),
                                    ))
                                    .child(divider())
                                    .child(action_row(
                                        lucide::file_key(),
                                        "Export wallet descriptor",
                                        palette().text_primary,
                                        || {},
                                    ))
                                    .child(divider())
                                    .child(action_row(
                                        lucide::trash_2(),
                                        "Wipe wallet",
                                        NEGATIVE,
                                        move || dialog.set(Some(Dialog::Wipe)),
                                    )),
                            ))
                            .child(section(
                                "About",
                                rect()
                                    .width(Size::fill())
                                    .child(kv_row("Version", env!("CARGO_PKG_VERSION")))
                                    .child(divider())
                                    .child(kv_row("UI toolkit", "Freya")),
                            )),
                    ),
            )
            .child(
                Popup::new()
                    .on_close_request(move |_| dialog.set(None))
                    .map(dialog(), |el, current| match current {
                        Dialog::Seed => el
                            .child(PopupTitle::new("Recovery phrase".to_string()))
                            .child(PopupContent::new().child(seed_grid()))
                            .child(
                                PopupButtons::new().child(
                                    Button::new()
                                        .expanded()
                                        .filled()
                                        .on_press(move |_| dialog.set(None))
                                        .child("I wrote it down"),
                                ),
                            ),
                        Dialog::Wipe => el
                            .child(PopupTitle::new("Wipe wallet?".to_string()))
                            .child(PopupContent::new().child(
                                "This deletes all keys from this device. Make sure you have your recovery phrase.",
                            ))
                            .child(
                                PopupButtons::new()
                                    .child(
                                        Button::new()
                                            .expanded()
                                            .on_press(move |_| dialog.set(None))
                                            .child("Cancel"),
                                    )
                                    .child(
                                        Button::new()
                                            .expanded()
                                            .filled()
                                            .on_press(move |_| dialog.set(None))
                                            .child("Wipe"),
                                    ),
                            ),
                    }),
            )
    }
}

fn setting_row(title: &'static str, subtitle: &'static str, trailing: impl Into<Element>) -> Rect {
    let colors = palette();
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::flex())
        .cross_align(Alignment::center())
        .spacing(12.)
        .padding((12., 0.))
        .child(
            rect()
                .width(Size::flex(1.))
                .spacing(2.)
                .child(label().text(title))
                .maybe(!subtitle.is_empty(), |el| {
                    el.child(
                        label()
                            .font_size(12.)
                            .color(colors.text_secondary)
                            .text(subtitle),
                    )
                }),
        )
        .child(trailing)
}

fn text_setting(
    title: &'static str,
    value: State<String>,
    on_submit: impl FnMut(String) + 'static,
) -> Rect {
    let mut on_submit = on_submit;
    rect()
        .width(Size::fill())
        .spacing(8.)
        .padding((12., 0.))
        .child(label().text(title))
        .child(
            Input::new(value)
                .width(Size::fill())
                .on_submit(move |text: String| on_submit(text)),
        )
}

fn action_row(bytes: Bytes, title: &'static str, color: Color, mut on_press: impl FnMut() + 'static) -> Rect {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::center())
        .spacing(12.)
        .padding((14., 0.))
        .on_press(move |_| on_press())
        .child(icon(bytes, 20., color))
        .child(label().color(color).text(title))
}

fn seed_grid() -> Rect {
    let colors = palette();
    rect()
        .width(Size::fill())
        .spacing(8.)
        .children(SEED_WORDS.chunks(3).enumerate().map(|(row, words)| {
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::flex())
                .spacing(8.)
                .children(words.iter().enumerate().map(|(col, word)| {
                    rect()
                        .width(Size::flex(1.))
                        .horizontal()
                        .spacing(6.)
                        .padding((8., 10.))
                        .corner_radius(10.)
                        .background(colors.surface_primary)
                        .child(
                            label()
                                .font_size(12.)
                                .color(colors.text_secondary)
                                .text(format!("{}", row * 3 + col + 1)),
                        )
                        .child(label().font_weight(FontWeight::BOLD).text(*word))
                }))
        }))
}
