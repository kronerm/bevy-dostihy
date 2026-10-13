//! Until the original game's art (and in a browser the font) is there, only a
//! little animation shows: no screen built from missing pictures and text.

use super::board::despawn;
use super::*;

pub(super) fn plugin(app: &mut App) {
    // The game starts loading, and the first `OnEnter` comes before `Art` exists.
    app.add_systems(Startup, spawn.after(super::load_art))
        .add_systems(OnExit(AppState::Loading), despawn::<LoadingRoot>)
        .add_systems(Update, (pulse, done).run_if(in_state(AppState::Loading)));
}

#[derive(Component)]
struct LoadingRoot;

/// One of the dots, pulsing after the one before it.
#[derive(Component)]
struct Dot(usize);

fn spawn(mut commands: Commands, art: Res<Art>) {
    commands
        .spawn((
            LoadingRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(20.0),
                ..default()
            },
            BackgroundColor(FELT_DARK),
        ))
        .with_children(|root| {
            root.spawn(Node { column_gap: Val::Px(14.0), ..default() }).with_children(|row| {
                for k in 0..3 {
                    row.spawn((
                        Dot(k),
                        Node { width: Val::Px(16.0), height: Val::Px(16.0), border_radius: BorderRadius::MAX, ..default() },
                        BackgroundColor(GOLD),
                    ));
                }
            });
            // In a browser it shows once the font is there – long before the installer.
            root.spawn(art.text("Stahuji grafiku z původní hry…", 16.0, MUTED));
        });
}

fn pulse(time: Res<Time>, mut dots: Query<(&Dot, &mut BackgroundColor)>) {
    for (Dot(k), mut color) in &mut dots {
        let wave = ((time.elapsed_secs() * 4.0 - *k as f32 * 0.8).sin() + 1.0) / 2.0;
        color.0 = GOLD.with_alpha(0.25 + 0.75 * wave);
    }
}

/// On to the new-game screen once everything has loaded – or failed to, in
/// which case it says so and the game is played without.
fn done(
    mut art: ResMut<Art>,
    original: Res<Original>,
    assets: Res<AssetServer>,
    images: Res<Assets<Image>>,
    audio: Res<Assets<AudioSource>>,
    mut next: ResMut<NextState<AppState>>,
) {
    let font_settled = |font: &Handle<Font>| assets.is_loaded(font) || assets.load_state(font).is_failed();
    if original.settled(&assets) && art.fonts.iter().all(font_settled) {
        art.take_originals(&original, &assets, &images, &audio);
        next.set(AppState::Setup);
    }
}
