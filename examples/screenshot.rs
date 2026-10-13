#![allow(clippy::type_complexity, clippy::too_many_arguments)]

//! Dev tool: plays a few turns automatically and saves a screenshot.
//! `cargo run --example screenshot -- out.png [setup|game|mid|rich] [human turns] [delay]`
//! (`mid` captures `delay` seconds after the last roll, while animations play;
//! `rich` checks the worst layout: six players owning everything, a trade
//! being put together and standing offers all around. Fails if anything is cut off.)

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::PresentMode;
use dostihy::{Busy, ai::AiPlugin, data::PAWNS, rules::*, ui::{AppState, CutOff, DismissCard, OpenTrade, UiPlugin}, window_plugins};
use std::time::Duration;

#[derive(Resource)]
struct Plan {
    out: String,
    setup_only: bool,
    mid: bool,
    rich: bool,
    delay: f32,
    human_turns: usize,
    idle_frames: u32,
}

fn main() -> AppExit {
    let args: Vec<String> = std::env::args().collect();
    let plan = Plan {
        out: args.get(1).cloned().unwrap_or("shot.png".into()),
        setup_only: args.get(2).is_some_and(|m| m == "setup"),
        mid: args.get(2).is_some_and(|m| m == "mid"),
        rich: args.get(2).is_some_and(|m| m == "rich"),
        delay: args.get(4).and_then(|n| n.parse().ok()).unwrap_or(0.6),
        human_turns: args.get(3).and_then(|n| n.parse().ok()).unwrap_or(4),
        idle_frames: 0,
    };
    // Without vsync it keeps rendering even when the window is hidden.
    let window = Window { title: "screenshot".into(), resolution: (1400, 880).into(), present_mode: PresentMode::AutoNoVsync, ..default() };
    App::new()
        .add_plugins((window_plugins().set(WindowPlugin { primary_window: Some(window), ..default() }), RulesPlugin, AiPlugin { delay: Duration::from_millis(50) }, UiPlugin))
        .insert_resource(plan)
        .add_systems(Update, drive)
        .run()
}

