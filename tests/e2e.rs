//! End-to-end: computer players play whole games through the real Bevy schedule,
//! with the invariants the type system can't express checked after every frame.
//! (Structural ones – one player on turn, pawns on the board, bankrupt players
//! owning nothing – hold by construction.)

use bevy::prelude::*;
use dostihy::{ai::AiPlugin, data::*, rules::*};
use std::time::Duration;

fn new_app(players: usize, seed: u64) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, RulesPlugin, AiPlugin { delay: Duration::ZERO }));
    let players = PAWNS[..players].iter().map(|&(name, color)| PlayerSpec { name: name.into(), color, ai: true }).collect();
    // Every other game with the Parkoviště house rule.
    app.world_mut().trigger(NewGame { players, seed, house_rules: HouseRules { parking_pot: seed.is_multiple_of(2), buy_before_roll: seed.is_multiple_of(3) } });
    app.update();
    app
}

fn phase(app: &App) -> Phase {
    app.world().resource::<Phase>().clone()
}

fn check_invariants(app: &mut App, ctx: &str) {
    let turn = app.world().resource::<Turn>().player;
    let mut players = app.world_mut().query::<(Entity, &Money)>();
    for (p, money) in players.iter(app.world()) {
        assert!(money.0 >= 0 || p == turn, "{ctx}: only the player on turn may be in debt");
    }
    let mut squares = app.world_mut().query::<(&Square, Option<&OwnedBy>, Option<&Races>)>();
    let squares: Vec<_> = squares.iter(app.world()).map(|(s, o, r)| (s.0, o.map(|o| o.0), r.copied().unwrap_or_default())).collect();
    for &(field, owner, races) in &squares {
        if races > Races::None {
            let stable = field.stable().expect("races only on horses");
            let stable_owners = squares.iter().filter(|s| s.0.stable() == Some(stable)).map(|s| s.1);
            assert!(stable_owners.clone().all(|o| o == owner && o.is_some()), "{ctx}: races only in a complete stable");
        }
    }
}

#[test]
fn computer_games_play_to_a_single_winner() {
    let (mut finished, mut trades) = (0, 0);
    for seed in 0..8 {
        let mut app = new_app(3, seed);
        for frame in 0..30_000 {
            app.update();
            check_invariants(&mut app, &format!("seed {seed}, frame {frame}"));
            if let Phase::GameOver { winner } = phase(&app) {
                let players: Vec<Entity> = app.world_mut().query_filtered::<Entity, With<Player>>().iter(app.world()).collect();
                assert_eq!(players, [winner.unwrap()], "seed {seed}: the winner is the last one standing");
                assert_eq!(app.world().resource::<Eliminated>().0.len(), 2);
                finished += 1;
                trades += app.world().resource::<GameLog>().0.iter().filter(|l| l.contains("přijímá nabídku")).count();
                break;
            }
        }
    }
    assert!(finished >= 6, "only {finished}/8 games ended by bankruptcy");
    assert!(trades > 0, "computers buy missing horses from each other");
}

#[test]
fn time_limit_picks_the_richest_player() {
    let mut app = new_app(4, 99);
    for frame in 0..400 {
        app.update();
        check_invariants(&mut app, &format!("frame {frame}"));
    }
    app.world_mut().trigger(FinishGame);
    app.world_mut().flush();
    let Phase::GameOver { winner: Some(winner) } = phase(&app) else { panic!("no winner") };

    let mut squares = app.world_mut().query::<(&Square, &OwnedBy, Option<&Races>)>();
    let holdings: Vec<(Entity, i64)> = squares
        .iter(app.world())
        .map(|(s, o, r)| (o.0, s.0.price().unwrap() + s.0.stable().map_or(0, |st| st.race_cost()) * r.map_or(0, |r| r.level() as i64)))
        .collect();
    let mut players = app.world_mut().query::<(Entity, &Money)>();
    let worth = |p: Entity, money: i64| money + holdings.iter().filter(|h| h.0 == p).map(|h| h.1).sum::<i64>();
    let best = players.iter(app.world()).map(|(p, m)| (worth(p, m.0), p)).max().unwrap();
    assert_eq!(best.1, winner);
}

/// A computer's offer for a person's horse never holds up the game: it stands,
/// rising each turn up to 3× the price, until the person takes it.
#[test]
fn computer_offers_stand_without_blocking() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, RulesPlugin, AiPlugin { delay: Duration::ZERO }));
    let players = PAWNS[..2].iter().enumerate().map(|(k, &(name, color))| PlayerSpec { name: name.into(), color, ai: k == 0 }).collect();
    app.world_mut().trigger(NewGame { players, seed: 5, house_rules: HouseRules::default() });
    app.update();
    let mut squares = app.world_mut().query::<(Entity, &Square)>();
    let mut named = |app: &App, name: &str| squares.iter(app.world()).find(|s| s.1.0.name() == name).unwrap().0;
    let (fantome, gavora) = (named(&app, "Fantome"), named(&app, "Gavora"));
    let mut people = app.world_mut().query::<(Entity, &Player)>();
    let mut seat = |app: &App, k: usize| people.iter(app.world()).find(|p| p.1.seat == k).unwrap().0;
    let (computer, person) = (seat(&app, 0), seat(&app, 1));
    app.world_mut().entity_mut(fantome).insert(OwnedBy(computer));
    app.world_mut().entity_mut(gavora).insert(OwnedBy(person));

    // The person just plays on, ignoring the offer.
    let mut person_turns = 0;
    for _ in 0..400 {
        app.update();
        if app.world().resource::<Turn>().player == person {
            match phase(&app) {
                Phase::PreRoll => app.world_mut().trigger(Roll),
                Phase::AfterMove { .. } => {
                    person_turns += 1;
                    app.world_mut().trigger(EndTurn)
                }
                Phase::GameOver { .. } => break,
            }
            app.world_mut().flush();
        }
    }
    assert!(person_turns > 10, "the game went on");
    let mut offers = app.world_mut().query::<(Entity, &TradeOffer, &OfferedBy)>();
    let standing: Vec<_> = offers.iter(app.world()).map(|(e, t, by)| (e, t.0.clone(), by.0)).collect();
    let log = &app.world().resource::<GameLog>().0;
    let trades: Vec<_> = log.iter().filter(|l| l.contains("abíd")).collect();
    let [(offer, trade, by)] = &standing[..] else { panic!("one standing offer, got {standing:?}; {trades:#?}") };
    assert_eq!((*by, trade.partner, &trade.take, trade.money), (computer, person, &vec![gavora], 3600), "risen to 3× Gavora's price");

    let before = app.world().get::<Money>(person).unwrap().0;
    app.world_mut().trigger(AcceptTrade(*offer));
    app.world_mut().flush();
    assert_eq!(app.world().get::<OwnedBy>(gavora).unwrap().0, computer);
    assert_eq!(app.world().get::<Money>(person).unwrap().0, before + 3600);
}
