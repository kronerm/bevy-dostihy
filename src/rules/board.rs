//! The board: a ring of square entities linked with `Next` / `Prev`, ownership
//! of squares, and race tokens on horses.

use super::*;

pub(super) fn plugin(app: &mut App) {
    app.add_observer(clear_races_when_unowned);
}

/// A board square and what it is.
#[derive(Component, Debug)]
pub struct Square(pub Field);

#[derive(Component, Debug)]
pub struct Next(pub Entity);

#[derive(Component, Debug)]
pub struct Prev(pub Entity);

#[derive(Resource, Debug)]
pub struct StartSquare(pub Entity);

/// The square a pawn stands on.
#[derive(Component, Debug)]
#[relationship(relationship_target = Occupants)]
pub struct OnSquare(pub Entity);

#[derive(Component, Debug, Default)]
#[relationship_target(relationship = OnSquare)]
pub struct Occupants(Vec<Entity>);

#[derive(Component, Debug)]
#[relationship(relationship_target = Owns)]
pub struct OwnedBy(pub Entity);

#[derive(Component, Debug, Default)]
#[relationship_target(relationship = OwnedBy)]
pub struct Owns(Vec<Entity>);

/// Race tokens on a horse (*dostihy*): up to four yellow ones, or the red main race.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Races {
    #[default]
    None,
    One,
    Two,
    Three,
    Four,
    Main,
}

impl Races {
    pub const ALL: [Races; 6] = [Races::None, Races::One, Races::Two, Races::Three, Races::Four, Races::Main];

    /// Index into `Horse::fees`, and also the number of race costs paid so far.
    pub fn level(self) -> usize {
        self as usize
    }

    pub fn from_level(level: usize) -> Option<Races> {
        Races::ALL.get(level).copied()
    }

    /// How many tokens can be bought on one visit; with four, only the main race.
    pub fn buyable(self) -> std::ops::RangeInclusive<usize> {
        match self {
            Races::Four => 1..=1,
            #[allow(clippy::reversed_empty_ranges)] // nothing left to buy
            Races::Main => 1..=0,
            r => 1..=4 - r.level(),
        }
    }
}

/// The horse got races on this visit; one purchase per visit.
#[derive(Component, Debug)]
pub struct RacedThisTurn;

/// A horse that loses its owner (sold to the bank, owner bankrupt) loses its races.
fn clear_races_when_unowned(ev: On<Remove, OwnedBy>, mut races: Query<&mut Races>) {
    if let Ok(mut r) = races.get_mut(ev.entity) {
        *r = Races::None;
    }
}

/// Spawns the ring of squares described by `BOARD`; returns Start.
pub(super) fn spawn(commands: &mut Commands) -> Entity {
    let squares: Vec<Entity> = BOARD
        .iter()
        .map(|&field| {
            let mut e = commands.spawn(Square(field));
            if field.stable().is_some() {
                e.insert(Races::None);
            }
            e.id()
        })
        .collect();
    let n = squares.len();
    for (i, &sq) in squares.iter().enumerate() {
        commands.entity(sq).insert((Next(squares[(i + 1) % n]), Prev(squares[(i + n - 1) % n])));
    }
    commands.insert_resource(StartSquare(squares[0]));
    squares[0]
}

/// Walks the ring of squares.
#[derive(SystemParam)]
pub struct Track<'w, 's> {
    start: Res<'w, StartSquare>,
    squares: Query<'w, 's, (&'static Square, &'static Next, &'static Prev)>,
}

impl Track<'_, '_> {
    pub fn start(&self) -> Entity {
        self.start.0
    }

    pub fn field(&self, sq: Entity) -> &Field {
        &self.squares.get(sq).expect("square").0 .0
    }

    pub fn step(&self, sq: Entity, direction: Direction) -> Entity {
        let (_, next, prev) = self.squares.get(sq).expect("square");
        match direction {
            Direction::Forward => next.0,
            Direction::Back => prev.0,
        }
    }

    /// All squares in board order, starting at Start.
    pub fn squares(&self) -> impl Iterator<Item = Entity> + '_ {
        let start = self.start();
        std::iter::successors(Some(start), move |&s| Some(self.step(s, Direction::Forward)).filter(|&n| n != start))
    }