fn drive(
    mut plan: ResMut<Plan>,
    mut frame: Local<u32>,
    time: Res<Time<Real>>,
    mut ready_since: Local<Option<f32>>,
    state: Res<State<AppState>>,
    mut next: ResMut<NextState<AppState>>,
    busy: Res<Busy>,
    phase: Option<Res<Phase>>,
    turn: Option<Res<Turn>>,
    players: Query<(&Money, Has<Ai>)>,
    seats: Query<(Entity, &Player)>,
    squares: Query<(Entity, &Square, Option<&OwnedBy>)>,
    cut_off: Res<CutOff>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    // Counted from the new-game screen on, after the art has loaded.
    if *state.get() == AppState::Loading {
        return;
    }
    *frame += 1;
    // Reads the drawn cards quickly.
    if busy.0 && state.get() == &AppState::Playing {
        commands.trigger(DismissCard);
    }
    if *frame == 60 && !plan.setup_only {
        let players = PAWNS[..if plan.rich { 6 } else { 4 }].iter().enumerate().map(|(k, &(name, color))| PlayerSpec { name: name.into(), color, ai: k > 0 }).collect();
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64;
        println!("seed {seed}");
        commands.trigger(NewGame { players, seed, house_rules: HouseRules { parking_pot: true, buy_before_roll: true } });
        next.set(AppState::Playing);
        return;
    }
    if *frame == 61 && plan.rich {
        // Whole stables in turn to the players, with ever more races.
        let mut seats: Vec<_> = seats.iter().map(|(e, p)| (p.seat, e)).collect();
        seats.sort();
        let mut stables = Vec::new();
        for (sq, square, _) in &squares {
            let Some(stable) = square.0.stable() else { continue };
            if !stables.contains(&stable) {
                stables.push(stable);
            }
            let k = stables.iter().position(|&s| s == stable).unwrap();
            let races = Races::ALL[k % Races::ALL.len()];
            commands.entity(sq).insert((OwnedBy(seats[k % seats.len()].1), races));
        }
        // Everything else to the first two players, for long hands.
        for (n, (sq, ..)) in squares.iter().filter(|s| s.1.0.stable().is_none() && s.1.0.price().is_some()).enumerate() {
            commands.entity(sq).insert(OwnedBy(seats[n % 2].1));
        }
    }
    if *frame == 63 && plan.rich {
        let mut seats: Vec<_> = seats.iter().map(|(e, p)| (p.seat, e)).collect();
        seats.sort();
        let seat = |k: usize| seats[k].1;
        let of = |p: Entity| squares.iter().filter(|s| s.2.is_some_and(|o| o.0 == p)).map(|s| s.0).collect::<Vec<_>>();
        let (me, mine) = (seat(0), of(seat(0)));
        // Every computer wants one of the person's horses; the person has the most offers out.
        for k in 1..seats.len() {
            let t = Trade { partner: me, give: Vec::new(), take: vec![mine[k % mine.len()]], money: 12_300 };
            commands.spawn((TradeOffer(t), OfferedBy(seat(k))));
        }
        // Sold without races, so they trade on their own: trainers and services.
        let loose: Vec<Entity> = mine.iter().copied().filter(|&sq| squares.get(sq).is_ok_and(|s| s.1.0.stable().is_none())).collect();
        for k in 1..=MAX_TRADE_OFFERS {
            let t = Trade { partner: seat(k), give: vec![loose[k - 1]], take: Vec::new(), money: -4_500 };
            commands.spawn((TradeOffer(t), OfferedBy(me)));
        }
        // And the biggest deal being put together.
        commands.insert_resource(Phase::AfterMove { offer: None });
        commands.trigger(OpenTrade(Trade { partner: seat(1), give: mine, take: of(seat(1)), money: 12_300 }));
    }
    let ready = match (state.get(), phase, turn) {
        // The worst layouts are set up by hand; nobody plays.
        (AppState::Playing, ..) if plan.rich => *frame > 70 && !busy.0,
        (AppState::Setup, ..) => plan.setup_only && *frame > 60,
        (AppState::Playing, Some(_), Some(_)) if plan.mid && plan.human_turns == 0 => true,
        (AppState::Playing, Some(phase), Some(turn)) if !busy.0 => {
            let Ok((money, ai)) = players.get(turn.player) else { return };
            if ai {
                false
            } else if plan.human_turns == 0 {
                true
            } else {
                match &*phase {
                    Phase::PreRoll if plan.mid && plan.human_turns == 1 => {
                        commands.trigger(Roll);
                        plan.human_turns = 0;
                    }
                    Phase::PreRoll => commands.trigger(Roll),
                    Phase::AfterMove { offer: Some(_), .. } => commands.trigger(Buy),
                    Phase::AfterMove { .. } if money.0 < 0 => commands.trigger(Liquidate(turn.player)),
                    Phase::AfterMove { .. } => {
                        commands.trigger(EndTurn);
                        plan.human_turns -= 1;
                    }
                    Phase::GameOver { .. } => plan.human_turns = 0,
                }
                false
            }
        }
        _ => false,
    };
    if ready {
        let since = *ready_since.get_or_insert(time.elapsed_secs());
        let wait = if plan.mid { plan.delay } else { 0.5 };
        let elapsed = time.elapsed_secs() - since;
        if elapsed >= wait && plan.idle_frames == 0 {
            plan.idle_frames = 1;
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(plan.out.clone()));
        }
        if elapsed >= wait + 1.0 {
            for problem in &cut_off.0 {
                eprintln!("CUT OFF: {problem}");
            }
            exit.write(if cut_off.0.is_empty() { AppExit::Success } else { AppExit::error() });
        }
    }
}
