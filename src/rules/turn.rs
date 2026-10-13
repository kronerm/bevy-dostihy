//! Rolling, moving, landing on squares and passing the turn.

use super::*;
use super::cards::DrawCard;

pub(super) fn plugin(app: &mut App) {
    app.add_observer(on_roll)
        .add_observer(on_use_cancel_card)
        .add_observer(on_travel)
        .add_observer(on_landed)
        .add_observer(on_end_turn)
        .add_observer(on_next_turn)
        .add_observer(on_finish_game);
}

#[derive(Event)]
pub struct Roll;

#[derive(Event)]
pub struct UseCancelCard;

#[derive(Event)]
pub struct EndTurn;

/// Ends the game by time limit: the richest player wins.
#[derive(Event)]
pub struct FinishGame;

/// Moves the player on turn along `path`, then resolves the square reached.
#[derive(Event)]
pub(super) struct Travel {
    pub path: Vec<Entity>,
    pub pay: StartPay,
}

/// What crossing or reaching Start pays on a move.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum StartPay {
    Nothing,
    Bonus,
    /// A dice move: the bonus, and more for stopping right on Start.
    DiceBonus,
}

/// The player on turn stopped on a square.
#[derive(Event)]
pub(super) struct Landed;

#[derive(Event)]
pub(super) struct NextTurn {
    /// Seat of the player whose turn just ended.
    pub after_seat: usize,
}

enum Throw {
    Move(u32),
    StayInDistanc,
    ThreeSixes,
}

/// A 6 earns another roll; three sixes in a row send the pawn to Distanc.
/// From Distanc the player has three tries for a 6, which frees the pawn for a normal roll.
fn throw(status: &mut Status, rng: &mut fastrand::Rng, rolls: &mut Vec<u32>) -> Throw {
    let mut d6 = || {
        let r = rng.u32(1..=6);
        rolls.push(r);
        r
    };
    if *status == Status::Distanc {
        if !(0..DISTANC_TRIES).any(|_| d6() == 6) {
            return Throw::StayInDistanc;
        }
        *status = Status::Free;
    }
    let mut total = 0;
    for _ in 0..3 {
        let r = d6();
        total += r;
        if r != 6 {
            return Throw::Move(total);
        }
    }
    Throw::ThreeSixes
}

fn on_roll(
    _: On<Roll>,
    turn: Res<Turn>,
    mut phase: ResMut<Phase>,
    mut players: Query<(&Player, &OnSquare, &mut Status)>,
    track: Track,
    mut rng: ResMut<GameRng>,
    mut last_roll: ResMut<LastRoll>,
    mut log: ResMut<GameLog>,
    mut commands: Commands,
) {
    if *phase != Phase::PreRoll {
        return;
    }
    *phase = Phase::AfterMove { offer: None };
    let (player, at, mut status) = players.get_mut(turn.player).expect("player on turn");
    let mut rolls = Vec::new();
    let result = throw(&mut status, &mut rng.0, &mut rolls);
    let shown = rolls.iter().map(u32::to_string).collect::<Vec<_>>().join(", ");
    commands.trigger(DiceRolled { player: turn.player, rolls });
    match result {
        Throw::Move(n) => {
            log.say(format!("{} hodil {shown}.", player.name));
            last_roll.0 = n as i64;
            let path = track.walk(at.0, Direction::Forward, Destination::Steps(n as usize));
            commands.trigger(Travel { path, pay: StartPay::DiceBonus });
        }
        Throw::StayInDistanc => log.say(format!("{} hodil {shown} a zůstává na distancu.", player.name)),
        Throw::ThreeSixes => {
            log.say(format!("{} hodil {shown} – tři šestky, jde na distanc!", player.name));
            let path = track.walk(at.0, Direction::Forward, Destination::Nearest(Landmark::Distanc));
            commands.trigger(Travel { path: vec![*path.last().unwrap()], pay: StartPay::Nothing });
        }
    }
    commands.trigger(SettleDebts);
}

