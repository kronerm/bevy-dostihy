//! Game rules as ECS. Players and board squares are entities; player actions
//! and rule effects are events handled by observers. The rules apply effects
//! instantly and announce them with presentation events (`DiceRolled`,
//! `PawnMoved`, `MoneyMoved`, `CardDrawn`) that the UI may animate.

mod board;
mod cards;
mod market;
mod turn;

pub use board::*;
pub use market::*;
pub use turn::*;

use crate::data::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use std::collections::VecDeque;

pub struct RulesPlugin;

impl Plugin for RulesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameLog>()
            .add_observer(on_new_game)
            .add_plugins((board::plugin, turn::plugin, cards::plugin, market::plugin));
    }
}

// ------------------------------------------------------------------ players

#[derive(Component, Debug, Clone)]
pub struct Player {
    pub name: String,
    pub color: Color,
    /// Turn order.
    pub seat: usize,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Money(pub i64);

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Free,
    /// Waits on the Distanc square for a 6.
    Distanc,
    /// Sits out turns; with `doping` the player's horses don't race meanwhile.
    Sitting { turns: u8, doping: bool },
}

impl Status {
    /// A suspended owner only collects the stable-visit fee.
    pub fn suspended(self) -> bool {
        matches!(self, Status::Distanc | Status::Sitting { doping: true, .. })
    }
}

/// Controlled by the computer.
#[derive(Component)]
pub struct Ai;

/// The *Zrušen distanc* card. A player keeps it (`OwnedBy`) until they use
/// it to leave Distanc, or trade it; otherwise it's in the Náhoda deck.
#[derive(Component)]
pub struct CancelCard;

/// Whose turn it is. Bankrupt players are despawned, so this is always a live player.
#[derive(Resource, Debug)]
pub struct Turn {
    pub player: Entity,
}

/// Players who went bankrupt, in order; kept only for display.
#[derive(Resource, Default, Debug)]
pub struct Eliminated(pub Vec<Player>);

// -------------------------------------------------------------------- phase

/// A deal between the proposer and `partner`: cards both ways, plus money.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trade {
    pub partner: Entity,
    /// Cards the proposer hands over: squares, maybe the *Zrušen distanc* card.
    pub give: Vec<Entity>,
    /// Cards the partner hands over.
    pub take: Vec<Entity>,
    /// Money the proposer pays the partner; negative when the partner pays.
    pub money: i64,
}

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Before rolling.
    PreRoll,
    /// After moving; buying and proposing trades happen now. `offer` is the
    /// unowned square the player stopped on, for sale until bought or the turn ends.
    AfterMove { offer: Option<Entity> },
    GameOver { winner: Option<Entity> },
}

impl Phase {
    pub fn offer(&self) -> Option<Entity> {
        match self {
            Phase::AfterMove { offer, .. } => *offer,
            _ => None,
        }
    }

    pub fn can_end_turn(&self, money: i64) -> bool {
        matches!(self, Phase::AfterMove { .. }) && money >= 0
    }

    fn set_offer(&mut self, o: Option<Entity>) {
        if let Phase::AfterMove { offer, .. } = self {
            *offer = o;
        }
    }
}

// ---------------------------------------------------------------- resources

/// Card indices into `FINANCE` / `NAHODA`, drawn from the front and returned to the back.
#[derive(Resource)]
pub struct Decks {
    finance: VecDeque<usize>,
    nahoda: VecDeque<usize>,
}

#[derive(Resource)]
pub struct GameRng(pub fastrand::Rng);

/// The last move's dice total – Přeprava / Stáje fees multiply it.
#[derive(Resource, Default)]
pub struct LastRoll(pub i64);

#[derive(Resource, Default)]
pub struct GameLog(pub Vec<String>);

impl GameLog {
    pub fn say(&mut self, msg: impl Into<String>) {
        self.0.push(msg.into());
    }
}

/// "12 300,-" – an amount as the game writes it.
pub fn money(x: i64) -> String {
    let digits = x.unsigned_abs().to_string();
    let mut out = String::new();
    for (k, ch) in digits.chars().enumerate() {
        if k > 0 && (digits.len() - k).is_multiple_of(3) {
            out.push('\u{a0}');
        }
        out.push(ch);
    }
    format!("{}{out},-", if x < 0 { "-" } else { "" })
}

// ------------------------------------------------------- presentation events

/// `player` threw these dice, one after another (a six means throwing again).
#[derive(Event, Clone, Debug)]
pub struct DiceRolled {
    pub player: Entity,
    pub rolls: Vec<u32>,
}

