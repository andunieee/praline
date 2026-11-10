use freya::prelude::*;

use crate::{
    mock::{
        self,
        Channel,
        Utxo,
    },
    ui::*,
};

#[derive(PartialEq)]
pub struct InternalsScreen;

impl Component for InternalsScreen {
    fn render(&self) -> impl IntoElement {
        let settings = use_settings();
        let (unit, network) = {
            let s = settings.read();
            (s.unit, s.network)
        };
        let utxos = use_hook(mock::utxos);
        let channels = use_hook(mock::channels);
        let colors = palette();

        let utxo_total: u64 = utxos.iter().map(|u| u.value_sat).sum();
        let node_id = format!("{}…{}", &mock::NODE_ID[..12], &mock::NODE_ID[54..]);

        ScrollView::new()
            .width(Size::fill())
            .height(Size::fill())
            .child(page_header("Internals", false))
            .child(
                rect()
                    .width(Size::fill())
                    .padding((4., 16., 24., 16.))
                    .spacing(20.)
                    .child(section(
                        "Node",
                        rect()
                            .width(Size::fill())
                            .child(kv_row("Node ID", node_id))
                            .child(divider())
                            .child(kv_row("Network", network.to_string()))
                            .child(divider())
                            .child(kv_row("Block height", mock::BLOCK_HEIGHT.to_string()))
                            .child(divider())
                            .child(kv_row("Peers", "4 connected"))
                            .child(divider())
                            .child(kv_row("Descriptor", "wpkh([a1b2c3d4/84'/0'/0']xpub6C…/0/*)")),
                    ))
                    .child(section(
                        "UTXOs",
                        rect()
                            .width(Size::fill())
                            .child(kv_row(
                                format!("{} unspent outputs", utxos.len()),
                                mock::format_amount(utxo_total as i64, unit),
                            ))
                            .children(utxos.iter().map(|utxo| {
                                rect()
                                    .key(&utxo.outpoint)
                                    .width(Size::fill())
                                    .child(divider())
                                    .child(utxo_row(utxo, unit))
                            })),
                    ))
                    .child(section(
                        "Lightning channels",
                        rect().width(Size::fill()).children(channels.iter().enumerate().map(
                            |(i, channel)| {
                                rect()
                                    .key(channel.alias)
                                    .width(Size::fill())
                                    .maybe(i > 0, |el| el.child(divider()))
                                    .child(channel_row(channel, unit))
                            },
                        )),
                    ))
                    .child(section(
                        "Fee estimates",
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .content(Content::flex())
                            .padding((10., 0.))
                            .children(mock::fee_estimates().into_iter().map(|fee| {
                                rect()
                                    .width(Size::flex(1.))
                                    .center()
                                    .spacing(2.)
                                    .child(
                                        label()
                                            .font_size(20.)
                                            .font_weight(FontWeight::BOLD)
                                            .text(fee.sat_per_vb.to_string()),
                                    )
                                    .child(
                                        label()
                                            .font_size(11.)
                                            .color(colors.text_secondary)
                                            .text(format!("sat/vB · {}", fee.target)),
                                    )
                            })),
                    ))
                    .child(section(
                        "Sync log",
                        rect()
                            .width(Size::fill())
                            .padding((10., 0.))
                            .spacing(4.)
                            .children(mock::sync_log().map(|line| {
                                paragraph()
                                    .width(Size::fill())
                                    .font_size(11.)
                                    .font_family("monospace")
                                    .color(colors.text_secondary)
                                    .span(line)
                            })),
                    )),
            )
    }
}

fn utxo_row(utxo: &Utxo, unit: crate::state::Unit) -> Rect {
    let colors = palette();
    let confirmations = match utxo.confirmations {
        0 => pill("Unconfirmed", ACCENT),
        n if n < 6 => pill(format!("{n} conf"), ACCENT),
        n => pill(format!("{n} conf"), POSITIVE),
    };

    rect()
        .width(Size::fill())
        .padding((10., 0.))
        .spacing(4.)
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::flex())
                .spacing(6.)
                .cross_align(Alignment::center())
                .child(
                    label()
                        .width(Size::flex(1.))
                        .font_weight(FontWeight::BOLD)
                        .text(mock::format_amount(utxo.value_sat as i64, unit)),
                )
                .maybe(utxo.frozen, |el| el.child(pill("Frozen", colors.info)))
                .child(confirmations),
        )
        .child(
            label()
                .font_size(12.)
                .font_family("monospace")
                .color(colors.text_secondary)
                .text(utxo.outpoint.clone()),
        )
        .child(
            label()
                .width(Size::fill())
                .font_size(12.)
                .font_family("monospace")
                .color(colors.text_secondary)
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .text(utxo.address),
        )
}

fn channel_row(channel: &Channel, unit: crate::state::Unit) -> Rect {
    let colors = palette();
    let local = channel.local_sat;
    let remote = channel.capacity_sat - local;
    let ratio = local as f32 / channel.capacity_sat as f32;

    rect()
        .width(Size::fill())
        .padding((12., 0.))
        .spacing(8.)
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::flex())
                .spacing(8.)
                .cross_align(Alignment::center())
                .child(
                    rect()
                        .width(Size::px(8.))
                        .height(Size::px(8.))
                        .corner_radius(4.)
                        .background(if channel.active {
                            POSITIVE
                        } else {
                            colors.text_placeholder
                        }),
                )
                .child(
                    label()
                        .width(Size::flex(1.))
                        .font_weight(FontWeight::BOLD)
                        .text(channel.alias),
                )
                .child(
                    label()
                        .font_size(12.)
                        .color(colors.text_secondary)
                        .text(mock::format_amount(channel.capacity_sat as i64, unit)),
                ),
        )
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .height(Size::px(6.))
                .corner_radius(3.)
                .overflow(Overflow::Clip)
                .background(colors.surface_primary)
                .child(
                    rect()
                        .width(Size::percent(ratio * 100.))
                        .height(Size::fill())
                        .background(ACCENT),
                ),
        )
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::flex())
                .child(
                    label()
                        .width(Size::flex(1.))
                        .font_size(12.)
                        .color(colors.text_secondary)
                        .text(format!(
                            "Can send {}",
                            mock::format_amount(local as i64, unit)
                        )),
                )
                .child(
                    label()
                        .font_size(12.)
                        .color(colors.text_secondary)
                        .text(format!(
                            "Can receive {}",
                            mock::format_amount(remote as i64, unit)
                        )),
                ),
        )
}
