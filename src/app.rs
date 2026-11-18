use freya::{
    icons::lucide,
    prelude::*,
    router::*,
};

use crate::{
    engine::Engine,
    screens::*,
    state::{
        PendingFlow,
        PendingInvoice,
        Refresh,
        SelectedMint,
        Settings,
    },
    ui::{
        ACCENT,
        icon,
        notice,
        palette,
        NEGATIVE,
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
            secondary: Color::from_rgb(110, 70, 48),
            tertiary: Color::from_rgb(160, 104, 64),
            background: Color::from_rgb(20, 14, 12),
            surface_primary: Color::from_rgb(54, 40, 35),
            surface_secondary: Color::from_rgb(34, 25, 22),
            surface_tertiary: Color::from_rgb(27, 19, 17),
            border: Color::from_rgb(56, 42, 37),
            text_secondary: Color::from_rgb(168, 150, 140),
            text_highlight: ACCENT,
            ..DARK_COLORS
        }
    } else {
        ColorsSheet {
            primary: ACCENT,
            secondary: Color::from_rgb(110, 70, 48),
            tertiary: Color::from_rgb(160, 104, 64),
            background: Color::from_rgb(248, 242, 236),
            surface_primary: Color::from_rgb(232, 220, 210),
            surface_secondary: Color::WHITE,
            surface_tertiary: Color::from_rgb(240, 230, 221),
            text_secondary: Color::from_rgb(122, 100, 88),
            text_highlight: ACCENT,
            ..LIGHT_COLORS
        }
    };
    theme
}

/// Where the wallet keeps its files.
fn data_dir() -> std::path::PathBuf {
    #[cfg(target_os = "android")]
    {
        use freya::prelude::GlobalContexts;
        use winit::platform::android::activity::AndroidApp;
        if let Some(dir) = GlobalContexts::get()
            .try_get_context::<AndroidApp>()
            .and_then(|app| app.internal_data_path())
        {
            return dir;
        }
    }
    dirs_next()
}

#[cfg(not(target_os = "android"))]
fn dirs_next() -> std::path::PathBuf {
    std::env::var_os("PRALINE_HOME")
        .map(Into::into)
        .or_else(|| {
            std::env::var_os("XDG_DATA_HOME")
                .map(|d| std::path::PathBuf::from(d).join("praline"))
        })
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|d| std::path::PathBuf::from(d).join(".local/share/praline"))
        })
        .unwrap_or_else(|| std::path::PathBuf::from("praline-data"))
}

#[cfg(target_os = "android")]
fn dirs_next() -> std::path::PathBuf {
    std::path::PathBuf::from("praline-data")
}

pub fn app() -> impl IntoElement {
    use_init_theme(|| wallet_theme(true));
    let settings = use_state(Settings::default);
    use_provide_context(move || settings);
    let selected = use_state(|| None::<String>);
    use_provide_context(move || SelectedMint(selected));
    let flow = use_state(|| None);
    use_provide_context(move || PendingFlow(flow));
    let invoice = use_state(|| None::<String>);
    use_provide_context(move || PendingInvoice(invoice));
    let refresh = use_state(|| 0u64);
    use_provide_context(move || Refresh(refresh));

    // Opening the wallet takes the database and the node; it happens once, with the first frame.
    let engine = use_hook(|| Engine::start(&data_dir()));
    match engine {
        Ok(engine) => {
            use_provide_context(move || engine.clone());
            Router::<Route>::new(RouterConfig::default).into_element()
        }
        Err(error) => rect()
            .expanded()
            .theme_background()
            .padding(24.)
            .center()
            .child(notice(format!("The wallet could not start: {error}"), NEGATIVE))
            .into_element(),
    }
}

#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Shell)]
        #[route("/", HomeScreen)]
        Home,
        #[route("/mint", MintScreen)]
        Mint,
        #[route("/mint/deposit", DepositScreen)]
        Deposit,
        #[route("/mint/withdraw", WithdrawScreen)]
        Withdraw,
        #[route("/mint/onchain-deposit", OnchainDepositScreen)]
        OnchainDeposit,
        #[route("/mint/onchain-withdraw", OnchainWithdrawScreen)]
        OnchainWithdraw,
        #[route("/mint/send", SendTokenScreen)]
        SendToken,
        #[route("/mint/receive", ReceiveTokenScreen)]
        ReceiveToken,
        #[route("/transactions", TransactionsScreen)]
        Transactions,
        #[route("/pay", PayScreen)]
        Pay,
        #[route("/order", OrderScreen)]
        Order,
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
                    .child(NavTab::new(Route::Transactions, "Activity", lucide::list))
                    .child(NavTab::new(Route::Pay, "Pay", lucide::qr_code))
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
            (
                Route::Home,
                Route::Mint
                | Route::Deposit
                | Route::Withdraw
                | Route::OnchainDeposit
                | Route::OnchainWithdraw
                | Route::SendToken
                | Route::ReceiveToken,
            ) => true,
            (Route::Pay, Route::Order) => true,
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