/// A pawn went along `path`, ending on its last square.
#[derive(Event, Clone, Debug)]
pub struct PawnMoved {
    pub player: Entity,
    pub path: Vec<Entity>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Party {
    Bank,
    Player(Entity),
    /// The fines piled up on Parkoviště (house rule, see `Pot`).
    Parking,
}

/// House rule: fines go onto Parkoviště instead of to the bank, and whoever
/// lands there takes them. The resource exists only when the rule is played.
#[derive(Resource, Default, Debug)]
pub struct Pot(pub i64);

#[derive(Event, Clone, Copy, Debug)]
pub struct MoneyMoved {
    pub from: Party,
    pub to: Party,
    pub amount: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deck {
    Finance,
    Nahoda,
}

#[derive(Event, Clone, Copy, Debug)]
pub struct CardDrawn {
    pub deck: Deck,
    pub text: &'static str,
}

/// All money moves through here so that every payment is announced.
#[derive(SystemParam)]
pub struct Ledger<'w, 's> {
    money: Query<'w, 's, &'static mut Money>,
    pot: Option<ResMut<'w, Pot>>,
    commands: Commands<'w, 's>,
}

impl<'w, 's> Ledger<'w, 's> {
    pub fn balance(&self, p: Entity) -> i64 {
        self.money.get(p).map_or(0, |m| m.0)
    }

    /// Commands queued in order with the payments, for systems whose effects
    /// must be announced in the order they happen (a card before its money).
    pub fn commands(&mut self) -> &mut Commands<'w, 's> {
        &mut self.commands
    }

    /// Fines go to the bank, or onto Parkoviště under the house rule.
    pub fn fine(&mut self, p: Entity, amount: i64) {
        let to = if self.pot.is_some() { Party::Parking } else { Party::Bank };
        self.pay(Party::Player(p), to, amount);
    }

    /// Hands the fines on Parkoviště to `p`; returns how much that was.
    pub fn collect_pot(&mut self, p: Entity) -> i64 {
        let amount = self.pot.as_ref().map_or(0, |pot| pot.0);
        self.pay(Party::Parking, Party::Player(p), amount);
        amount
    }

    pub fn pay(&mut self, from: Party, to: Party, amount: i64) {
        if amount == 0 {
            return;
        }
        for (party, delta) in [(from, -amount), (to, amount)] {
            match party {
                Party::Bank => {}
                Party::Player(p) => {
                    if let Ok(mut m) = self.money.get_mut(p) {
                        m.0 += delta;
                    }
                }
                Party::Parking => {
                    if let Some(pot) = &mut self.pot {
                        pot.0 += delta;
                    }
                }
            }
        }
        self.commands.trigger(MoneyMoved { from, to, amount });
    }
}

// ----------------------------------------------------------------- new game

pub struct PlayerSpec {
    pub name: String,
    pub color: Color,
    pub ai: bool,
}

/// Starts a fresh game, replacing any running one.
#[derive(Event)]
pub struct NewGame {
    pub players: Vec<PlayerSpec>,
    pub seed: u64,
    pub house_rules: HouseRules,
}

/// Optional rules, chosen before the game.
#[derive(Clone, Copy, Debug, Default)]
pub struct HouseRules {
    /// Fines go onto Parkoviště (`Pot`).
    pub parking_pot: bool,
    /// The free square one stands on can be bought before rolling (`BuyBeforeRoll`).
    pub buy_before_roll: bool,
}

/// House rule: a player may buy the unowned square they stand on before
/// rolling – one they passed up, or one sold back to the bank meanwhile.
/// The resource exists only when the rule is played.
#[derive(Resource, Debug)]
pub struct BuyBeforeRoll;

fn on_new_game(ev: On<NewGame>, old: Query<Entity, Or<(With<Player>, With<Square>, With<CancelCard>, With<TradeOffer>)>>, mut commands: Commands) {
    for e in &old {
        commands.entity(e).despawn();
    }
    let start = board::spawn(&mut commands);
    commands.spawn(CancelCard);
    for (seat, spec) in ev.players.iter().enumerate() {
        let p = commands
            .spawn((Player { name: spec.name.clone(), color: spec.color, seat }, Money(START_MONEY), OnSquare(start), Status::Free))
            .id();
        if spec.ai {
            commands.entity(p).insert(Ai);
        }
        if seat == 0 {
            commands.insert_resource(Turn { player: p });
        }
    }
    let mut rng = fastrand::Rng::with_seed(ev.seed);
    let mut finance: Vec<usize> = (0..FINANCE.len()).collect();
    let mut nahoda: Vec<usize> = (0..NAHODA.len()).collect();
    rng.shuffle(&mut finance);
    rng.shuffle(&mut nahoda);
    commands.insert_resource(Decks { finance: finance.into(), nahoda: nahoda.into() });
    commands.insert_resource(GameRng(rng));
    commands.insert_resource(LastRoll::default());
    if ev.house_rules.parking_pot {
        commands.insert_resource(Pot::default());
    } else {
        commands.remove_resource::<Pot>();
    }
    if ev.house_rules.buy_before_roll {
        commands.insert_resource(BuyBeforeRoll);
    } else {
        commands.remove_resource::<BuyBeforeRoll>();
    }
    commands.insert_resource(Eliminated::default());
    commands.insert_resource(Phase::PreRoll);
    let first = ev.players.first().map_or(String::new(), |p| p.name.clone());
    commands.insert_resource(GameLog(vec![format!("Hra začíná! Na tahu je {first}.")]));
}

// -------------------------------------------------------------------- tests

#[cfg(test)]
pub(crate) mod test_util {
    use super::*;

