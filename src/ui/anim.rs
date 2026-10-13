//! Plays the rules' presentation events one after another so people can follow:
//! dice are thrown from the player's pawn into the middle, pawns hop square by square, banknotes fly between players and
//! the bank, cards pop up. `Busy` stays raised until the queue is empty.

use super::board::{BoardItem, Pawn, PotPile, Slot, place_on, rest_position};
use super::geometry::CENTER;
use super::widgets::bill_color;
use super::*;
use bevy::ecs::system::SystemParam;
use bevy::text::TextBounds;
use std::collections::VecDeque;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Script>()
        .add_observer(|ev: On<DiceRolled>, mut cue: Cue| cue.add(Beat::Dice(ev.player, ev.rolls.clone())))
        .add_observer(|ev: On<PawnMoved>, mut cue: Cue| cue.add(Beat::Walk(ev.player, ev.path.clone())))
        .add_observer(|ev: On<MoneyMoved>, mut cue: Cue| cue.add(Beat::Money(*ev)))
        .add_observer(|ev: On<CardDrawn>, mut cue: Cue| cue.add(Beat::Card(*ev)))
        .add_systems(OnEnter(AppState::Playing), |mut s: ResMut<Script>| *s = Script::default())
        .add_observer(|_: On<DismissCard>, mut s: ResMut<Script>| s.dismissed = true)
        .add_systems(Update, (dismiss_card, direct, walk, fly, tumble, pop_in, pulse_turn, fanfare).chain().run_if(in_state(AppState::Playing)));
}

enum Beat {
    Dice(Entity, Vec<u32>),
    Walk(Entity, Vec<Entity>),
    Money(MoneyMoved),
    Card(CardDrawn),
}

#[derive(Resource, Default)]
struct Script {
    queue: VecDeque<Beat>,
    /// The beat being played.
    playing: Option<Hold>,
    /// The shown card was clicked away.
    dismissed: bool,
}

/// How long a beat stays.
enum Hold {
    For(Timer),
    /// A drawn card stays until clicked away; a computer's also goes after a while.
    Card(Option<Timer>),
}

/// Closes the drawn Finance / Náhoda card, as clicking it does.
#[derive(Event)]
pub struct DismissCard;

/// Size of a drawn Finance / Náhoda card.
const CARD: Vec2 = Vec2::new(390.0, 250.0);

/// A click on the card, or Space / Enter, puts it away.
fn dismiss_card(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
    mut script: ResMut<Script>,
) {
    if !matches!(script.playing, Some(Hold::Card(_))) {
        return;
    }
    let (camera, transform) = *camera;
    let on_card = || {
        let pos = window.cursor_position().and_then(|c| camera.viewport_to_world_2d(transform, c).ok());
        pos.is_some_and(|p| ((p - CENTER).abs() - CARD / 2.0).max_element() <= 4.0)
    };
    if keys.any_just_pressed([KeyCode::Space, KeyCode::Enter]) || (mouse.just_pressed(MouseButton::Left) && on_card()) {
        script.dismissed = true;
    }
}

/// Queues a beat. `Busy` goes up right away, so nothing shows the rules'
/// new state (pawns, panel) before the animation got there.
#[derive(SystemParam)]
struct Cue<'w> {
    script: ResMut<'w, Script>,
    busy: ResMut<'w, Busy>,
}

impl Cue<'_> {
    fn add(&mut self, beat: Beat) {
        self.script.queue.push_back(beat);
        self.busy.0 = true;
    }
}

const HOP: f32 = 0.16;
/// How long one die rolls before it settles.
const THROW: f32 = 1.4;
/// After a six the next die is thrown once the six has settled.
const RETHROW: f32 = THROW + 0.4;