    fn is(&self, sq: Entity, landmark: Landmark) -> bool {
        match (landmark, self.field(sq)) {
            (Landmark::Start, Field::Start)
            | (Landmark::Finance, Field::Finance)
            | (Landmark::Trainer, Field::Trainer(_))
            | (Landmark::Distanc, Field::Distanc)
            | (Landmark::Parking, Field::Parking) => true,
            (Landmark::LastSquare, _) => self.step(sq, Direction::Forward) == self.start(),
            _ => false,
        }
    }

    /// The squares passed on the way, ending at the destination.
    pub fn walk(&self, from: Entity, direction: Direction, to: Destination) -> Vec<Entity> {
        let mut path = Vec::new();
        let mut at = from;
        loop {
            at = self.step(at, direction);
            path.push(at);
            let arrived = match to {
                Destination::Steps(n) => path.len() == n,
                Destination::Nearest(landmark) => self.is(at, landmark),
            };
            if arrived {
                return path;
            }
        }
    }
}

/// The cards players own: the squares' deeds, with races on horses, and the
/// *Zrušen distanc* card.
#[derive(SystemParam)]
pub struct Deeds<'w, 's> {
    cards: Query<
        'w,
        's,
        (Entity, Option<&'static Square>, Option<&'static OwnedBy>, Option<&'static mut Races>, Has<RacedThisTurn>),
        Or<(With<Square>, With<CancelCard>)>,
    >,
}

impl Deeds<'_, '_> {
    pub fn field(&self, sq: Entity) -> &Field {
        self.square(sq).expect("square")
    }

    /// The square of a deed; `None` for the *Zrušen distanc* card.
    pub fn square(&self, card: Entity) -> Option<&Field> {
        self.cards.get(card).ok()?.1.map(|s| &s.0)
    }

    pub fn name(&self, card: Entity) -> String {
        self.square(card).map_or("Zrušen distanc".into(), Field::name)
    }

    pub fn stable(&self, card: Entity) -> Option<Stable> {
        self.square(card)?.stable()
    }

    pub fn cancel_card(&self) -> Entity {
        self.cards.iter().find(|c| c.1.is_none()).expect("the game has the card").0
    }

    pub fn holds_cancel_card(&self, p: Entity) -> bool {
        self.owner(self.cancel_card()) == Some(p)
    }

    pub fn owner(&self, sq: Entity) -> Option<Entity> {
        self.cards.get(sq).ok()?.2.map(|o| o.0)
    }

    pub fn races(&self, sq: Entity) -> Races {
        self.cards.get(sq).ok().and_then(|s| s.3.copied()).unwrap_or_default()
    }

    pub(super) fn set_races(&mut self, sq: Entity, races: Races) {
        if let Ok((.., Some(mut r), _)) = self.cards.get_mut(sq) {
            *r = races;
        }
    }

    /// The squares `p` owns (not the *Zrušen distanc* card).
    pub fn owned_by(&self, p: Entity) -> Vec<Entity> {
        self.cards.iter().filter(|c| c.1.is_some() && c.2.is_some_and(|o| o.0 == p)).map(|c| c.0).collect()
    }

    pub fn stable_squares(&self, stable: Stable) -> impl Iterator<Item = Entity> + '_ {
        self.cards.iter().filter(move |c| c.1.is_some_and(|s| s.0.stable() == Some(stable))).map(|c| c.0)
    }

    pub fn owns_stable(&self, p: Entity, stable: Stable) -> bool {
        self.owns_stable_with(p, stable, &[])
    }

    /// Whether `p` would own the whole stable after also getting `extra`.
    pub fn owns_stable_with(&self, p: Entity, stable: Stable, extra: &[Entity]) -> bool {
        self.stable_squares(stable).all(|sq| self.owner(sq) == Some(p) || extra.contains(&sq))
    }

    pub fn stable_has_races(&self, stable: Stable) -> bool {
        self.stable_squares(stable).any(|sq| self.races(sq) > Races::None)
    }

