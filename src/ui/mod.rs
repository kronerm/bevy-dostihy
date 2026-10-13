//! Presentation. The board is drawn in world space as the original "cake",
//! the side panel is Bevy UI. Rules events are animated one after another
//! (see `anim`); while that happens `Busy` is raised and the panel waits.

mod anim;
mod board;
mod geometry;
mod loading;
mod panel;
mod setup;
mod widgets;

use crate::Busy;
use crate::data::*;
use crate::original::{Original, original};
use crate::rules::*;
use bevy::audio::Volume;
use bevy::camera::ScalingMode;
use bevy::prelude::*;
use bevy::text::FontSource;
use std::collections::HashMap;

pub use anim::DismissCard;
pub use panel::CutOff;
pub use setup::SetupState;

/// Logical size the layout is designed for; it is scaled to fit the window.
const DESIGN: Vec2 = Vec2::new(1400.0, 880.0);

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .insert_resource(ClearColor(WOOD_DARK))
            .init_resource::<UiState>()
            .init_resource::<Busy>()
            .add_systems(Startup, (spawn_camera, load_art))
            .add_observer(open_trade)
            .add_systems(Update, (fit_to_window, handle_clicks, widgets::hover_buttons))
            .add_plugins((loading::plugin, setup::plugin, board::plugin, anim::plugin, panel::plugin));
    }
}

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AppState {
    /// Until the art has loaded (`loading`).
    #[default]
    Loading,
    Setup,
    Playing,
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin { min_width: DESIGN.x, min_height: DESIGN.y },
            ..OrthographicProjection::default_2d()
        }),
    ));
}

/// Scales the UI like the camera scales the world, so both stay aligned.
fn fit_to_window(window: Single<&Window>, mut scale: ResMut<UiScale>) {
    let s = (window.width() / DESIGN.x).min(window.height() / DESIGN.y);
    if s > 0.0 && (scale.0 - s).abs() > 0.001 {
        scale.0 = s;
    }
}

// --------------------------------------------------------------------- art

/// Graphics and sounds; most come from the original game (see `crate::original`),
/// taken out by `take_originals` once it has loaded.
#[derive(Resource)]
pub struct Art {
    pawn: Handle<Image>,
    race: Handle<Image>,
    main_race: Handle<Image>,
    icons: Handle<Image>,
    icons_layout: Handle<TextureAtlasLayout>,
    horses: HashMap<&'static str, Handle<Image>>,
    sounds: HashMap<Sound, Handle<AudioSource>>,
    /// The system's sans-serif font, or in a browser the one the page brings.
    font: FontSource,
    /// The browser's font, loaded with the art.
    fonts: Vec<Handle<Font>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Sound {
    Dice,
    Coins,
    Register,
    Card,
    Horse,
    Gong,
    Click,
    Laugh,
}

const SOUNDS: [(Sound, &str); 8] = [
    (Sound::Dice, "dice"),
    (Sound::Coins, "coins"),
    (Sound::Register, "register"),
    (Sound::Card, "card"),
    (Sound::Horse, "horse"),
    (Sound::Gong, "gong"),
    (Sound::Click, "click"),
    (Sound::Laugh, "laugh"),
];

fn load_art(mut commands: Commands, assets: Res<AssetServer>, mut layouts: ResMut<Assets<TextureAtlasLayout>>) {
    // A browser has no system fonts to find: the web build brings Noto Sans (`build-wasm.sh`).
    let (font, fonts) = if cfg!(target_arch = "wasm32") {
        (FontSource::Family("Noto Sans".into()), vec![assets.load("fonts/NotoSans-Regular.ttf"), assets.load("fonts/NotoSans-Bold.ttf")])
    } else {
        (FontSource::SansSerif, Vec::new())
    };
    commands.insert_resource(Art {
        pawn: default(),
        race: default(),
        main_race: default(),
        icons: default(),
        icons_layout: layouts.add(TextureAtlasLayout::from_grid(UVec2::splat(32), 8, 8, None, None)),
        horses: HashMap::new(),
        sounds: HashMap::new(),
        font,
        fonts,
    });
}

impl Art {
    /// The original game's pictures and sounds, once the installer has loaded –
    /// asked for earlier, each would load it all again. Without the installer
    /// they stay empty and nothing shows in their place.
    fn take_originals(&mut self, installer: &Original, assets: &AssetServer, images: &Assets<Image>, audio: &Assets<AudioSource>) {
        let loaded = installer.loaded(assets);
        let get = |label: &str| if loaded { assets.load(original(label)) } else { images.reserve_handle() };
        self.pawn = get("pawn");
        self.race = get("race");
        self.main_race = get("main_race");
        self.icons = get("icons");
        self.horses = BOARD.iter().filter_map(|f| if let Field::Horse(h) = f { Some((h.photo, get(&format!("horse/{}", h.photo)))) } else { None }).collect();
        self.sounds = SOUNDS
            .map(|(sound, file)| (sound, if loaded { assets.load(original(&format!("sound/{file}"))) } else { audio.reserve_handle() }))
            .into();
    }

