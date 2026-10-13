//! Finance and Náhoda cards.

use super::*;

pub(super) fn plugin(app: &mut App) {
    app.add_observer(on_draw_card).add_observer(return_cancel_card);
}

/// The *Zrušen distanc* card goes under the Náhoda deck once nobody holds it
/// – used, or its holder went bankrupt.
fn return_cancel_card(ev: On<Remove, OwnedBy>, cards: Query<(), With<CancelCard>>, decks: Option<ResMut<Decks>>) {
    if let (true, Some(mut decks)) = (cards.contains(ev.entity), decks) {
        decks.nahoda.push_back(NAHODA.iter().position(|c| c.1 == NahodaCard::CancelDistanc).expect("the deck has the card"));
    }
}

#[derive(Event)]
pub(super) struct DrawCard(pub Deck);

fn on_draw_card(
    ev: On<DrawCard>,
    turn: Res<Turn>,
    mut players: Query<(Entity, &Player, &OnSquare, &mut Status)>,
    deeds: Deeds,
    track: Track,
    mut ledger: Ledger,
    mut decks: ResMut<Decks>,
    mut log: ResMut<GameLog>,
) {
    let me = turn.player;
    let deck = match ev.0 {
        Deck::Finance => &mut decks.finance,
        Deck::Nahoda => &mut decks.nahoda,
    };
    let idx = deck.pop_front().expect("decks are never empty");
    let text = match ev.0 {
        Deck::Finance => FINANCE[idx].0,
        Deck::Nahoda => NAHODA[idx].0,
    };
    // The kept card returns to the deck only once it's used.
    let kept = ev.0 == Deck::Nahoda && NAHODA[idx].1 == NahodaCard::CancelDistanc;
    if !kept {
        deck.push_back(idx);
    }
    let (_, player, at, mut status) = players.get_mut(me).expect("player on turn");
    log.say(format!("{} – {}: {text}", player.name, if ev.0 == Deck::Finance { "Finance" } else { "Náhoda" }));
    // Through the ledger, so the card shows before the money it moves.
    ledger.commands().trigger(CardDrawn { deck: ev.0, text });

    match ev.0 {
        Deck::Nahoda => match NAHODA[idx].1 {
            NahodaCard::CancelDistanc => {
                let card = deeds.cancel_card();
                ledger.commands().entity(card).insert(OwnedBy(me));
            }
            NahodaCard::Sit(turns) => *status = Status::Sitting { turns, doping: false },
            NahodaCard::Walk { direction, to, start_bonus } => {
                let pay = if start_bonus { StartPay::Bonus } else { StartPay::Nothing };
                ledger.commands().trigger(Travel { path: track.walk(at.0, direction, to), pay });
            }
        },
        Deck::Finance => match FINANCE[idx].1 {
            FinanceCard::Pay(x) => ledger.fine(me, x),
            FinanceCard::Receive(x) => ledger.pay(Party::Bank, Party::Player(me), x),
            FinanceCard::RaceWinnings(x) => {
                if deeds.owned_by(me).into_iter().any(|sq| deeds.races(sq) > Races::None) {
                    ledger.pay(Party::Bank, Party::Player(me), x);
                } else {
                    log.say(format!("{} nemá žádné dostihy, takže nic nedostává.", player.name));
                }
            }
            FinanceCard::FromEachPlayer(x) => {
                for (other, ..) in players.iter().filter(|p| p.0 != me) {
                    ledger.pay(Party::Player(other), Party::Player(me), x);
                }
            }
            FinanceCard::PerRace { race, main } => {
                let total: i64 = deeds
                    .owned_by(me)
                    .into_iter()
                    .map(|sq| match deeds.races(sq) {
                        Races::Main => main,
                        r => r.level() as i64 * race,
                    })
                    .sum();
                ledger.fine(me, total);
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_util::*;
    use super::*;

    fn draw(app: &mut App, deck: Deck, idx: usize) {
        let mut decks = app.world_mut().resource_mut::<Decks>();
        let cards = if deck == Deck::Finance { &mut decks.finance } else { &mut decks.nahoda };
        cards.retain(|&i| i != idx);
        cards.push_front(idx);
        trigger(app, DrawCard(deck));
    }

    fn position(app: &mut App) -> String {
        let p0 = player(app, 0);
        let sq = app.world().get::<OnSquare>(p0).unwrap().0;
        app.world().get::<Square>(sq).unwrap().0.name()
    }

    fn nahoda_squares(app: &mut App) -> Vec<Entity> {
        let mut q = app.world_mut().query::<(Entity, &Square)>();
        q.iter(app.world()).filter(|(_, s)| s.0 == Field::Nahoda).map(|(e, _)| e).collect()
    }

    /// Moving Náhoda cards never end on another Náhoda, so card chains are short.
    #[test]
    fn nahoda_cards_never_chain_into_nahoda() {
        let mut app = app(2);
        after_move(&mut app);
        let p0 = player(&mut app, 0);
        for (idx, (_, card)) in NAHODA.iter().enumerate() {
            for sq in nahoda_squares(&mut app) {
                app.world_mut().entity_mut(p0).insert(OnSquare(sq));
                draw(&mut app, Deck::Nahoda, idx);
                let now = app.world().get::<OnSquare>(p0).unwrap().0;
                assert!(now == sq || position(&mut app) != "Náhoda", "{card:?} chains into Náhoda");
            }
        }
    }

    #[test]
    fn back_to_distanc_pays_start_bonus_only_when_crossing_start() {
        let mut app = app(2);
        after_move(&mut app);
        let card = NAHODA.iter().position(|c| c.0.starts_with("Zpět na pole Distanc")).unwrap();
        let nahodas = nahoda_squares(&mut app);
        let p0 = player(&mut app, 0);
        let mut gained = Vec::new();
        for sq in nahodas {
            app.world_mut().entity_mut(p0).insert((OnSquare(sq), Status::Free));
            let before = balance(&mut app, 0);
            draw(&mut app, Deck::Nahoda, card);
            assert_eq!(position(&mut app), "Distanc");
            gained.push(balance(&mut app, 0) - before);
        }
        gained.sort();
        assert_eq!(gained, [0, 0, START_BONUS], "only the first Náhoda lies before Distanc");
    }

    #[test]
    fn card_is_announced_before_its_money_moves() {
        #[derive(Resource, Default)]
        struct Seen(Vec<&'static str>);
        let mut app = app(3);
        app.init_resource::<Seen>()
            .add_observer(|_: On<CardDrawn>, mut s: ResMut<Seen>| s.0.push("card"))
            .add_observer(|_: On<MoneyMoved>, mut s: ResMut<Seen>| s.0.push("money"));
        let card = FINANCE.iter().position(|c| matches!(c.1, FinanceCard::FromEachPlayer(_))).unwrap();
        draw(&mut app, Deck::Finance, card);
        assert_eq!(app.world().resource::<Seen>().0, ["card", "money", "money"]);
    }

    #[test]
    fn race_winnings_need_races() {
        let mut app = app(2);
        let card = FINANCE.iter().position(|c| matches!(c.1, FinanceCard::RaceWinnings(_))).unwrap();
        draw(&mut app, Deck::Finance, card);
        assert_eq!(balance(&mut app, 0), START_MONEY);
        give(&mut app, 0, "Fantome", Races::One);
        draw(&mut app, Deck::Finance, card);
        assert_eq!(balance(&mut app, 0), START_MONEY + 2000);
    }

    #[test]
    fn birthday_collects_from_everyone() {
        let mut app = app(3);
        let card = FINANCE.iter().position(|c| matches!(c.1, FinanceCard::FromEachPlayer(_))).unwrap();
        draw(&mut app, Deck::Finance, card);
        assert_eq!(balance(&mut app, 0), START_MONEY + 400);
        assert_eq!(balance(&mut app, 2), START_MONEY - 200);
    }

    #[test]
    fn cancel_card_is_kept_until_used() {
        let mut app = app(2);
        let card = NAHODA.iter().position(|c| c.1 == NahodaCard::CancelDistanc).unwrap();
        draw(&mut app, Deck::Nahoda, card);
        let p0 = player(&mut app, 0);
        assert_eq!(owner(&mut app, "Zrušen distanc"), Some(p0));
        assert!(!app.world().resource::<Decks>().nahoda.contains(&card));
        set(&mut app, 0, Status::Distanc);
        set_phase(&mut app, Phase::PreRoll);
        trigger(&mut app, UseCancelCard);
        assert_eq!(*app.world().get::<Status>(p0).unwrap(), Status::Free);
        assert_eq!(app.world().resource::<Decks>().nahoda.back(), Some(&card));
    }
}
