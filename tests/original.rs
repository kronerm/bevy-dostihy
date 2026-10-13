//! The original game's installer unpacks into every picture and sound the game
//! uses. Needs the installer: `assets/Dostihy3000setup.exe`, or a download.
//! Run with `cargo test -- --ignored`.

use bevy::asset::AssetPlugin;
use bevy::audio::AudioSource;
use bevy::prelude::*;
use dostihy::data::{BOARD, Field};
use dostihy::original::{Original, OriginalPlugin, original};

#[test]
#[ignore = "needs the installer (or a download of it)"]
fn installer_unpacks_into_the_game_assets() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), OriginalPlugin))
        .init_asset::<Image>()
        .init_asset::<AudioSource>();
    let server = app.world().resource::<AssetServer>().clone();

    // As the game does: the installer first, its pictures and sounds once it's there.
    let start = std::time::Instant::now();
    while !app.world().get_resource::<Original>().is_some_and(|o| o.settled(&server)) {
        assert!(start.elapsed().as_secs() < 120, "the installer didn't load in time");
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(app.world().resource::<Original>().loaded(&server), "the installer couldn't be loaded");
    println!("unpacked in {:?}", start.elapsed());

    let horses = BOARD.iter().filter_map(|f| if let Field::Horse(h) = f { Some(h.photo) } else { None });
    let images: Vec<(String, Handle<Image>)> = ["pawn", "race", "main_race", "icons"]
        .into_iter()
        .map(String::from)
        .chain(horses.map(|h| format!("horse/{h}")))
        .map(|label| (label.clone(), server.load(original(&label))))
        .collect();
    let sounds: Vec<Handle<AudioSource>> =
        ["dice", "coins", "register", "card", "horse", "gong", "click", "laugh"].map(|s| server.load(original(&format!("sound/{s}")))).into();

    // Already there: asking for them loads nothing more.
    let assets = app.world().resource::<Assets<Image>>();
    for (label, handle) in &images {
        let image = assets.get(handle).unwrap_or_else(|| panic!("{label} is in the installer"));
        let alpha = image.data.as_ref().unwrap().chunks(4).map(|px| px[3]);
        if label.starts_with("horse/") {
            assert_eq!(alpha.clone().next(), Some(0), "{label}: the wood around the horse is cut out");
            assert!(alpha.clone().any(|a| a == 255), "{label}: the horse is there");
        }
    }
    let audio = app.world().resource::<Assets<AudioSource>>();
    assert!(sounds.iter().all(|h| audio.contains(h)), "every sound is in the installer");
}
