//! Buying, races, selling, debts, bankruptcy and trading between players.

use super::*;

pub(super) fn plugin(app: &mut App) {
    app.add_observer(on_buy)
        .add_observer(on_buy_races)
        .add_observer(on_sell_to_bank)
        .add_observer(on_sell_race)
        .add_observer(on_settle_debts)
        .add_observer(on_liquidate)
        .add_observer(on_bankruptcy)
        .add_observer(on_propose_trade)
        .add_observer(on_accept_trade)
        .add_observer(on_cancel_trade)
        .add_observer(on_decline_trade)
        .add_systems(Update, lapse_offers.run_if(resource_exists::<Phase>));
}

#[derive(Event)]
pub struct Buy;

/// Buys `count` yellow races for the horse the player stopped on, or the main
/// race (1) once it has four – see `race_horse`.
#[derive(Event)]
pub struct BuyRaces(pub usize);

/// The player on turn sells one of their squares back to the bank for its price.
#[derive(Event)]
pub struct SellToBank(pub Entity);

/// The player on turn returns one race token of their horse to the bank for half its cost.
#[derive(Event)]
pub struct SellRace(pub Entity);

/// Sells the player's assets to the bank until out of debt, else bankrupts them.
#[derive(Event)]
pub struct Liquidate(pub Entity);

/// The player on turn offers a trade. The offer stands – nobody waits for
/// it – until the partner accepts it or either side calls it off. The same
/// deal offered again replaces the old offer (e.g. with a higher price).
#[derive(Event)]
pub struct ProposeTrade(pub Trade);

/// The partner takes up a standing offer, any time during the game.
#[derive(Event)]
pub struct AcceptTrade(pub Entity);

/// The proposer withdraws a standing offer.
#[derive(Event)]
pub struct CancelTrade(pub Entity);

/// The partner turns an offer down. People simply leave offers standing;
/// computers answer theirs at once.
#[derive(Event)]
pub struct DeclineTrade(pub Entity);

/// Standing offers a player may have at once.
pub const MAX_TRADE_OFFERS: usize = 3;

/// A standing trade offer; an entity of its own, gone with its proposer.
#[derive(Component, Clone, Debug)]
pub struct TradeOffer(pub Trade);

#[derive(Component, Debug)]
#[relationship(relationship_target = Offering)]
pub struct OfferedBy(pub Entity);

#[derive(Component, Debug, Default)]
#[relationship_target(relationship = OfferedBy, linked_spawn)]
pub struct Offering(Vec<Entity>);

/// Opponents in debt after someone else's move sell automatically.
#[derive(Event)]
pub(super) struct SettleDebts;

#[derive(Event)]
struct Bankruptcy(Entity);

impl Deeds<'_, '_> {
    /// Lowers the races by one level (the main race turns back into four) for half a race.
    fn sell_one_race(&mut self, sq: Entity, owner: Entity, ledger: &mut Ledger) -> Option<String> {
        let lower = self.races(sq).level().checked_sub(1).and_then(Races::from_level)?;
        let refund = self.field(sq).stable()?.race_cost() / 2;
        self.set_races(sq, lower);
        ledger.pay(Party::Bank, Party::Player(owner), refund);
        Some(format!("vrací dostih koně {} bance za {}", self.field(sq).name(), money(refund)))
    }

    fn sell_card(&self, sq: Entity, owner: Entity, ledger: &mut Ledger, commands: &mut Commands) -> String {
        let price = self.field(sq).price().expect("owned squares have a price");
        commands.entity(sq).remove::<OwnedBy>();
        ledger.pay(Party::Bank, Party::Player(owner), price);
        format!("prodává {} bance za {}", self.field(sq).name(), money(price))
    }
}

/// The horse the player on turn may buy races for: their own horse of a
/// complete stable they stopped on, once per visit.
pub fn race_horse(phase: &Phase, player: Entity, at: Entity, deeds: &Deeds) -> Option<Entity> {
    (matches!(phase, Phase::AfterMove { .. }) && deeds.can_add_races(player, at)).then_some(at)
}

