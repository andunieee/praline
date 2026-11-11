use freya::{
    icons::lucide,
    prelude::*,
    router::*,
};

use crate::{
    screens::*,
    state::Settings,
    ui::{
        ACCENT,
        icon,
        palette,
    },
};

/// Extra breathing room under the nav tabs. On Android the system bar insets are
/// applied as padding on the root element, so no extra space is needed.
const BOTTOM_INSET: f32 = if cfg!(target_os = "android") { 0. } else { 6. };

pub fn wallet_theme(dark: bool) -> Theme {
    let mut theme = if dark { dark_theme() } else { light_theme() };
    theme.name = if dark { "dark" } else { "light" };
    theme.colors = if dark {
        ColorsSheet {
            primary: ACCENT,
            secondary: Color::from_rgb(120, 72, 14),
            tertiary: Color::from_rgb(200, 110, 10),
            background: Color::from_rgb(13, 13, 16),
            surface_primary: Color::from_rgb(44, 44, 52),
            surface_secondary: Color::from_rgb(30, 30, 36),
            surface_tertiary: Color::from_rgb(22, 22, 27),
            border: Color::from_rgb(42, 42, 50),
            text_secondary: Color::from_rgb(150, 150, 162),
            text_highlight: ACCENT,
            ..DARK_COLORS
        }
    } else {
        ColorsSheet {
            primary: ACCENT,
            secondary: Color::from_rgb(120, 72, 14),
            tertiary: Color::from_rgb(200, 110, 10),
            background: Color::from_rgb(246, 246, 248),
            surface_primary: Color::from_rgb(225, 225, 232),
            surface_secondary: Color::WHITE,
            surface_tertiary: Color::from_rgb(238, 238, 242),
            text_secondary: Color::from_rgb(105, 105, 118),
            text_highlight: ACCENT,
            ..LIGHT_COLORS
        }
    };
    theme
}

pub fn app() -> impl IntoElement {
    use_init_theme(|| wallet_theme(true));
    let settings = use_state(Settings::default);
    use_provide_context(move || settings);

    Router::<Route>::new(RouterConfig::default)
}

#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Shell)]
        #[route("/", HomeScreen)]
        Home,
        #[route("/send", SendScreen)]
        Send,
        #[route("/pay/:invoice", PayScreen)]
        Pay { invoice: String },
        #[route("/receive", ReceiveScreen)]
        Receive,
        #[route("/internals", InternalsScreen)]
        Internals,
        #[route("/settings", SettingsScreen)]
        Settings,
}

#[derive(PartialEq)]
struct Shell;

impl Component for Shell {
    fn render(&self) -> impl IntoElement {
        let colors = palette();
        let insets = crate::insets::use_insets();

        rect()
            .native_router()
            .expanded()
            .content(Content::flex())
            .theme_background()
            .theme_color()
            .padding((insets.top, insets.right, insets.bottom, insets.left))
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .child(Outlet::<Route>::new()),
            )
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::flex())
                    .padding((8., 8., BOTTOM_INSET, 8.))
                    .background(colors.surface_tertiary)
                    .border(
                        Border::new()
                            .fill(colors.border)
                            .width(BorderWidth {
                                top: 1.,
                                ..Default::default()
                            })
                            .alignment(BorderAlignment::Inner),
                    )
                    .child(NavTab::new(Route::Home, "Wallet", lucide::wallet))
                    .child(NavTab::new(Route::Receive, "Receive", lucide::qr_code))
                    .child(NavTab::new(Route::Internals, "Internals", lucide::cpu))
                    .child(NavTab::new(Route::Settings, "Settings", lucide::settings)),
            )
    }
}

#[derive(PartialEq)]
struct NavTab {
    route: Route,
    label: &'static str,
    icon: Bytes,
}

impl NavTab {
    fn new(route: Route, label: &'static str, icon: fn() -> Bytes) -> Self {
        Self {
            route,
            label,
            icon: icon(),
        }
    }
}

impl Component for NavTab {
    fn render(&self) -> impl IntoElement {
        let current = use_route::<Route>();
        let colors = palette();

        let active = match (&self.route, &current) {
            (Route::Home, Route::Send | Route::Pay { .. }) => true,
            (a, b) => a == b,
        };
        let color = if active {
            ACCENT
        } else {
            colors.text_secondary
        };
        let route = self.route.clone();

        rect()
            .width(Size::flex(1.))
            .center()
            .spacing(4.)
            .padding((6., 0.))
            .corner_radius(12.)
            .on_press(move |_| {
                if RouterContext::get().current::<Route>() != route {
                    let _ = RouterContext::get().replace(route.clone());
                }
            })
            .child(icon(self.icon.clone(), 22., color))
            .child(label().font_size(12.).color(color).text(self.label))
    }
}