    pub fn app(players: usize) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, RulesPlugin));
        let players = (0..players).map(|i| PlayerSpec { name: format!("P{i}"), color: Color::BLACK, ai: false }).collect();
        app.world_mut().trigger(NewGame { players, seed: 7, house_rules: HouseRules::default() });
        app.world_mut().flush();
        app
    }

    pub fn trigger<'a>(app: &mut App, ev: impl Event<Trigger<'a>: Default>) {
        app.world_mut().trigger(ev);
        app.world_mut().flush();
    }

    pub fn player(app: &mut App, seat: usize) -> Entity {
        let mut q = app.world_mut().query::<(Entity, &Player)>();
        q.iter(app.world()).find(|(_, p)| p.seat == seat).expect("player exists").0
    }

    /// A square by name, or the *Zrušen distanc* card.
    pub fn square(app: &mut App, name: &str) -> Entity {
        if name == "Zrušen distanc" {
            return app.world_mut().query_filtered::<Entity, With<CancelCard>>().single(app.world()).unwrap();
        }
        let mut q = app.world_mut().query::<(Entity, &Square)>();
        q.iter(app.world()).find(|(_, s)| s.0.name() == name).expect("square exists").0
    }

    pub fn give(app: &mut App, seat: usize, name: &str, races: Races) {
        let p = player(app, seat);
        let sq = square(app, name);
        app.world_mut().entity_mut(sq).insert(OwnedBy(p));
        if app.world().get::<Races>(sq).is_some() {
            app.world_mut().entity_mut(sq).insert(races);
        }
    }

    pub fn balance(app: &mut App, seat: usize) -> i64 {
        let p = player(app, seat);
        app.world().get::<Money>(p).unwrap().0
    }

    pub fn set<C: Component>(app: &mut App, seat: usize, c: C) {
        let p = player(app, seat);
        app.world_mut().entity_mut(p).insert(c);
    }

    pub fn owner(app: &mut App, name: &str) -> Option<Entity> {
        let sq = square(app, name);
        app.world().get::<OwnedBy>(sq).map(|o| o.0)
    }

    pub fn races(app: &mut App, name: &str) -> Races {
        let sq = square(app, name);
        *app.world().get::<Races>(sq).unwrap()
    }

    pub fn phase(app: &App) -> Phase {
        app.world().resource::<Phase>().clone()
    }

    pub fn set_phase(app: &mut App, phase: Phase) {
        app.world_mut().insert_resource(phase);
    }

    pub fn after_move(app: &mut App) {
        set_phase(app, Phase::AfterMove { offer: None });
    }

    pub fn place(app: &mut App, seat: usize, name: &str) {
        let sq = square(app, name);
        set(app, seat, OnSquare(sq));
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::*;
    use super::*;

    #[test]
    fn new_game_spawns_everything() {
        let mut app = app(3);
        assert_eq!(app.world_mut().query::<&Player>().iter(app.world()).count(), 3);
        assert_eq!(app.world_mut().query::<&Square>().iter(app.world()).count(), BOARD.len());
        assert_eq!(app.world_mut().query::<&Races>().iter(app.world()).count(), 22);
        assert_eq!(balance(&mut app, 2), START_MONEY);
    }

    #[test]
    fn ownership_relationship_tracks_both_sides() {
        let mut app = app(2);
        give(&mut app, 1, "Fantome", Races::None);
        give(&mut app, 1, "Gavora", Races::None);
        let p = player(&mut app, 1);
        assert_eq!(app.world().get::<Owns>(p).unwrap().len(), 2);
    }
}