/// The square the player on turn may buy: the one they stopped on, or – under
/// the `BuyBeforeRoll` house rule – the free one they stand on before rolling.
pub fn for_sale(phase: &Phase, before_roll: bool, at: Entity, deeds: &Deeds) -> Option<Entity> {
    match phase {
        Phase::AfterMove { offer } => *offer,
        Phase::PreRoll if before_roll && deeds.owner(at).is_none() && deeds.field(at).price().is_some() => Some(at),
        _ => None,
    }
}

/// The cards can change hands as agreed: each side owns what it hands over,
/// stables with races go whole, and something changes hands – money alone
/// will do, between players who cooperate.
pub fn trade_possible(t: &Trade, me: Entity, deeds: &Deeds) -> bool {
    let hands_over = |squares: &[Entity], owner: Entity| {
        squares.iter().all(|&sq| deeds.owner(sq) == Some(owner) && deeds.trade_group(sq).iter().all(|g| squares.contains(g)))
    };
    let something = !(t.give.is_empty() && t.take.is_empty() && t.money == 0);
    t.partner != me && something && hands_over(&t.give, me) && hands_over(&t.take, t.partner)
}

/// Whoever pays has the money. A player in debt may trade too, to raise money
/// from the others.
pub fn trade_affordable(t: &Trade, my_money: i64, partner_money: i64) -> bool {
    (t.money <= 0 || my_money >= t.money) && (t.money >= 0 || partner_money >= -t.money)
}

pub fn trade_valid(t: &Trade, me: Entity, deeds: &Deeds, my_money: i64, partner_money: i64) -> bool {
    trade_possible(t, me, deeds) && trade_affordable(t, my_money, partner_money)
}

/// "Fantome, Gavora, 1 000,- za Lukava" from the proposer's side.
pub fn describe_trade(t: &Trade, deeds: &Deeds) -> String {
    format!("{} za {}", trade_side(&t.give, t.money, deeds), trade_side(&t.take, -t.money, deeds))
}

/// What one side hands over: squares, and money if it pays.
pub fn trade_side(squares: &[Entity], money: i64, deeds: &Deeds) -> String {
    let mut parts: Vec<String> = squares.iter().map(|&sq| deeds.name(sq)).collect();
    if money > 0 {
        parts.push(super::money(money));
    }
    if parts.is_empty() { "nic".to_string() } else { parts.join(", ") }
}

fn on_buy(
    _: On<Buy>,
    turn: Res<Turn>,
    mut phase: ResMut<Phase>,
    before_roll: Option<Res<BuyBeforeRoll>>,
    names: Query<(&Player, &OnSquare)>,
    deeds: Deeds,
    mut ledger: Ledger,
    mut log: ResMut<GameLog>,
    mut commands: Commands,
) {
    let (player, at) = names.get(turn.player).expect("player on turn");
    let Some(sq) = for_sale(&phase, before_roll.is_some(), at.0, &deeds) else { return };
    let price = deeds.field(sq).price().expect("only ownable squares are offered");
    if ledger.balance(turn.player) < price {
        return;
    }
    ledger.pay(Party::Player(turn.player), Party::Bank, price);
    commands.entity(sq).insert(OwnedBy(turn.player));
    phase.set_offer(None);
    log.say(format!("{} kupuje {} za {}", player.name, deeds.field(sq).name(), money(price)));
}

fn on_buy_races(
    ev: On<BuyRaces>,
    turn: Res<Turn>,
    phase: Res<Phase>,
    names: Query<(&Player, &OnSquare)>,
    mut deeds: Deeds,
    mut ledger: Ledger,
    mut log: ResMut<GameLog>,
) {
    let n = ev.0;
    let (player, at) = names.get(turn.player).expect("player on turn");
    let Some(sq) = race_horse(&phase, turn.player, at.0, &deeds) else { return };
    let current = deeds.races(sq);
    let cost = deeds.field(sq).stable().expect("races only on horses").race_cost() * n as i64;
    let Some(target) = Races::from_level(current.level() + n) else { return };
    if !current.buyable().contains(&n) || ledger.balance(turn.player) < cost {
        return;
    }
    ledger.pay(Party::Player(turn.player), Party::Bank, cost);
    deeds.set_races(sq, target);
    ledger.commands().entity(sq).insert(RacedThisTurn);
    let (name, horse) = (&player.name, deeds.field(sq).name());
    log.say(if target == Races::Main {
        format!("{name} staví koně {horse} do hlavního dostihu sezóny ({}).", money(cost))
    } else {
        format!("{name} kupuje {n}× dostih pro koně {horse} ({}).", money(cost))
    });
}