fn on_use_cancel_card(
    _: On<UseCancelCard>,
    turn: Res<Turn>,
    phase: Res<Phase>,
    mut players: Query<(&Player, &mut Status)>,
    deeds: Deeds,
    mut log: ResMut<GameLog>,
    mut commands: Commands,
) {
    let (player, mut status) = players.get_mut(turn.player).expect("player on turn");
    if *phase != Phase::PreRoll || *status != Status::Distanc || !deeds.holds_cancel_card(turn.player) {
        return;
    }
    *status = Status::Free;
    commands.entity(deeds.cancel_card()).remove::<OwnedBy>();
    log.say(format!("{} používá kartu Zrušen distanc.", player.name));
}

fn on_travel(
    ev: On<Travel>,
    turn: Res<Turn>,
    names: Query<&Player>,
    track: Track,
    mut ledger: Ledger,
    mut log: ResMut<GameLog>,
) {
    let Some(&to) = ev.path.last() else { return };
    let name = &names.get(turn.player).expect("player on turn").name;
    // One queue, so the walk shows before the bonus and the bonus before what the square costs.
    ledger.commands().entity(turn.player).insert(OnSquare(to));
    ledger.commands().trigger(PawnMoved { player: turn.player, path: ev.path.clone() });
    if ev.pay != StartPay::Nothing && ev.path.contains(&track.start()) {
        ledger.pay(Party::Bank, Party::Player(turn.player), START_BONUS);
        log.say(format!("{name} prošel startem a dostává {}", money(START_BONUS)));
        if ev.pay == StartPay::DiceBonus && to == track.start() {
            ledger.pay(Party::Bank, Party::Player(turn.player), START_LANDING_BONUS);
            log.say(format!("{name} zastavil přímo na startu a dostává navíc {}", money(START_LANDING_BONUS)));
        }
    }
    ledger.commands().trigger(Landed);
}

fn on_landed(
    _: On<Landed>,
    turn: Res<Turn>,
    mut players: Query<(&Player, &OnSquare, &mut Status)>,
    deeds: Deeds,
    mut ledger: Ledger,
    last_roll: Res<LastRoll>,
    mut phase: ResMut<Phase>,
    mut log: ResMut<GameLog>,
    mut commands: Commands,
) {
    let me = turn.player;
    let (player, at, _) = players.get(me).expect("player on turn");
    let (name, sq) = (player.name.clone(), at.0);
    let field = *deeds.field(sq);
    phase.set_offer(None);
    match field {
        Field::Start => {}
        Field::Parking => {
            let pot = ledger.collect_pot(me);
            if pot > 0 {
                log.say(format!("{name} bere z parkoviště {}", money(pot)));
            }
        }
        Field::Horse(_) | Field::Trainer(_) | Field::Service(_) => match deeds.owner(sq) {
            None => phase.set_offer(Some(sq)),
            Some(o) if o == me => {}
            Some(o) => {
                let (owner, _, owner_status) = players.get(o).expect("owners are players");
                let fee = deeds.fee(sq, o, *owner_status, last_roll.0);
                log.say(format!("{name} platí {} {} ({}).", owner.name, money(fee), field.name()));
                ledger.pay(Party::Player(me), Party::Player(o), fee);
            }
        },
        Field::Finance => commands.trigger(DrawCard(Deck::Finance)),
        Field::Nahoda => commands.trigger(DrawCard(Deck::Nahoda)),
        Field::Vet(fee) => {
            ledger.fine(me, fee);
            log.say(format!("{name} platí veterináři {}", money(fee)));
        }
        Field::Distanc => {
            *players.get_mut(me).unwrap().2 = Status::Distanc;
            log.say(format!("{name} je na distancu."));
        }
        Field::Doping => {
            *players.get_mut(me).unwrap().2 = Status::Sitting { turns: 1, doping: true };
            log.say(format!("{name} je podezřelý z dopingu – 1× nehází."));
        }
    }
}