    /// Icon from the original sprite sheet for squares that have one.
    fn icon(&self, field: &Field) -> Option<TextureAtlas> {
        let index = match field {
            Field::Horse(h) => h.stable as usize,
            Field::Trainer(n) => 7 + *n as usize,
            Field::Service(Service::Transport) => 12,
            Field::Service(Service::Stables) => 13,
            Field::Nahoda => 14,
            Field::Finance => 15,
            _ => return None,
        };
        Some(TextureAtlas { layout: self.icons_layout.clone(), index })
    }

    fn photo(&self, horse: &Horse) -> Handle<Image> {
        self.horses[horse.photo].clone()
    }

    fn play(&self, commands: &mut Commands, sound: Sound) {
        commands.spawn((AudioPlayer::new(self.sounds[&sound].clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(0.6))));
    }
}

// ------------------------------------------------------------------ palette

const FELT: Color = Color::srgb(0.09, 0.42, 0.18);
const FELT_DARK: Color = Color::srgb(0.05, 0.27, 0.11);
/// The track of squares nobody owns; owned ones take the owner's colour.
const TRACK_GREY: Color = Color::srgb(0.52, 0.5, 0.47);
const CREAM: Color = Color::srgb(0.97, 0.95, 0.9);
const INK: Color = Color::srgb(0.13, 0.1, 0.08);
const WOOD: Color = Color::srgb(0.55, 0.3, 0.1);
const WOOD_DARK: Color = Color::srgb(0.33, 0.17, 0.06);
const WOOD_LIGHT: Color = Color::srgb(0.85, 0.55, 0.22);
const TEXT: Color = Color::srgb(0.98, 0.94, 0.86);
const MUTED: Color = Color::srgb(0.85, 0.75, 0.6);
/// Faint text on paper.
const MUTED_INK: Color = Color::srgb(0.5, 0.45, 0.4);
const GOLD: Color = Color::srgb(1.0, 0.8, 0.2);
const DANGER: Color = Color::srgb(0.9, 0.25, 0.2);

// --------------------------------------------------------------- ui state

/// UI-only state: nothing here affects the rules until a command is sent.
#[derive(Resource, Default)]
pub struct UiState {
    /// Square whose card is shown.
    selected: Option<Entity>,
    /// The trade being put together, while the trading section is open.
    draft: Option<Draft>,
    /// "End the game now?" is being asked.
    confirm_finish: bool,
}

/// A trade being put together; it becomes a `Trade` once a partner is picked.
#[derive(Default)]
struct Draft {
    partner: Option<Entity>,
    give: Vec<Entity>,
    take: Vec<Entity>,
    money: i64,
}

/// Opens the trading dialog with this deal filled in.
#[derive(Event)]
pub struct OpenTrade(pub Trade);

fn open_trade(ev: On<OpenTrade>, mut ui: ResMut<UiState>) {
    let Trade { partner, give, take, money } = ev.0.clone();
    ui.draft = Some(Draft { partner: Some(partner), give, take, money });
}

impl Draft {
    fn trade(&self) -> Option<Trade> {
        Some(Trade { partner: self.partner?, give: self.give.clone(), take: self.take.clone(), money: self.money })
    }
}

/// What a button does when pressed.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Click {
    Roll,
    UseCancelCard,
    Buy,
    BuyRaces(usize),
    EndTurn,
    SellToBank(Entity),
    SellRace(Entity),
    Liquidate,
    ProposeTrade,
    AcceptTrade(Entity),
    CancelTrade(Entity),
    FinishGame,
    ConfirmFinish(bool),
    NewGameScreen,
    Select(Entity),
    CloseCard,
    /// Opens or closes the trading section.
    Trade(bool),
    DraftPartner(Entity),
    DraftToggle(Entity),
    DraftMoney(i64),
    SetupSlot(usize),
    ToggleParkingPot,
    ToggleBuyBeforeRoll,
    StartGame,
}

