//! The cake-shaped board in world space: wedges with names and photos, the
//! grey track showing owners, race tokens on the horses, and the pawns.

use super::geometry::*;
use super::widgets::bill_color;
use super::*;
use bevy::input::common_conditions::input_just_pressed;
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use bevy::text::TextBounds;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Playing), spawn_board)
        .add_systems(OnExit(AppState::Playing), despawn::<BoardItem>)
        .add_systems(
            Update,
            (
                select_square.run_if(input_just_pressed(MouseButton::Left)),
                (show_owners, show_races, show_selection, show_pot).run_if(not_busy),
                rest_pawns.run_if(not_busy),
            )
                .run_if(in_state(AppState::Playing)),
        );
}

pub fn not_busy(busy: Res<Busy>) -> bool {
    !busy.0
}

pub fn despawn<T: Component>(q: Query<Entity, With<T>>, mut commands: Commands) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

/// Everything drawn for a game; removed when leaving it.
#[derive(Component)]
pub struct BoardItem;

/// Board position of a square, on its wedge.
#[derive(Component, Clone, Copy)]
pub struct Slot {
    pub index: usize,
    pub square: Entity,
}

/// The colours of a square: its wedge and its piece of the track.
#[derive(Component)]
struct SquareVisual {
    wedge: Handle<ColorMaterial>,
    base: Color,
    track: Handle<ColorMaterial>,
}

/// Holds the race-token sprites of a square.
#[derive(Component)]
struct TokenHolder(Entity);

/// Where the fines for Parkoviště lie, in the middle of the board (house rule).
#[derive(Component)]
pub struct PotPile;

/// The sprite of a player's pawn.
#[derive(Component)]
pub struct Pawn(pub Entity);

const Z_BOARD: f32 = 0.0;
const Z_PAWN: f32 = 5.0;

fn spawn_board(
    mut commands: Commands,
    track: Track,
    players: Query<(Entity, &Player)>,
    pot: Option<Res<Pot>>,
    art: Res<Art>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let squares: Vec<Entity> = track.squares().collect();
    let n = squares.len();
    let mut shape = |mesh: Mesh, color: Color, z: f32| {
        (
            BoardItem,
            Mesh2d(meshes.add(mesh)),
            MeshMaterial2d(materials.add(color)),
            Transform::from_translation(CENTER.extend(Z_BOARD + z)),
        )
    };
    commands.spawn(shape(ring(0.0, FRAME), FELT_DARK, 0.0));
    commands.spawn(shape(ring(TRACK - 6.0, OUTER + 4.0), INK, 0.1));
    commands.spawn(shape(ring(0.0, TRACK - 6.0), FELT, 0.2));

    for (i, &sq) in squares.iter().enumerate() {
        let field = *track.field(sq);
        let base = field.stable().map_or(CREAM, Stable::color);
        let wedge = materials.add(base);
        let track_color = materials.add(TRACK_GREY);
        commands.spawn((
            BoardItem,
            Slot { index: i, square: sq },
            SquareVisual { wedge: wedge.clone(), base, track: track_color.clone() },
            Mesh2d(meshes.add(geometry::wedge(i, n, INNER, OUTER))),
            MeshMaterial2d(wedge),
            Transform::from_translation(CENTER.extend(Z_BOARD + 1.0)),
        ));
        commands.spawn((
            BoardItem,
            Mesh2d(meshes.add(geometry::wedge(i, n, TRACK, INNER - 4.0))),
            MeshMaterial2d(track_color),
            Transform::from_translation(CENTER.extend(Z_BOARD + 1.0)),
        ));
        // Race tokens lie in a row at the foot of the horse's photo, away from the pawns.
        let tokens_at = Transform::from_translation(spot(i, n, OUTER - 52.0).extend(3.0)).with_rotation(radial_rotation(i, n));
        commands.spawn((BoardItem, TokenHolder(sq), tokens_at, Visibility::default()));
        spawn_square_art(&mut commands, &art, &field, i, n);
    }
    if pot.is_some() {
        commands.spawn((BoardItem, PotPile, Transform::from_translation((CENTER - Vec2::Y * 105.0).extend(3.0)), Visibility::default()));
    }

    for (p, player) in &players {
        commands
            .spawn((
                BoardItem,
                Pawn(p),
                Sprite { image: art.pawn.clone(), color: player.color, custom_size: Some(Vec2::splat(26.0)), ..default() },
                Transform::from_translation(rest_position(0, n, 0, 1).extend(Z_PAWN)),
            ))
            .with_children(|pawn| {
                // A dark outline keeps the pawn visible on a track of its own colour.
                pawn.spawn((Sprite { image: art.pawn.clone(), color: INK, custom_size: Some(Vec2::splat(31.0)), ..default() }, Transform::from_xyz(0.0, 0.0, -0.01)));
            });
    }
}

/// Name, price and photo or icon of a square, laid out along its wedge.
fn spawn_square_art(commands: &mut Commands, art: &Art, field: &Field, i: usize, n: usize) {
    let rotation = radial_rotation(i, n);
    let at = |r: f32| Transform::from_translation(spot(i, n, r).extend(2.0)).with_rotation(rotation);
    let ink = super::widgets::ink_on(field);
    // Along the wedge from the inside: price, name, picture.
    commands.spawn((
        BoardItem,
        art.label(field.name(), 12.0, ink),
        TextBounds::new_horizontal(78.0),
        TextLayout { justify: Justify::Center, ..default() },
        at(INNER + 82.0),
    ));
    if let Some(price) = field.price().or(match field {
        Field::Vet(fee) => Some(*fee),
        _ => None,
    }) {
        commands.spawn((BoardItem, art.label(money(price), 10.0, ink), at(INNER + 22.0)));
    }
    let picture = match field {
        Field::Horse(h) => Some(Sprite { image: art.photo(h), custom_size: Some(Vec2::new(52.0, 50.0)), ..default() }),
        f => art.icon(f).map(|icon| Sprite { image: art.icons.clone(), texture_atlas: Some(icon), custom_size: Some(Vec2::splat(34.0)), ..default() }),
    };
    if let Some(sprite) = picture {
        commands.spawn((BoardItem, sprite, at(OUTER - 27.0)));
    }
}

