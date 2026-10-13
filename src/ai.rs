//! A simple computer opponent: buys when it keeps a cash reserve, races the
//! horses of its complete stables it stops on, keeps offers out for the horses
//! that would complete its stables, and answers offers with a rough value estimate.

use crate::Busy;
use crate::data::{DISTANC_TRIES, Field, Stable};
use crate::rules::*;
use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;
use std::time::Duration;

const RESERVE: i64 = 2500;

/// Computer players act at most once per `delay`, and never while the UI animates.
pub struct AiPlugin {
    pub delay: Duration,
}

impl Default for AiPlugin {
    fn default() -> Self {
        AiPlugin { delay: Duration::from_millis(500) }
    }
}

impl Plugin for AiPlugin {
    fn build(&self, app: &mut App) {
        let idle = |busy: Option<Res<Busy>>| !busy.is_some_and(|b| b.0);
        app.add_observer(|ev: On<Add, Ai>, mut commands: Commands| {
            commands.entity(ev.entity).insert(Haggling::default());
        })
        .add_systems(Update, withdraw_unwanted.run_if(resource_exists::<Phase>))
        .add_systems(Update, act.run_if(resource_exists::<Phase>.and_then(idle).and_then(on_timer(self.delay))));
    }
}

/// The wanted squares a computer has seen to this turn: each of its standing
/// offers is raised at most once a turn.
#[derive(Component, Default)]
struct Haggling(Vec<Vec<Entity>>);

fn act(
    turn: Res<Turn>,
    phase: Res<Phase>,
    players: Query<(&Money, &Status, Has<Ai>)>,
    mut haggling: Query<&mut Haggling>,
    offers: Query<(Entity, &TradeOffer, &OfferedBy)>,
    pawns: Query<(Entity, &OnSquare)>,
    before_roll: Option<Res<BuyBeforeRoll>>,
    track: Track,
    deeds: Deeds,
    mut commands: Commands,
) {
    let me = turn.player;
    if turn.is_changed()
        && let Ok(mut h) = haggling.get_mut(me)
    {
        h.0.clear();
    }
    if matches!(*phase, Phase::GameOver { .. }) {
        return;
    }
    // Offers to a computer are answered right away.
    for (offer, TradeOffer(t), by) in &offers {
        if let Ok((money, _, true)) = players.get(t.partner) {
            if accepts(t, by.0, &offers, &deeds, money.0, cancel_card_worth(t.partner, &pawns, &track, &deeds)) {
                commands.trigger(AcceptTrade(offer));
            } else {
                commands.trigger(DeclineTrade(offer));
            }
            return;
        }
    }
    let Ok((money, status, true)) = players.get(me) else { return };
    let Ok(mut haggling) = haggling.get_mut(me) else { return };
    let Ok((_, &OnSquare(at))) = pawns.get(me) else { return };
    let buys = for_sale(&phase, before_roll.is_some(), at, &deeds)
        .and_then(|sq| deeds.field(sq).price())
        .is_some_and(|price| money.0 - price >= RESERVE);
    match *phase {
        Phase::GameOver { .. } => {}
        _ if buys => commands.trigger(Buy),
        Phase::PreRoll if *status == Status::Distanc && deeds.holds_cancel_card(me) => commands.trigger(UseCancelCard),
        Phase::PreRoll => commands.trigger(Roll),
        Phase::AfterMove { .. } if money.0 < 0 => commands.trigger(Liquidate(me)),
        Phase::AfterMove { .. } => {
            let horse = race_horse(&phase, me, at, &deeds);
            if let Some(ev) = horse.and_then(|h| races_to_buy(h, money.0, &deeds)) {
                commands.trigger(ev);
            } else {
                match haggle(me, money.0, &mut haggling, &offers, &players, &deeds) {
                    Some(t) => commands.trigger(ProposeTrade(t)),
                    None => commands.trigger(EndTurn),
                }
            }
        }
    }
}

/// A computer's offer stops making sense when others change the board – say
/// its own horse of that stable got traded away. It withdraws at once, every
/// frame and even mid-animation, before anyone can accept. Offers that can't
/// be carried out at all lapse by the rules instead.
fn withdraw_unwanted(offers: Query<(Entity, &TradeOffer, &OfferedBy)>, computers: Query<(), With<Ai>>, deeds: Deeds, mut commands: Commands) {
    for (offer, TradeOffer(t), by) in &offers {
        let still_wanted = || wanted(by.0, &deeds).iter().any(|(owner, missing)| (*owner, missing) == (t.partner, &t.take));
        if computers.contains(by.0) && trade_possible(t, by.0, &deeds) && !still_wanted() {
            commands.trigger(CancelTrade(offer));
        }
    }
}