fn on_sell_to_bank(ev: On<SellToBank>, turn: Res<Turn>, deeds: Deeds, names: Query<&Player>, mut ledger: Ledger, mut log: ResMut<GameLog>, mut commands: Commands) {
    let owner = turn.player;
    if deeds.can_sell_to_bank(owner, ev.0) {
        let what = deeds.sell_card(ev.0, owner, &mut ledger, &mut commands);
        log.say(format!("{} {what}", names.get(owner).unwrap().name));
    }
}

fn on_sell_race(ev: On<SellRace>, turn: Res<Turn>, mut deeds: Deeds, names: Query<&Player>, mut ledger: Ledger, mut log: ResMut<GameLog>) {
    let owner = turn.player;
    if deeds.owner(ev.0) != Some(owner) {
        return;
    }
    if let Some(what) = deeds.sell_one_race(ev.0, owner, &mut ledger) {
        log.say(format!("{} {what}", names.get(owner).unwrap().name));
    }
}

fn on_settle_debts(_: On<SettleDebts>, turn: Res<Turn>, players: Query<(Entity, &Money)>, mut commands: Commands) {
    for (e, money) in &players {
        if e != turn.player && money.0 < 0 {
            commands.trigger(Liquidate(e));
        }
    }
}

/// Races go first (cheapest horses first), then cards from the cheapest.
fn on_liquidate(ev: On<Liquidate>, mut deeds: Deeds, names: Query<&Player>, mut ledger: Ledger, mut log: ResMut<GameLog>, mut commands: Commands) {
    let p = ev.0;
    let Ok(player) = names.get(p) else { return };
    let mut owned = deeds.owned_by(p);
    owned.sort_by_key(|&sq| deeds.field(sq).price());
    while ledger.balance(p) < 0 {
        let what = if let Some(&sq) = owned.iter().find(|&&sq| deeds.races(sq) > Races::None) {
            deeds.sell_one_race(sq, p, &mut ledger).expect("has races")
        } else if !owned.is_empty() {
            deeds.sell_card(owned.remove(0), p, &mut ledger, &mut commands)
        } else {
            commands.trigger(Bankruptcy(p));
            return;
        };
        log.say(format!("{} {what}", player.name));
    }
}

/// The player leaves the game; despawning releases their cards (squares lose
/// their races, the *Zrušen distanc* card returns to the deck).
fn on_bankruptcy(
    ev: On<Bankruptcy>,
    mut turn: ResMut<Turn>,
    players: Query<(Entity, &Player)>,
    offers: Query<(Entity, &TradeOffer)>,
    mut eliminated: ResMut<Eliminated>,
    mut phase: ResMut<Phase>,
    mut log: ResMut<GameLog>,
    mut commands: Commands,
) {
    let Ok((p, player)) = players.get(ev.0) else { return };
    log.say(format!("{} zbankrotoval a končí ve hře.", player.name));
    eliminated.0.push(player.clone());
    // Their own offers go with them; offers to them go too.
    for (offer, TradeOffer(t)) in &offers {
        if t.partner == p {
            commands.entity(offer).try_despawn();
        }
    }
    commands.entity(p).despawn();
    let rest: Vec<_> = players.iter().filter(|(e, ..)| *e != p).collect();
    if let [(winner, w)] = rest[..] {
        turn.player = winner;
        *phase = Phase::GameOver { winner: Some(winner) };
        log.say(format!("Vítězem je {}!", w.name));
    } else if p == turn.player {
        commands.trigger(NextTurn { after_seat: player.seat });
    }
}