/// Starts the next beat when the current one is over.
fn direct(
    time: Res<Time>,
    mut script: ResMut<Script>,
    mut busy: ResMut<Busy>,
    mut ui: ResMut<UiState>,
    art: Res<Art>,
    pawns: Query<(Entity, &Pawn, &Transform)>,
    slots: Query<&Slot>,
    occupants: Query<&Occupants>,
    shown: Query<Entity, With<Shown>>,
    fleeting: Query<Entity, With<Fleeting>>,
    pot: Query<&Transform, With<PotPile>>,
    turn: Res<Turn>,
    ai: Query<(), With<Ai>>,
    mut rng: Local<Option<fastrand::Rng>>,
    time_real: Res<Time<Real>>,
    mut commands: Commands,
) {
    let rng = rng.get_or_insert_with(|| fastrand::Rng::with_seed(time_real.elapsed().as_nanos() as u64));
    let dismissed = std::mem::take(&mut script.dismissed);
    if let Some(hold) = &mut script.playing {
        let over = match hold {
            Hold::For(timer) => timer.tick(time.delta()).is_finished(),
            Hold::Card(timer) => dismissed || timer.as_mut().is_some_and(|t| t.tick(time.delta()).is_finished()),
        };
        if !over {
            return;
        }
        script.playing = None;
        for e in &fleeting {
            commands.entity(e).despawn();
        }
    }
    let Some(beat) = script.queue.pop_front() else {
        if busy.0 {
            busy.0 = false;
        }
        return;
    };
    let n = slots.iter().len();
    let slot_of = |sq: Entity| slots.iter().find(|s| s.square == sq).map_or(0, |s| s.index);
    let pawn_of = |p: Entity| pawns.iter().find(|(_, pawn, _)| pawn.0 == p);
    let hold = match beat {
        Beat::Dice(player, rolls) => {
            // A square's card left open would hide the dice.
            ui.selected = None;
            for e in &shown {
                commands.entity(e).despawn();
            }
            let from = pawn_of(player).map_or(CENTER, |(_, _, t)| t.translation.truncate());
            // Scattered on the felt, so where the first die lands doesn't tell whether more follow.
            let mut spots: Vec<Vec2> = Vec::new();
            for _ in &rolls {
                let spot = (0..100)
                    .map(|_| CENTER + Vec2::new(rng.f32() * 220.0 - 110.0, rng.f32() * 220.0 - 110.0))
                    .find(|p| spots.iter().all(|s| s.distance(*p) > 90.0))
                    .unwrap_or(CENTER);
                spots.push(spot);
            }
            for (k, (&value, to)) in rolls.iter().zip(spots).enumerate() {
                commands.spawn((
                    BoardItem,
                    Shown,
                    Tumble { value, from, to, elapsed: -(k as f32) * RETHROW, flips: None, spin: if rng.bool() { 1.0 } else { -1.0 } },
                    Sprite::from_color(CREAM, Vec2::splat(60.0)),
                    Transform::from_translation(from.extend(8.0)).with_scale(Vec3::ZERO),
                ));
            }
            Hold::For(Timer::from_seconds((rolls.len() - 1) as f32 * RETHROW + THROW + 0.6, TimerMode::Once))
        }
        Beat::Walk(player, path) => {
            let Some((pawn, _, transform)) = pawn_of(player) else { return };
            let last = path.len() - 1;
            let mut points: Vec<Vec2> = path.iter().map(|&sq| rest_position(slot_of(sq), n, 0, 1)).collect();
            let (k, count) = place_on(&occupants, path[last], player);
            points[last] = rest_position(slot_of(path[last]), n, k, count);
            commands.entity(pawn).insert(Walk { from: transform.translation.truncate(), points, elapsed: 0.0 });
            if path.len() > 1 {
                art.play(&mut commands, Sound::Horse);
            }
            Hold::For(Timer::from_seconds(path.len() as f32 * HOP + 0.15, TimerMode::Once))
        }
        Beat::Money(MoneyMoved { from, to, amount }) => {
            let at = |party: Party| match party {
                Party::Bank => CENTER,
                Party::Player(p) => pawn_of(p).map_or(CENTER, |(_, _, t)| t.translation.truncate()),
                Party::Parking => pot.single().map_or(CENTER, |t| t.translation.truncate()),
            };
            art.play(&mut commands, if to == Party::Bank { Sound::Register } else { Sound::Coins });
            commands
                .spawn((
                    BoardItem,
                    Flight { from: at(from), to: at(to), elapsed: 0.0 },
                    Sprite::from_color(bill_color(amount), Vec2::new(86.0, 40.0)),
                    Transform::from_translation(at(from).extend(9.0)),
                ))
                .with_children(|b| {
                    b.spawn((art.label(money(amount), 15.0, INK), Transform::from_xyz(0.0, 0.0, 0.1)));
                });
            Hold::For(Timer::from_seconds(0.6, TimerMode::Once))
        }
        Beat::Card(CardDrawn { deck, text }) => {
            art.play(&mut commands, Sound::Card);
            let (title, field, color) = match deck {
                Deck::Finance => ("FINANCE", Field::Finance, Color::srgb(0.16, 0.36, 0.62)),
                Deck::Nahoda => ("NÁHODA", Field::Nahoda, Color::srgb(0.72, 0.18, 0.14)),
            };
            const SIZE: Vec2 = CARD;
            // A paper card with a coloured header, big enough to read across the table.
            commands
                .spawn((
                    BoardItem,
                    Fleeting,
                    PopIn(0.0),
                    Sprite::from_color(INK, SIZE + 8.0),
                    Transform::from_translation(CENTER.extend(10.0)).with_scale(Vec3::ZERO),
                ))
                .with_children(|c| {
                    c.spawn((Sprite::from_color(CREAM, SIZE), Transform::from_xyz(0.0, 0.0, 0.1)));
                    c.spawn((Sprite::from_color(color, Vec2::new(SIZE.x, 56.0)), Transform::from_xyz(0.0, SIZE.y / 2.0 - 28.0, 0.2)));
                    if let Some(icon) = art.icon(&field) {
                        for x in [-1.0, 1.0] {
                            c.spawn((
                                Sprite { image: art.icons.clone(), texture_atlas: Some(icon.clone()), custom_size: Some(Vec2::splat(40.0)), ..default() },
                                Transform::from_xyz(x * (SIZE.x / 2.0 - 34.0), SIZE.y / 2.0 - 28.0, 0.3),
                            ));
                        }
                    }
                    c.spawn((art.label(title, 30.0, Color::WHITE), Transform::from_xyz(0.0, SIZE.y / 2.0 - 28.0, 0.3)));
                    c.spawn((
                        art.label(text, 24.0, INK),
                        TextBounds::new_horizontal(SIZE.x - 40.0),
                        TextLayout { justify: Justify::Center, ..default() },
                        Transform::from_xyz(0.0, -20.0, 0.3),
                    ));
                    c.spawn((art.label("klikni na kartu", 13.0, MUTED_INK), Transform::from_xyz(0.0, -SIZE.y / 2.0 + 16.0, 0.3)));
                });
            // People read at their own pace; the computer's cards go by themselves.
            let computer = ai.contains(turn.player);
            Hold::Card(computer.then(|| Timer::from_seconds(2.5, TimerMode::Once)))
        }
    };
    script.playing = Some(hold);
}