fn on_end_turn(_: On<EndTurn>, turn: Res<Turn>, phase: Res<Phase>, players: Query<(&Player, &Money)>, mut commands: Commands) {
    let (player, money) = players.get(turn.player).expect("player on turn");
    if phase.can_end_turn(money.0) {
        commands.trigger(NextTurn { after_seat: player.seat });
    }
}

fn on_next_turn(
    ev: On<NextTurn>,
    raced: Query<Entity, With<RacedThisTurn>>,
    mut commands: Commands,
    mut players: Query<(Entity, &Player, &mut Status)>,
    mut turn: ResMut<Turn>,
    mut phase: ResMut<Phase>,
    mut log: ResMut<GameLog>,
) {
    let mut order: Vec<(usize, Entity)> = players.iter().map(|(e, p, _)| (p.seat, e)).collect();
    order.sort();
    let Some(&(_, next)) = order.iter().find(|(seat, _)| *seat > ev.after_seat).or(order.first()) else { return };
    turn.player = next;
    for horse in &raced {
        commands.entity(horse).remove::<RacedThisTurn>();
    }
    let (_, player, mut status) = players.get_mut(next).unwrap();
    if let Status::Sitting { turns, doping } = *status {
        *status = if turns > 1 { Status::Sitting { turns: turns - 1, doping } } else { Status::Free };
        *phase = Phase::AfterMove { offer: None };
        log.say(format!("{} tento tah vynechává.", player.name));
    } else {
        *phase = Phase::PreRoll;
        log.say(format!("Na tahu je {}.", player.name));
    }
}