fn on_propose_trade(
    ev: On<ProposeTrade>,
    turn: Res<Turn>,
    phase: Res<Phase>,
    names: Query<(&Player, Option<&Offering>)>,
    offers: Query<&TradeOffer>,
    deeds: Deeds,
    ledger: Ledger,
    mut log: ResMut<GameLog>,
    mut commands: Commands,
) {
    let (me, t) = (turn.player, &ev.0);
    let (Ok((player, mine)), Ok((partner, _))) = (names.get(me), names.get(t.partner)) else { return };
    if !matches!(*phase, Phase::AfterMove { .. }) || !trade_valid(t, me, &deeds, ledger.balance(me), ledger.balance(t.partner)) {
        return;
    }
    let mine: Vec<Entity> = mine.map_or(Vec::new(), |o| o.iter().collect());
    let same = |o: &Entity| offers.get(*o).is_ok_and(|TradeOffer(old)| (old.partner, &old.give, &old.take) == (t.partner, &t.give, &t.take));
    let replaced = mine.iter().find(|o| same(o));
    if replaced.is_none() && mine.len() >= MAX_TRADE_OFFERS {
        return;
    }
    if let Some(&old) = replaced {
        commands.entity(old).despawn();
    }
    commands.spawn((TradeOffer(t.clone()), OfferedBy(me)));
    log.say(format!("{} nabízí hráči {}: {}", player.name, partner.name, describe_trade(t, &deeds)));
}

fn on_accept_trade(
    ev: On<AcceptTrade>,
    phase: Res<Phase>,
    offers: Query<(&TradeOffer, &OfferedBy)>,
    names: Query<&Player>,
    deeds: Deeds,
    mut ledger: Ledger,
    mut log: ResMut<GameLog>,
    mut commands: Commands,
) {
    let Ok((TradeOffer(t), by)) = offers.get(ev.0) else { return };
    let (proposer, partner) = (by.0, t.partner);
    let (Ok(a), Ok(b)) = (names.get(proposer), names.get(partner)) else { return };
    // Short of money for now, the offer waits; `lapse_offers` drops impossible ones.
    if matches!(*phase, Phase::GameOver { .. }) || !trade_valid(t, proposer, &deeds, ledger.balance(proposer), ledger.balance(partner)) {
        return;
    }
    commands.entity(ev.0).try_despawn();
    if t.money >= 0 {
        ledger.pay(Party::Player(proposer), Party::Player(partner), t.money);
    } else {
        ledger.pay(Party::Player(partner), Party::Player(proposer), -t.money);
    }
    for &sq in &t.give {
        commands.entity(sq).insert(OwnedBy(partner));
    }
    for &sq in &t.take {
        commands.entity(sq).insert(OwnedBy(proposer));
    }
    log.say(format!("{} přijímá nabídku hráče {}: {}", b.name, a.name, describe_trade(t, &deeds)));
}

/// Takes standing offers off the table, each with a line in the log.
#[derive(SystemParam)]
struct OfferBook<'w, 's> {
    offers: Query<'w, 's, (Entity, &'static TradeOffer, &'static OfferedBy)>,
    names: Query<'w, 's, &'static Player>,
    deeds: Deeds<'w, 's>,
    log: ResMut<'w, GameLog>,
    commands: Commands<'w, 's>,
}

impl OfferBook<'_, '_> {
    /// `say` gets the names of the proposer and the partner.
    fn close(&mut self, offer: Entity, say: impl Fn(&str, &str) -> String) {
        let Ok((_, TradeOffer(t), by)) = self.offers.get(offer) else { return };
        let name = |p: Entity| self.names.get(p).map_or("?", |p| p.name.as_str());
        self.log.say(format!("{}: {}", say(name(by.0), name(t.partner)), describe_trade(t, &self.deeds)));
        self.commands.entity(offer).try_despawn();
    }
}

fn on_cancel_trade(ev: On<CancelTrade>, mut book: OfferBook) {
    book.close(ev.0, |a, b| format!("{a} stahuje nabídku pro hráče {b}"));
}

fn on_decline_trade(ev: On<DeclineTrade>, mut book: OfferBook) {
    book.close(ev.0, |a, b| format!("{b} odmítá nabídku hráče {a}"));
}