/// The dice of the current turn; cleared by the next roll.
#[derive(Component)]
struct Shown;

/// Gone when its beat is over, like a drawn card – it would hide the board.
#[derive(Component)]
struct Fleeting;

#[derive(Component)]
struct Walk {
    from: Vec2,
    points: Vec<Vec2>,
    elapsed: f32,
}

/// Pawns hop from square to square with a little jump.
fn walk(time: Res<Time>, mut q: Query<(Entity, &mut Walk, &mut Transform)>, mut commands: Commands) {
    for (e, mut w, mut t) in &mut q {
        w.elapsed += time.delta_secs();
        let step = ((w.elapsed / HOP) as usize).min(w.points.len());
        if step == w.points.len() {
            t.translation = w.points[step - 1].extend(t.translation.z);
            t.scale = Vec3::ONE;
            commands.entity(e).remove::<Walk>();
            continue;
        }
        let from = if step == 0 { w.from } else { w.points[step - 1] };
        let f = w.elapsed / HOP - step as f32;
        let jump = (f * std::f32::consts::PI).sin();
        t.translation = from.lerp(w.points[step], f).extend(t.translation.z);
        t.scale = Vec3::splat(1.0 + 0.35 * jump);
    }
}

#[derive(Component)]
struct Flight {
    from: Vec2,
    to: Vec2,
    elapsed: f32,
}

/// Banknotes fly from payer to payee and vanish.
fn fly(time: Res<Time>, mut q: Query<(Entity, &mut Flight, &mut Transform)>, mut commands: Commands) {
    for (e, mut f, mut t) in &mut q {
        f.elapsed += time.delta_secs();
        let k = (f.elapsed / 0.5).min(1.0);
        let eased = k * k * (3.0 - 2.0 * k);
        t.translation = (f.from.lerp(f.to, eased) + Vec2::Y * 60.0 * (k * std::f32::consts::PI).sin()).extend(9.0);
        t.rotation = Quat::from_rotation_z((1.0 - k) * 0.6);
        if k >= 1.0 {
            commands.entity(e).despawn();
        }
    }
}

