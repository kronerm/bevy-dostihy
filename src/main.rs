use bevy::prelude::*;
use dostihy::{ai::AiPlugin, rules::RulesPlugin, ui::UiPlugin, window_plugins};

fn main() {
    App::new().add_plugins((window_plugins(), RulesPlugin, AiPlugin::default(), UiPlugin)).run();
}