/// An offer that can no longer be carried out lapses at once – a square in it
/// changed hands or got races – rather than waiting in the proposer's few
/// slots. Running short of money only holds it up (see `on_accept_trade`).
fn lapse_offers(mut book: OfferBook) {
    let lapsed: Vec<Entity> = book.offers.iter().filter(|(_, TradeOffer(t), by)| !trade_possible(t, by.0, &book.deeds)).map(|o| o.0).collect();
    for offer in lapsed {
        book.close(offer, |a, b| format!("Nabídka {a} pro {b} už neplatí"));
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_util::*;
    use super::*;

    #[test]
    fn buying_and_races() {
        let mut app = app(2);
        let fantome = square(&mut app, "Fantome");
        set_phase(&mut app, Phase::AfterMove { offer: Some(fantome) });
        trigger(&mut app, Buy);
        assert_eq!(owner(&mut app, "Fantome"), Some(player(&mut app, 0)));
        place(&mut app, 0, "Fantome");
        trigger(&mut app, BuyRaces(1));
        assert_eq!(races(&mut app, "Fantome"), Races::None, "stable not complete");
        give(&mut app, 0, "Gavora", Races::None);
        place(&mut app, 0, "Start");
        trigger(&mut app, BuyRaces(1));
        assert_eq!(races(&mut app, "Fantome"), Races::None, "only when standing on the horse");
        place(&mut app, 0, "Fantome");
        trigger(&mut app, BuyRaces(5));
        assert_eq!(races(&mut app, "Fantome"), Races::None, "can't jump to the main race");
        trigger(&mut app, BuyRaces(4));
        assert_eq!(races(&mut app, "Fantome"), Races::Four);
        trigger(&mut app, BuyRaces(1));
        assert_eq!(races(&mut app, "Fantome"), Races::Four, "the main race takes another visit");
        trigger(&mut app, NextTurn { after_seat: 1 });
        set_phase(&mut app, Phase::PreRoll);
        trigger(&mut app, BuyRaces(1));
        assert_eq!(races(&mut app, "Fantome"), Races::Four, "only after stopping there");
        after_move(&mut app);
        trigger(&mut app, BuyRaces(1));
        assert_eq!(races(&mut app, "Fantome"), Races::Main);
        assert_eq!(balance(&mut app, 0), START_MONEY - 1200 - 5000);
        let gavora = square(&mut app, "Gavora");
        trigger(&mut app, SellToBank(gavora));
        assert!(owner(&mut app, "Gavora").is_some(), "a stable with races can't be split");
        trigger(&mut app, SellRace(fantome));
        assert_eq!(races(&mut app, "Fantome"), Races::Four);
        assert_eq!(balance(&mut app, 0), START_MONEY - 6200 + 500);
    }

    /// Offers `t` and returns the standing offer, if it was made.
    fn offer(app: &mut App, t: Trade) -> Option<Entity> {
        trigger(app, ProposeTrade(t.clone()));
        let mut q = app.world_mut().query::<(Entity, &TradeOffer)>();
        q.iter(app.world()).find(|(_, o)| o.0 == t).map(|(e, _)| e)
    }

    #[test]
    fn offers_stand_until_accepted_or_withdrawn() {
        let mut app = app(2);
        after_move(&mut app);
        give(&mut app, 1, "Lukava", Races::None);
        let (partner, lukava) = (player(&mut app, 1), square(&mut app, "Lukava"));
        let low = offer(&mut app, Trade { partner, give: vec![], take: vec![lukava], money: 3000 }).unwrap();
        assert!(phase(&app).can_end_turn(START_MONEY), "an offer blocks nobody");
        let high = offer(&mut app, Trade { partner, give: vec![], take: vec![lukava], money: 4000 }).unwrap();
        assert!(app.world().get_entity(low).is_err(), "the same deal again replaces the offer");
        trigger(&mut app, CancelTrade(high));
        assert!(app.world().get_entity(high).is_err());
        // A few different deals at once, but no more.
        for (k, name) in ["1. Trenér", "2. Trenér", "3. Trenér", "4. Trenér"].into_iter().enumerate() {
            give(&mut app, 0, name, Races::None);
            let sq = square(&mut app, name);
            let made = offer(&mut app, Trade { partner, give: vec![sq], take: vec![], money: -100 });
            assert_eq!(made.is_some(), k < MAX_TRADE_OFFERS);
        }
        let mut offers = app.world_mut().query::<(Entity, &TradeOffer)>();
        let all: Vec<Entity> = offers.iter(app.world()).map(|(e, _)| e).collect();
        trigger(&mut app, CancelTrade(all[0]));
        let later = offer(&mut app, Trade { partner, give: vec![], take: vec![lukava], money: 9000 }).unwrap();
        // The partner accepts on their own turn.
        trigger(&mut app, EndTurn);
        trigger(&mut app, AcceptTrade(later));
        assert_eq!(owner(&mut app, "Lukava"), Some(player(&mut app, 0)));
        assert_eq!(balance(&mut app, 1), START_MONEY + 9000);
    }

    #[test]
    fn offers_lapse_when_they_can_no_longer_be_carried_out() {
        let mut app = app(3);
        after_move(&mut app);
        give(&mut app, 1, "Lukava", Races::None);
        let (partner, lukava) = (player(&mut app, 1), square(&mut app, "Lukava"));
        let o = offer(&mut app, Trade { partner, give: vec![], take: vec![lukava], money: 3000 }).unwrap();
        // Meanwhile someone else gets the horse.
        give(&mut app, 2, "Lukava", Races::None);
        app.update();
        assert!(app.world().get_entity(o).is_err());
        let seller = player(&mut app, 2);
        let o = offer(&mut app, Trade { partner: seller, give: vec![], take: vec![lukava], money: 3000 }).unwrap();
        // Short of money, the proposer's offer waits for better times.
        set(&mut app, 0, Money(2000));
        app.update();
        trigger(&mut app, AcceptTrade(o));
        assert_eq!(owner(&mut app, "Lukava"), Some(seller), "can't be paid now");
        set(&mut app, 0, Money(5000));
        trigger(&mut app, AcceptTrade(o));
        assert_eq!(owner(&mut app, "Lukava"), Some(player(&mut app, 0)));
    }

    #[test]
    fn money_alone_can_change_hands() {
        let mut app = app(2);
        after_move(&mut app);
        let partner = player(&mut app, 1);
        assert_eq!(offer(&mut app, Trade { partner, give: vec![], take: vec![], money: 0 }), None, "nothing for nothing");
        let o = offer(&mut app, Trade { partner, give: vec![], take: vec![], money: 2000 }).unwrap();
        trigger(&mut app, AcceptTrade(o));
        assert_eq!((balance(&mut app, 0), balance(&mut app, 1)), (START_MONEY - 2000, START_MONEY + 2000));
    }

    #[test]
    fn cancel_card_trades_like_a_deed() {
        let mut app = app(3);
        after_move(&mut app);
        // Drawn by the partner earlier.
        app.world_mut().resource_mut::<Decks>().nahoda.retain(|&i| NAHODA[i].1 != NahodaCard::CancelDistanc);
        give(&mut app, 1, "Zrušen distanc", Races::None);
        let (partner, card) = (player(&mut app, 1), square(&mut app, "Zrušen distanc"));
        let o = offer(&mut app, Trade { partner, give: vec![], take: vec![card], money: 500 }).unwrap();
        trigger(&mut app, AcceptTrade(o));
        assert_eq!(owner(&mut app, "Zrušen distanc"), Some(player(&mut app, 0)));
        // A bankrupt holder's card goes back under the deck.
        give(&mut app, 2, "Zrušen distanc", Races::None);
        set(&mut app, 2, Money(-1));
        trigger(&mut app, SettleDebts);
        assert_eq!(owner(&mut app, "Zrušen distanc"), None);
        let in_deck = app.world().resource::<Decks>().nahoda.iter().filter(|&&i| NAHODA[i].1 == NahodaCard::CancelDistanc).count();
        assert_eq!(in_deck, 1);
    }

    #[test]
    fn house_rule_buys_the_free_square_before_rolling() {
        let mut app = app(2);
        place(&mut app, 0, "Fantome");
        trigger(&mut app, Buy);
        assert_eq!(owner(&mut app, "Fantome"), None, "only after stopping there");
        app.world_mut().insert_resource(BuyBeforeRoll);
        trigger(&mut app, Buy);
        assert_eq!(owner(&mut app, "Fantome"), Some(player(&mut app, 0)));
        assert_eq!(phase(&app), Phase::PreRoll, "still to roll");
        assert_eq!(balance(&mut app, 0), START_MONEY - 1200);
    }

    #[test]
    fn whole_stable_with_races_trades_together() {
        let mut app = app(2);
        after_move(&mut app);
        give(&mut app, 0, "Fantome", Races::One);
        give(&mut app, 0, "Gavora", Races::None);
        let partner = player(&mut app, 1);
        let (fantome, gavora) = (square(&mut app, "Fantome"), square(&mut app, "Gavora"));
        let t = Trade { partner, give: vec![fantome], take: vec![], money: -5000 };
        assert_eq!(offer(&mut app, t.clone()), None, "half a stable with races is refused");
        let o = offer(&mut app, Trade { give: vec![fantome, gavora], ..t }).unwrap();
        trigger(&mut app, AcceptTrade(o));
        assert_eq!(owner(&mut app, "Gavora"), Some(partner));
        assert_eq!(races(&mut app, "Fantome"), Races::One);
        assert_eq!(balance(&mut app, 0), START_MONEY + 5000);
        assert_eq!(balance(&mut app, 1), START_MONEY - 5000);
    }

    #[test]
    fn swap_horses_with_money_on_top() {
        let mut app = app(2);
        after_move(&mut app);
        give(&mut app, 0, "Fantome", Races::None);
        give(&mut app, 1, "Lukava", Races::None);
        let partner = player(&mut app, 1);
        let (fantome, lukava) = (square(&mut app, "Fantome"), square(&mut app, "Lukava"));
        let o = offer(&mut app, Trade { partner, give: vec![fantome], take: vec![lukava], money: 1500 }).unwrap();
        trigger(&mut app, AcceptTrade(o));
        assert_eq!((owner(&mut app, "Fantome"), owner(&mut app, "Lukava")), (Some(partner), Some(player(&mut app, 0))));
        assert_eq!(balance(&mut app, 0), START_MONEY - 1500);
        assert_eq!(balance(&mut app, 1), START_MONEY + 1500);
    }

    #[test]
    fn player_in_debt_may_sell_to_another() {
        let mut app = app(2);
        after_move(&mut app);
        give(&mut app, 0, "Fantome", Races::None);
        set(&mut app, 0, Money(-2000));
        let partner = player(&mut app, 1);
        let fantome = square(&mut app, "Fantome");
        assert_eq!(offer(&mut app, Trade { partner, give: vec![fantome], take: vec![], money: 1 }), None, "can't pay while in debt");
        let o = offer(&mut app, Trade { partner, give: vec![fantome], take: vec![], money: -1000 }).unwrap();
        trigger(&mut app, AcceptTrade(o));
        assert_eq!(balance(&mut app, 0), -1000);
        assert_eq!(owner(&mut app, "Fantome"), Some(partner));
    }

    #[test]
    fn debt_forces_liquidation_then_bankruptcy() {
        let mut app = app(3);
        after_move(&mut app);
        give(&mut app, 1, "Fantome", Races::None);
        set(&mut app, 1, Money(-1000));
        trigger(&mut app, SettleDebts);
        assert_eq!(balance(&mut app, 1), 200);
        assert_eq!(owner(&mut app, "Fantome"), None);

        give(&mut app, 2, "Napoli", Races::Two);
        set(&mut app, 2, Money(-100_000));
        trigger(&mut app, SettleDebts);
        assert_eq!(owner(&mut app, "Napoli"), None, "a bankrupt player's squares return to the bank");
        assert_eq!(races(&mut app, "Napoli"), Races::None);
        assert_eq!(app.world().resource::<Eliminated>().0.len(), 1);

        set(&mut app, 1, Money(-1));
        trigger(&mut app, SettleDebts);
        let p0 = player(&mut app, 0);
        assert_eq!(phase(&app), Phase::GameOver { winner: Some(p0) });
    }
}
