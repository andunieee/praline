#[cfg(target_os = "android")]
mod android;
mod app;
mod insets;
mod mock;
#[cfg(target_os = "android")]
mod scanner;
mod screens;
mod state;
mod ui;

pub use app::app;

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(droid_app: winit::platform::android::activity::AndroidApp) {
    use freya::{
        android::AndroidPlugin,
        prelude::{
            LaunchConfig,
            WindowConfig,
            launch,
        },
    };
    use freya_winit::renderer::NativeEvent;
    use winit::{
        event_loop::EventLoop,
        platform::android::EventLoopBuilderExtAndroid,
    };

    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    let event_loop = EventLoop::<NativeEvent>::with_user_event()
        .with_android_app(droid_app.clone())
        .build()
        .expect("Failed to build event loop");

    crate::android::install_proxy(event_loop.create_proxy());

    launch(
        LaunchConfig::new()
            .with_plugin(AndroidPlugin::new(droid_app))
            .with_window(WindowConfig::new(app))
            .with_event_loop(event_loop),
    )
}