impl Click {
    /// Game commands wait until the animations have caught up; UI clicks don't.
    fn is_game_command(self) -> bool {
        !matches!(
            self,
            Click::Select(_)
                | Click::CloseCard
                | Click::Trade(_)
                | Click::ConfirmFinish(_)
                | Click::DraftPartner(_)
                | Click::DraftToggle(_)
                | Click::DraftMoney(_)
                | Click::SetupSlot(_)
                | Click::ToggleParkingPot
                | Click::ToggleBuyBeforeRoll
                | Click::StartGame
                | Click::NewGameScreen
        )
    }
}

fn handle_clicks(
    q: Query<(&Interaction, &Click), Changed<Interaction>>,
    mut ui: ResMut<UiState>,
    mut setup: ResMut<SetupState>,
    busy: Res<Busy>,
    turn: Option<Res<Turn>>,
    deeds: Deeds,
    art: Res<Art>,
    time: Res<Time<Real>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut commands: Commands,
) {
    for (interaction, &click) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        art.play(&mut commands, Sound::Click);
        if click.is_game_command() && busy.0 {
            continue;
        }
        match click {
            Click::Roll => commands.trigger(Roll),
            Click::UseCancelCard => commands.trigger(UseCancelCard),
            Click::Buy => commands.trigger(Buy),
            Click::BuyRaces(n) => commands.trigger(BuyRaces(n)),
            Click::EndTurn => commands.trigger(EndTurn),
            Click::SellToBank(sq) => commands.trigger(SellToBank(sq)),
            Click::SellRace(sq) => commands.trigger(SellRace(sq)),
            Click::Liquidate => {
                if let Some(turn) = &turn {
                    commands.trigger(Liquidate(turn.player));
                }
            }
            Click::ProposeTrade => {
                if let Some(trade) = ui.draft.take().and_then(|d| d.trade()) {
                    commands.trigger(ProposeTrade(trade));
                }
            }
            Click::AcceptTrade(offer) => commands.trigger(AcceptTrade(offer)),
            Click::CancelTrade(offer) => commands.trigger(CancelTrade(offer)),
            Click::FinishGame => commands.trigger(FinishGame),
            Click::NewGameScreen => next_state.set(AppState::Setup),
            Click::ConfirmFinish(ask) => ui.confirm_finish = ask,
            Click::Select(sq) => ui.selected = Some(sq),
            Click::CloseCard => ui.selected = None,
            Click::Trade(open) => ui.draft = open.then(Draft::default),
            Click::DraftPartner(p) => ui.draft = Some(Draft { partner: Some(p), ..default() }),
            Click::DraftToggle(sq) => {
                if let Some(d) = &mut ui.draft {
                    // Own squares are given, the partner's taken; a stable with races moves whole.
                    let side = if deeds.owner(sq) == d.partner { &mut d.take } else { &mut d.give };
                    let group = deeds.trade_group(sq);
                    if side.contains(&sq) {
                        side.retain(|s| !group.contains(s));
                    } else {
                        side.extend(group.into_iter().filter(|s| !side.contains(s)).collect::<Vec<_>>());
                    }
                }
            }
            Click::DraftMoney(delta) => {
                if let Some(d) = &mut ui.draft {
                    d.money += delta;
                }
            }
            Click::SetupSlot(k) => setup.cycle(k),
            Click::ToggleParkingPot => setup.rules.parking_pot ^= true,
            Click::ToggleBuyBeforeRoll => setup.rules.buy_before_roll ^= true,
            Click::StartGame => {
                // When the button gets pressed is random enough, and works on wasm too.
                let seed = time.elapsed().as_nanos() as u64;
                commands.trigger(NewGame { players: setup.players(), seed, house_rules: setup.rules });
                *ui = UiState::default();
                next_state.set(AppState::Playing);
            }
        }
    }
}