/// As many races for the horse stopped on as the reserve allows.
fn races_to_buy(horse: Entity, money: i64, deeds: &Deeds) -> Option<BuyRaces> {
    let cost = deeds.field(horse).stable()?.race_cost();
    deeds.races(horse).buyable().rev().find(|&n| money - cost * n as i64 >= RESERVE).map(BuyRaces)
}

/// Horses of a single other player that would complete one of `me`'s stables
/// – worth chasing only for stables it already has at least half of.
fn wanted(me: Entity, deeds: &Deeds) -> Vec<(Entity, Vec<Entity>)> {
    let mut stables = Vec::new();
    for stable in deeds.owned_by(me).into_iter().filter_map(|sq| deeds.stable(sq)) {
        if !stables.contains(&stable) {
            stables.push(stable);
        }
    }
    stables
        .into_iter()
        .filter_map(|stable| {
            let (mine, missing): (Vec<Entity>, Vec<Entity>) = deeds.stable_squares(stable).partition(|&sq| deeds.owner(sq) == Some(me));
            let owner = deeds.owner(*missing.first()?)?;
            (missing.len() <= mine.len() && missing.iter().all(|&sq| deeds.owner(sq) == Some(owner))).then_some((owner, missing))
        })
        .collect()
}

/// Keeps a standing offer out for the horses that would complete each of its
/// stables: another computer sells at 3× their value, a person gets an offer
/// rising each turn from 2× to 3×. It never stakes more than half the money
/// it can spare on one deal.
fn haggle(
    me: Entity,
    money: i64,
    seen: &mut Haggling,
    offers: &Query<(Entity, &TradeOffer, &OfferedBy)>,
    players: &Query<(&Money, &Status, Has<Ai>)>,
    deeds: &Deeds,
) -> Option<Trade> {
    let mine = || offers.iter().filter(move |o| o.2.0 == me).map(|(_, TradeOffer(t), _)| t);
    for (owner, missing) in wanted(me, deeds) {
        if seen.0.contains(&missing) {
            continue;
        }
        seen.0.push(missing.clone());
        let value: i64 = missing.iter().map(|&sq| deeds.value(sq)).sum();
        let current = mine().find(|t| t.take == missing).map(|t| t.money);
        let Ok((owner_money, _, computer)) = players.get(owner) else { continue };
        let asking = if computer { 3 * value } else { current.map_or(2 * value, |m| (m + value / 2).min(3 * value)) };
        let price = asking.min((money - RESERVE) / 2);
        // Another computer wouldn't sell for less; nobody sells below value.
        let worth_it = if computer { price == asking } else { price >= value };
        let t = Trade { partner: owner, give: Vec::new(), take: missing, money: price };
        if worth_it && current.is_none_or(|c| price > c) && trade_valid(&t, me, deeds, money, owner_money.0) {
            return Some(t);
        }
    }
    None
}

/// The race fees its horses would lose while it waits in Distanc – they earn
/// only the stable visit then: what the *Zrušen distanc* card saves it. Rough:
/// the other pawns' chances to stop on its horses with races in one roll, times
/// the turns a stay in Distanc lasts on average.
fn cancel_card_worth(ai: Entity, pawns: &Query<(Entity, &OnSquare)>, track: &Track, deeds: &Deeds) -> i64 {
    let ring: Vec<Entity> = track.squares().collect();
    let index = |sq: Entity| ring.iter().position(|&s| s == sq).unwrap_or(0);
    let stay = 1.0 / (1.0 - (5.0f64 / 6.0).powi(DISTANC_TRIES as i32));
    let mut lost = 0.0;
    for (_, at) in pawns.iter().filter(|(p, _)| *p != ai) {
        for horse in deeds.owned_by(ai) {
            let Some(Field::Horse(h)) = deeds.square(horse) else { continue };
            let extra = h.fees[deeds.races(horse).level()] - h.fees[0];
            let ahead = (index(horse) + ring.len() - index(at.0)) % ring.len();
            lost += extra as f64 * landing_chance(ahead);
        }
    }
    (lost * stay) as i64
}

/// The chance that one roll moves a pawn exactly `d` squares: a 6 throws
/// again, and three sixes go to Distanc.
fn landing_chance(d: usize) -> f64 {
    let sixes = d / 6;
    if d.is_multiple_of(6) || sixes > 2 { 0.0 } else { (1.0f64 / 6.0).powi(sixes as i32 + 1) }
}