    /// `p` owns the horse's whole stable, it isn't in the main race yet and
    /// got no races on this visit.
    pub fn can_add_races(&self, p: Entity, sq: Entity) -> bool {
        let raced = self.cards.get(sq).is_ok_and(|s| s.4);
        self.field(sq).stable().is_some_and(|s| self.owns_stable(p, s)) && self.races(sq) < Races::Main && !raced
    }

    /// Cards go back to the bank only from stables without races.
    pub fn can_sell_to_bank(&self, p: Entity, sq: Entity) -> bool {
        self.owner(sq) == Some(p) && self.square(sq).is_some_and(|f| f.price().is_some()) && !self.stable(sq).is_some_and(|s| self.stable_has_races(s))
    }

    /// Cards that change hands together with `sq`: a stable with races goes whole.
    pub fn trade_group(&self, sq: Entity) -> Vec<Entity> {
        match self.stable(sq) {
            Some(s) if self.stable_has_races(s) => self.stable_squares(s).collect(),
            _ => vec![sq],
        }
    }

    /// Purchase price of the card plus everything paid for its races; the
    /// *Zrušen distanc* card has no price.
    pub fn value(&self, sq: Entity) -> i64 {
        let Some(field) = self.square(sq) else { return 0 };
        let races = field.stable().map_or(0, |s| s.race_cost() * self.races(sq).level() as i64);
        field.price().unwrap_or(0) + races
    }

    pub fn net_worth(&self, p: Entity, money: i64) -> i64 {
        money + self.owned_by(p).into_iter().map(|sq| self.value(sq)).sum::<i64>()
    }

    fn count_owned(&self, p: Entity, pred: impl Fn(&Field) -> bool) -> usize {
        self.owned_by(p).into_iter().filter(|&sq| pred(self.field(sq))).count()
    }

    /// What an opponent pays for stopping on `sq`.
    pub fn fee(&self, sq: Entity, owner: Entity, owner_status: Status, roll: i64) -> i64 {
        match self.field(sq) {
            Field::Horse(h) if owner_status.suspended() => h.fees[0],
            Field::Horse(h) => h.fees[self.races(sq).level()],
            Field::Trainer(_) => TRAINER_FEE * self.count_owned(owner, |f| matches!(f, Field::Trainer(_))) as i64,
            Field::Service(_) => {
                let both = self.count_owned(owner, |f| matches!(f, Field::Service(_))) == 2;
                roll * if both { BOTH_SERVICES_FEE } else { SERVICE_FEE }
            }
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_util::*;
    use super::*;

    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn track_is_closed_and_ordered() {
        let mut app = app(2);
        let names = app.world_mut().run_system_once(|track: Track| track.squares().map(|s| track.field(s).name()).collect::<Vec<_>>()).unwrap();
        assert_eq!(names.len(), BOARD.len());
        assert_eq!(names.first().unwrap(), "Start");
        assert_eq!(names.last().unwrap(), "Napoli");
    }

    #[test]
    fn walking_finds_landmarks_both_ways() {
        let mut app = app(2);
        let nahoda = square(&mut app, "Náhoda");
        let names = app.world_mut().run_system_once(move |track: Track| {
            let end = |dir, to| track.field(*track.walk(nahoda, dir, to).last().unwrap()).name();
            [
                end(Direction::Back, Destination::Nearest(Landmark::LastSquare)),
                end(Direction::Back, Destination::Nearest(Landmark::Start)),
                end(Direction::Forward, Destination::Nearest(Landmark::Trainer)),
                end(Direction::Back, Destination::Steps(3)),
            ]
        })
        .unwrap();
        assert_eq!(names[0], "Napoli");
        assert_eq!(names[1], "Start");
        assert!(names[2].ends_with("Trenér"));
    }

    #[test]
    fn losing_an_owner_clears_races() {
        let mut app = app(2);
        give(&mut app, 0, "Fantome", Races::Three);
        let sq = square(&mut app, "Fantome");
        app.world_mut().entity_mut(sq).remove::<OwnedBy>();
        assert_eq!(races(&mut app, "Fantome"), Races::None);
    }
}
