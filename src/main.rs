use freya::prelude::{
    LaunchConfig,
    WindowConfig,
    launch,
};

fn main() {
    env_logger::init();
    launch(
        LaunchConfig::new().with_window(
            WindowConfig::new(caskade::app)
                .with_title("Praline")
                .with_size(420., 820.),
        ),
    )
}