fn on_finish_game(
    _: On<FinishGame>,
    players: Query<(Entity, &Player, &Money)>,
    deeds: Deeds,
    mut phase: ResMut<Phase>,
    mut log: ResMut<GameLog>,
) {
    let winner = players.iter().max_by_key(|(e, _, m)| deeds.net_worth(*e, m.0));
    *phase = Phase::GameOver { winner: winner.map(|w| w.0) };
    if let Some((e, p, m)) = winner {
        log.say(format!("Konec hry. Vítězem je {} s majetkem {}", p.name, money(deeds.net_worth(e, m.0))));
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_util::*;
    use super::*;

    fn land(app: &mut App, seat: usize, name: &str) {
        place(app, seat, name);
        trigger(app, Landed);
    }

    #[test]
    fn throw_rules() {
        let mut rolls = Vec::new();
        for seed in 0..500 {
            let mut rng = fastrand::Rng::with_seed(seed);
            let mut status = Status::Free;
            rolls.clear();
            match throw(&mut status, &mut rng, &mut rolls) {
                Throw::Move(n) => {
                    assert_eq!(n, rolls.iter().sum::<u32>());
                    assert!(rolls.len() <= 3 && rolls[..rolls.len() - 1].iter().all(|&r| r == 6) && rolls[rolls.len() - 1] != 6);
                }
                Throw::ThreeSixes => assert_eq!(rolls, [6, 6, 6]),
                Throw::StayInDistanc => unreachable!(),
            }
            let mut status = Status::Distanc;
            rolls.clear();
            match throw(&mut status, &mut rng, &mut rolls) {
                Throw::StayInDistanc => assert!(rolls.len() == 3 && !rolls.contains(&6) && status == Status::Distanc),
                Throw::Move(_) | Throw::ThreeSixes => {
                    let escape = rolls.iter().position(|&r| r == 6).unwrap();
                    assert!(escape < 3 && status == Status::Free);
                }
            }
        }
    }

    #[test]
    fn fines_pile_up_on_parking_under_house_rule() {
        let mut app = app(2);
        app.world_mut().insert_resource(Pot::default());
        after_move(&mut app);
        land(&mut app, 0, "Veterinární vyšetření");
        assert_eq!(balance(&mut app, 0), START_MONEY - 1000);
        assert_eq!(app.world().resource::<Pot>().0, 1000);
        land(&mut app, 0, "Parkoviště");
        assert_eq!(balance(&mut app, 0), START_MONEY);
        assert_eq!(app.world().resource::<Pot>().0, 0);
    }

    #[test]
    fn passing_start_pays_bonus() {
        let mut app = app(2);
        after_move(&mut app);
        let path = vec![square(&mut app, "Start"), square(&mut app, "Fantome")];
        trigger(&mut app, Travel { path, pay: StartPay::DiceBonus });
        assert_eq!(balance(&mut app, 0), START_MONEY + START_BONUS);
        assert_eq!(phase(&app).offer(), Some(square(&mut app, "Fantome")));
    }

    #[test]
    fn stopping_on_start_pays_extra() {
        let mut app = app(2);
        after_move(&mut app);
        let path = vec![square(&mut app, "Napoli"), square(&mut app, "Start")];
        trigger(&mut app, Travel { path: path.clone(), pay: StartPay::DiceBonus });
        assert_eq!(balance(&mut app, 0), START_MONEY + START_BONUS + START_LANDING_BONUS);
        trigger(&mut app, Travel { path, pay: StartPay::Bonus });
        assert_eq!(balance(&mut app, 0), START_MONEY + 2 * START_BONUS + START_LANDING_BONUS, "cards pay only the bonus");
    }

    #[test]
    fn landing_on_opponent_horse_pays_race_fee() {
        let mut app = app(2);
        after_move(&mut app);
        give(&mut app, 1, "Napoli", Races::Two);
        land(&mut app, 0, "Napoli");
        assert_eq!(balance(&mut app, 0), START_MONEY - 12000);
        assert_eq!(balance(&mut app, 1), START_MONEY + 12000);
    }

    #[test]
    fn suspended_owner_gets_only_visit_fee() {
        let mut app = app(2);
        after_move(&mut app);
        give(&mut app, 1, "Napoli", Races::Main);
        set(&mut app, 1, Status::Distanc);
        land(&mut app, 0, "Napoli");
        assert_eq!(balance(&mut app, 1), START_MONEY + 1000);
    }

    #[test]
    fn services_and_trainers() {
        let mut app = app(2);
        after_move(&mut app);
        app.world_mut().resource_mut::<LastRoll>().0 = 5;
        give(&mut app, 1, "Přeprava", Races::None);
        land(&mut app, 0, "Přeprava");
        assert_eq!(balance(&mut app, 1), START_MONEY + 400);
        give(&mut app, 1, "Stáje", Races::None);
        land(&mut app, 0, "Přeprava");
        assert_eq!(balance(&mut app, 1), START_MONEY + 1400);
        give(&mut app, 1, "1. Trenér", Races::None);
        give(&mut app, 1, "2. Trenér", Races::None);
        land(&mut app, 0, "1. Trenér");
        assert_eq!(balance(&mut app, 1), START_MONEY + 3400);
    }

    #[test]
    fn doping_sits_out_one_turn() {
        let mut app = app(2);
        after_move(&mut app);
        land(&mut app, 0, "Podezření z dopingu");
        let p0 = player(&mut app, 0);
        assert_eq!(*app.world().get::<Status>(p0).unwrap(), Status::Sitting { turns: 1, doping: true });
        trigger(&mut app, EndTurn);
        after_move(&mut app);
        trigger(&mut app, EndTurn);
        assert_eq!(app.world().resource::<Turn>().player, p0);
        assert!(matches!(phase(&app), Phase::AfterMove { .. }), "the skipped turn has no roll");
        assert_eq!(*app.world().get::<Status>(p0).unwrap(), Status::Free);
    }

    #[test]
    fn rolling_moves_and_ends_turn() {
        let mut app = app(3);
        trigger(&mut app, Roll);
        let p0 = player(&mut app, 0);
        let start = square(&mut app, "Start");
        assert_ne!(app.world().get::<OnSquare>(p0).unwrap().0, start);
        assert!(matches!(phase(&app), Phase::AfterMove { .. }));
        trigger(&mut app, EndTurn);
        let p1 = player(&mut app, 1);
        assert_eq!(app.world().resource::<Turn>().player, p1);
    }
}
