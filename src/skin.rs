use std::rc::Rc;

use freya::{
    icons::lucide,
    prelude::*,
};
use freya_hypereffect::{
    ButtonProps,
    ButtonVariant,
    CardProps,
    CheckboxProps,
    InputProps,
    ProgressProps,
    SharedSkin,
    Skin,
};

use crate::ui::{
    ACCENT,
    NEGATIVE,
    icon,
    palette,
};

/// The look of a COP page: flat surfaces, no frames, the wallet's accent for the main action.
pub struct PralineSkin;

impl PralineSkin {
    pub fn shared() -> SharedSkin {
        Rc::new(PralineSkin)
    }
}

impl Skin for PralineSkin {
    fn button(&self, props: ButtonProps) -> Element {
        SkinButton {
            label: props.label,
            variant: props.variant,
            enabled: props.enabled,
            on_press: props.on_press,
        }
        .into_element()
    }

    fn card(&self, props: CardProps) -> Element {
        let colors = palette();
        let style = props.style;
        rect()
            .width(Size::fill())
            .padding(style.space_medium + 4.)
            .spacing(style.space_medium)
            .corner_radius(18.)
            .background(colors.surface_secondary)
            .map(props.title, |el, title| {
                el.child(
                    label()
                        .font_size(13.)
                        .color(colors.text_secondary)
                        .text(title),
                )
            })
            .children(props.children)
            .into_element()
    }

    fn input(&self, props: InputProps) -> Element {
        let on_submit = props.on_submit;
        Input::new(WritableSetter::new(props.value, props.on_change))
            .filled()
            .placeholder(props.placeholder)
            .multiline(props.multiline)
            .enabled(props.enabled)
            .on_submit(move |text: String| on_submit.call(text))
            .width(Size::fill())
            .into_element()
    }

    fn checkbox(&self, props: CheckboxProps) -> Element {
        let colors = palette();
        let on_toggle = props.on_toggle;
        rect()
            .horizontal()
            .spacing(10.)
            .cross_align(Alignment::center())
            .maybe(props.enabled, |el| {
                el.cursor(CursorIcon::Pointer)
                    .on_pointer_down(move |_: Event<PointerEventData>| on_toggle.call(()))
            })
            .child(
                rect()
                    .width(Size::px(22.))
                    .height(Size::px(22.))
                    .corner_radius(7.)
                    .center()
                    .background(if props.checked {
                        ACCENT
                    } else {
                        colors.surface_tertiary
                    })
                    .maybe(props.checked, |el| {
                        el.child(icon(lucide::check(), 14., colors.background))
                    }),
            )
            .maybe(!props.label.is_empty(), |el| {
                el.child(label().font_size(15.).text(props.label))
            })
            .into_element()
    }

    fn progress(&self, props: ProgressProps) -> Element {
        let colors = palette();
        let fraction = props.fraction.clamp(0., 1.);
        rect()
            .width(Size::fill())
            .spacing(6.)
            .map(props.label, |el, text| {
                el.child(
                    label()
                        .font_size(13.)
                        .color(colors.text_secondary)
                        .text(text),
                )
            })
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .height(Size::px(6.))
                    .corner_radius(3.)
                    .background(colors.surface_tertiary)
                    .child(
                        rect()
                            .width(Size::percent(fraction * 100.))
                            .height(Size::fill())
                            .corner_radius(3.)
                            .background(ACCENT),
                    ),
            )
            .into_element()
    }
}

#[derive(PartialEq)]
struct SkinButton {
    label: String,
    variant: ButtonVariant,
    enabled: bool,
    on_press: EventHandler<()>,
}

impl Component for SkinButton {
    fn render(&self) -> impl IntoElement {
        let colors = palette();
        let mut hovering = use_state(|| false);
        let (background, hover, text) = match self.variant {
            ButtonVariant::Primary => (ACCENT, ACCENT.with_a(215), colors.background),
            ButtonVariant::Danger => (NEGATIVE, NEGATIVE.with_a(215), colors.background),
            ButtonVariant::Secondary => (
                colors.surface_primary,
                colors.surface_primary.with_a(200),
                colors.text_primary,
            ),
            ButtonVariant::Plain => (Color::TRANSPARENT, colors.surface_secondary, ACCENT),
        };
        let on_press = self.on_press.clone();
        rect()
            .width(Size::fill())
            .padding((13., 16.))
            .corner_radius(14.)
            .center()
            .background(if hovering() && self.enabled {
                hover
            } else {
                background
            })
            .opacity(if self.enabled { 1. } else { 0.45 })
            .cursor(if self.enabled {
                CursorIcon::Pointer
            } else {
                CursorIcon::NotAllowed
            })
            .on_pointer_enter(move |_| hovering.set(true))
            .on_pointer_leave(move |_| hovering.set(false))
            .maybe(self.enabled, |el| el.on_press(move |_| on_press.call(())))
            .child(
                label()
                    .font_size(15.)
                    .font_weight(FontWeight::BOLD)
                    .color(text)
                    .text(self.label.clone()),
            )
    }
}