#[derive(Component)]
struct Tumble {
    value: u32,
    from: Vec2,
    to: Vec2,
    /// Seconds since the throw; negative while waiting for the previous die.
    elapsed: f32,
    /// Quarter-turns so far, each showing a new face; `None` until thrown.
    flips: Option<u32>,
    /// Direction of the spin.
    spin: f32,
}

/// A die flies from the thrower to the middle, bouncing ever lower, spinning
/// ever slower and flipping faces until it settles on the rolled value.
fn tumble(time: Res<Time>, art: Res<Art>, mut q: Query<(Entity, &mut Tumble, &mut Transform)>, mut rng: Local<Option<fastrand::Rng>>, mut commands: Commands) {
    let rng = rng.get_or_insert_with(|| fastrand::Rng::with_seed(9));
    for (e, mut die, mut t) in &mut q {
        die.elapsed += time.delta_secs();
        if die.elapsed < 0.0 {
            continue;
        }
        let k = (die.elapsed / THROW).min(1.0);
        let travelled = 1.0 - (1.0 - k).powi(3);
        let bounce = (k * 3.0 * std::f32::consts::PI).sin().abs() * (1.0 - k);
        t.translation = (die.from.lerp(die.to, travelled) + Vec2::Y * 50.0 * bounce).extend(8.0);
        t.scale = Vec3::splat(0.6 + 0.4 * (k * 4.0).min(1.0) + 0.3 * bounce);
        t.rotation = Quat::from_rotation_z(die.spin * ((1.0 - k).powi(2) * 14.0 + 0.12));
        // Faces flip with the spin, so they slow down along with it.
        let flips = if k >= 1.0 { u32::MAX } else { (travelled * 14.0) as u32 };
        if die.flips == Some(flips) {
            continue;
        }
        if die.flips.is_none() {
            art.play(&mut commands, Sound::Dice);
        }
        die.flips = Some(flips);
        let face = if k >= 1.0 { die.value } else { rng.u32(1..=6) };
        commands.entity(e).despawn_children().with_children(|d| {
            for (x, y) in pips(face) {
                d.spawn((Sprite::from_color(INK, Vec2::splat(11.0)), Transform::from_xyz(x * 17.0, y * 17.0, 0.1)));
            }
        });
    }
}

fn pips(face: u32) -> &'static [(f32, f32)] {
    match face {
        1 => &[(0.0, 0.0)],
        2 => &[(-1.0, 1.0), (1.0, -1.0)],
        3 => &[(-1.0, 1.0), (0.0, 0.0), (1.0, -1.0)],
        4 => &[(-1.0, 1.0), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)],
        5 => &[(-1.0, 1.0), (1.0, 1.0), (0.0, 0.0), (-1.0, -1.0), (1.0, -1.0)],
        _ => &[(-1.0, 1.0), (1.0, 1.0), (-1.0, 0.0), (1.0, 0.0), (-1.0, -1.0), (1.0, -1.0)],
    }
}

#[derive(Component)]
struct PopIn(f32);

fn pop_in(time: Res<Time>, mut q: Query<(&mut PopIn, &mut Transform)>) {
    for (mut p, mut t) in &mut q {
        p.0 = (p.0 + time.delta_secs() / 0.3).min(1.0);
        let overshoot = 1.0 + 0.15 * (p.0 * std::f32::consts::PI).sin();
        t.scale = Vec3::splat(p.0 * overshoot);
    }
}

/// The pawn whose turn it is gently pulses.
fn pulse_turn(time: Res<Time>, turn: Res<Turn>, busy: Res<Busy>, mut pawns: Query<(&Pawn, &mut Transform), Without<Walk>>) {
    for (pawn, mut t) in &mut pawns {
        let pulse = if pawn.0 == turn.player && !busy.0 { 1.0 + 0.12 * (time.elapsed_secs() * 5.0).sin() } else { 1.0 };
        t.scale = Vec3::splat(pulse);
    }
}

/// A laugh for every bankruptcy and a gong for the winner.
fn fanfare(phase: Res<Phase>, eliminated: Res<Eliminated>, busy: Res<Busy>, art: Res<Art>, mut celebrated: Local<bool>, mut commands: Commands) {
    if busy.0 {
        return;
    }
    if eliminated.is_changed() && !eliminated.0.is_empty() {
        art.play(&mut commands, Sound::Laugh);
    }
    match *phase {
        Phase::GameOver { .. } if !*celebrated => {
            *celebrated = true;
            art.play(&mut commands, Sound::Gong);
        }
        Phase::GameOver { .. } => {}
        _ => *celebrated = false,
    }
}
