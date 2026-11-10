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
                .with_title("Caskade")
                .with_size(420., 820.),
        ),
    )
}