/// The track segment of an owned square takes the owner's colour.
fn show_owners(visuals: Query<(&Slot, &SquareVisual)>, owners: Query<&OwnedBy>, players: Query<&Player>, mut materials: ResMut<Assets<ColorMaterial>>) {
    for (slot, v) in &visuals {
        let color = owners.get(slot.square).ok().and_then(|o| players.get(o.0).ok()).map_or(TRACK_GREY, |p| p.color);
        if materials.get(&v.track).is_some_and(|m| m.color != color) {
            materials.get_mut(&v.track).unwrap().color = color;
        }
    }
}

fn show_races(holders: Query<(Entity, &TokenHolder)>, races: Query<&Races, Changed<Races>>, art: Res<Art>, mut commands: Commands) {
    for (holder, TokenHolder(sq)) in &holders {
        let Ok(races) = races.get(*sq) else { continue };
        commands.entity(holder).despawn_children();
        let tokens: Vec<(Handle<Image>, Vec2, f32)> = match races {
            Races::Main => vec![(art.main_race.clone(), Vec2::ZERO, 20.0)],
            r => (0..r.level()).map(|k| (art.race.clone(), Vec2::new(0.0, (k as f32 - (r.level() as f32 - 1.0) / 2.0) * 13.0), 13.0)).collect(),
        };
        commands.entity(holder).with_children(|h| {
            for (image, offset, size) in tokens {
                h.spawn((Sprite { image, custom_size: Some(Vec2::splat(size)), ..default() }, Transform::from_translation(offset.extend(0.0))));
            }
        });
    }
}

/// The fines for Parkoviště as a little pile of banknotes.
fn show_pot(pile: Single<Entity, With<PotPile>>, pot: Option<Res<Pot>>, art: Res<Art>, mut commands: Commands) {
    let Some(pot) = pot.filter(|p| p.is_changed()) else { return };
    commands.entity(*pile).despawn_children().with_children(|p| {
        if pot.0 == 0 {
            return;
        }
        for k in 0..3 {
            let offset = Vec3::new(k as f32 * 3.0 - 3.0, k as f32 * 3.0 - 3.0, k as f32 * 0.01);
            p.spawn((Sprite::from_color(bill_color(pot.0), Vec2::new(86.0, 40.0)), Transform::from_translation(offset).with_rotation(Quat::from_rotation_z(0.08 * (k as f32 - 1.0)))));
        }
        p.spawn((art.label(money(pot.0), 15.0, INK), Transform::from_xyz(3.0, 3.0, 0.1)));
        p.spawn((art.label("pro parkoviště", 13.0, CREAM), Transform::from_xyz(0.0, -32.0, 0.1)));
    });
}

fn show_selection(ui: Res<UiState>, visuals: Query<(&Slot, &SquareVisual)>, mut materials: ResMut<Assets<ColorMaterial>>) {
    if !ui.is_changed() {
        return;
    }
    for (slot, v) in &visuals {
        if let Some(mut m) = materials.get_mut(&v.wedge) {
            m.color = if ui.selected == Some(slot.square) { v.base.mix(&Color::WHITE, 0.45) } else { v.base };
        }
    }
}

fn select_square(
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
    slots: Query<&Slot>,
    mut ui: ResMut<UiState>,
) {
    let (camera, transform) = *camera;
    let Some(pos) = window.cursor_position().and_then(|c| camera.viewport_to_world_2d(transform, c).ok()) else { return };
    let Some(i) = square_at(pos, slots.iter().len()) else { return };
    ui.selected = slots.iter().find(|s| s.index == i).map(|s| s.square);
}

/// Where the `k`-th of `count` pawns on a square stands: in single file across
/// the track, in the middle of the square's slice, closer together when crowded.
pub fn rest_position(slot: usize, n: usize, k: usize, count: usize) -> Vec2 {
    let gap = if count > 1 { (50.0 / (count - 1) as f32).min(24.0) } else { 0.0 };
    spot(slot, n, INNER - 18.0 - k as f32 * gap)
}

/// `(k, count)` of a player's pawn among those on `square`.
pub fn place_on(occupants: &Query<&Occupants>, square: Entity, player: Entity) -> (usize, usize) {
    occupants.get(square).map_or((0, 1), |o| (o.iter().position(|p| p == player).unwrap_or(0), o.len()))
}

/// Puts resting pawns where they belong; bankrupt players' pawns disappear.
fn rest_pawns(
    mut pawns: Query<(Entity, &Pawn, &mut Transform)>,
    players: Query<&OnSquare>,
    occupants: Query<&Occupants>,
    slots: Query<&Slot>,
    mut commands: Commands,
) {
    let n = slots.iter().len();
    for (e, pawn, mut transform) in &mut pawns {
        let Ok(on) = players.get(pawn.0) else {
            commands.entity(e).despawn();
            continue;
        };
        let Some(slot) = slots.iter().find(|s| s.square == on.0) else { continue };
        let (k, count) = place_on(&occupants, on.0, pawn.0);
        transform.translation = rest_position(slot.index, n, k, count).extend(Z_PAWN + k as f32 * 0.01);
    }
}