/// Takes squares at their value (with a premium when they complete a stable),
/// gives its own only at a profit: twice their value, more for a horse of a
/// stable it collects, three times when that breaks up its stable or
/// completes the proposer's. The *Zrušen distanc* card is worth `card_worth`.
fn accepts(t: &Trade, proposer: Entity, offers: &Query<(Entity, &TradeOffer, &OfferedBy)>, deeds: &Deeds, ai_money: i64, card_worth: i64) -> bool {
    let ai = t.partner;
    let worth = |card: Entity| if deeds.square(card).is_some() { deeds.value(card) } else { card_worth };
    let completes = t.give.iter().filter_map(|&sq| deeds.stable(sq)).any(|s| deeds.owns_stable_with(ai, s, &t.give));
    let gain = t.give.iter().map(|&c| worth(c)).sum::<i64>() * if completes { 3 } else { 2 } / 2 + t.money;
    // Stables it has offers out for, to complete them.
    let chasing: Vec<Stable> = offers.iter().filter(|o| o.2.0 == ai).flat_map(|o| o.1.0.take.iter()).filter_map(|&sq| deeds.stable(sq)).collect();
    let loss: i64 = t
        .take
        .iter()
        .map(|&sq| {
            let value = worth(sq);
            let Some(s) = deeds.stable(sq) else { return 2 * value };
            if deeds.owns_stable(ai, s) || deeds.owns_stable_with(proposer, s, &t.take) {
                3 * value
            } else if chasing.contains(&s) || deeds.stable_squares(s).any(|h| h != sq && deeds.owner(h) == Some(ai)) {
                5 * value / 2
            } else {
                2 * value
            }
        })
        .sum();
    gain >= loss && ai_money + t.money.min(0) >= RESERVE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::test_util::*;
    use bevy::ecs::system::RunSystemOnce;

    fn answer(app: &mut App, t: Trade, proposer: Entity) -> bool {
        app.world_mut()
            .run_system_once(move |offers: Query<(Entity, &TradeOffer, &OfferedBy)>, deeds: Deeds, wallets: Query<&Money>, pawns: Query<(Entity, &OnSquare)>, track: Track| {
                accepts(&t, proposer, &offers, &deeds, wallets.get(t.partner).unwrap().0, cancel_card_worth(t.partner, &pawns, &track, &deeds))
            })
            .unwrap()
    }

    fn card_worth(app: &mut App, ai: Entity) -> i64 {
        app.world_mut().run_system_once(move |pawns: Query<(Entity, &OnSquare)>, track: Track, deeds: Deeds| cancel_card_worth(ai, &pawns, &track, &deeds)).unwrap()
    }

    #[test]
    fn values_the_cancel_card_by_the_races_it_protects() {
        let mut app = app(2);
        let ai = player(&mut app, 1);
        give(&mut app, 1, "Zrušen distanc", Races::None);
        give(&mut app, 1, "Fantome", Races::None);
        give(&mut app, 1, "Gavora", Races::None);
        place(&mut app, 0, "Start");
        assert_eq!(card_worth(&mut app, ai), 0, "nothing to protect without races");
        give(&mut app, 1, "Gavora", Races::Three);
        let near = card_worth(&mut app, ai);
        // Gavora is three squares ahead: 1/6 × (1 800 − 40) × the turns in Distanc.
        assert!((600..800).contains(&near), "{near}");
        place(&mut app, 0, "Japan");
        assert_eq!(card_worth(&mut app, ai), 0, "too far to stop there in one roll");
        let (me, card) = (player(&mut app, 0), square(&mut app, "Zrušen distanc"));
        let t = |price| Trade { partner: ai, give: vec![], take: vec![card], money: price };
        place(&mut app, 0, "Start");
        assert!(!answer(&mut app, t(1000), me));
        assert!(answer(&mut app, t(1500), me));
    }

    #[test]
    fn keeps_horses_unless_paid_well() {
        let mut app = app(2);
        give(&mut app, 1, "Lukava", Races::None);
        let (me, ai, lukava) = (player(&mut app, 0), player(&mut app, 1), square(&mut app, "Lukava"));
        let t = |price| Trade { partner: ai, give: vec![], take: vec![lukava], money: price };
        assert!(!answer(&mut app, t(5400), me), "1.5× its price isn't worth a horse");
        assert!(answer(&mut app, t(7200), me));
        // A horse of a stable it collects is worth more.
        give(&mut app, 1, "Kostrava", Races::None);
        assert!(!answer(&mut app, t(7200), me));
        assert!(answer(&mut app, t(9000), me));
    }

    #[test]
    fn chases_only_stables_it_mostly_has() {
        let mut app = app(2);
        give(&mut app, 1, "Lukava", Races::None);
        give(&mut app, 0, "Kostrava", Races::None);
        give(&mut app, 0, "Melák", Races::None);
        let ai = player(&mut app, 1);
        let wants = |app: &mut App| app.world_mut().run_system_once(move |deeds: Deeds| wanted(ai, &deeds).len()).unwrap();
        assert_eq!(wants(&mut app), 0, "one horse of three");
        give(&mut app, 1, "Melák", Races::None);
        assert_eq!(wants(&mut app), 1);
    }
}
