//! The wooden side panel (players, actions, property, trading, log) and the
//! card overlay over the board. Rebuilt from scratch whenever the game changes
//! and the animations have caught up.

use super::board::{BoardItem, not_busy};
use super::widgets::*;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::ui::RelativeCursorPosition;
use super::*;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<PanelScrolled>()
        .init_resource::<CutOff>()
        .add_systems(Update, (follow_phase, draw).chain().run_if(in_state(AppState::Playing).and_then(not_busy).and_then(changed)))
        .add_systems(Update, (scroll, fan_hands))
        .add_systems(PostUpdate, find_cut_off.after(bevy::ui::UiSystems::Layout).run_if(in_state(AppState::Playing)));
}

/// A list in the panel that scrolls by itself – the history, the offers.
#[derive(Component)]
struct Scrolls;

/// The panel's column; it scrolls only when its content doesn't fit, which
/// `find_cut_off` reports.
#[derive(Component)]
struct PanelColumn;

/// Overlays over the board: a square's card, the trade dialog.
#[derive(Component)]
struct BoardOverlay;

/// How far the panel is scrolled, kept across redraws.
#[derive(Resource, Default)]
struct PanelScrolled(f32);

/// The wheel scrolls the history when over it, else the panel.
fn scroll(
    mut wheel: MessageReader<MouseWheel>,
    mut logs: Query<(&mut ScrollPosition, &RelativeCursorPosition), (With<Scrolls>, Without<PanelColumn>)>,
    mut panels: Query<(&mut ScrollPosition, &RelativeCursorPosition), With<PanelColumn>>,
    mut scrolled: ResMut<PanelScrolled>,
) {
    for ev in wheel.read() {
        let dy = match ev.unit {
            MouseScrollUnit::Line => ev.y * 24.0,
            MouseScrollUnit::Pixel => ev.y,
        };
        if let Some((mut pos, _)) = logs.iter_mut().find(|(_, c)| c.cursor_over()) {
            pos.y = (pos.y - dy).max(0.0);
        } else if let Some((mut pos, _)) = panels.iter_mut().find(|(_, c)| c.cursor_over()) {
            pos.y = (pos.y - dy).max(0.0);
            scrolled.0 = pos.y;
        }
    }
}

/// Parts of the screen whose content doesn't fit: the panel had to scroll, or
/// an overlay is larger than the board. Empty when everything is visible;
/// the screenshot tool fails on it.
#[derive(Resource, Default, Debug)]
pub struct CutOff(pub Vec<String>);

fn find_cut_off(nodes: Query<(&ComputedNode, Has<PanelColumn>), Or<(With<PanelColumn>, With<BoardOverlay>)>>, mut cut_off: ResMut<CutOff>) {
    let found: Vec<String> = nodes
        .iter()
        .filter(|(n, _)| !n.content_size.cmple(n.size + 1.0).all())
        .map(|(n, panel)| format!("{} content {} doesn't fit {}", if panel { "panel" } else { "board overlay" }, n.content_size, n.size))
        .collect();
    if cut_off.0 != found {
        cut_off.0 = found;
    }
}

/// The root of the panel UI.
#[derive(Component)]
struct PanelRoot;

fn changed(
    busy: Res<Busy>,
    phase: Res<Phase>,
    log: Res<GameLog>,
    ui: Res<UiState>,
    q: Query<(), Or<(Changed<Money>, Changed<Status>, Changed<OnSquare>, Changed<Races>, Changed<OwnedBy>)>>,
) -> bool {
    busy.is_changed() || phase.is_changed() || log.is_changed() || ui.is_changed() || !q.is_empty()
}

/// The card of the square stopped on, if for sale, or of a horse that can get
/// races, opens by itself and closes once that's resolved; a new turn starts
/// with a clean slate. A square for sale before the roll gets only its button
/// in the panel – a card would hide the board just when the turn starts.
fn follow_phase(
    phase: Res<Phase>,
    turn: Res<Turn>,
    at: Query<&OnSquare>,
    deeds: Deeds,
    mut ui: ResMut<UiState>,
    mut opened: Local<Option<Entity>>,
) {
    if turn.is_changed() {
        *ui = UiState::default();
    }
    let me = turn.player;
    let Ok(&OnSquare(at)) = at.get(me) else { return };
    let wanted = phase.offer().or_else(|| race_horse(&phase, me, at, &deeds));
    if wanted == *opened {
        return;
    }
    if let Some(sq) = opened.take()
        && ui.selected == Some(sq)
    {
        ui.selected = None;
    }
    if let Some(sq) = wanted {
        ui.selected = Some(sq);
    }
    *opened = wanted;
}

/// A player as the panel shows them.
#[derive(Clone, Copy)]
struct Row<'a> {
    entity: Entity,
    player: &'a Player,
    money: i64,
    status: Status,
    ai: bool,
}

/// Everything the panel shows, gathered once so drawing doesn't juggle queries.
struct View<'a> {
    players: Vec<Row<'a>>,
    me: usize,
    phase: &'a Phase,
    eliminated: &'a [Player],
    owned: Vec<Vec<Entity>>,
    /// The square the player on turn stands on.
    at: Entity,
    /// The square the player on turn may buy now.
    for_sale: Option<Entity>,
    /// Standing trade offers with their proposers, and whether the money's there now.
    offers: Vec<(Entity, Entity, &'a Trade, bool)>,
    trade_ok: bool,
    log: &'a [String],
}

impl View<'_> {
    fn me(&self) -> Row<'_> {
        self.players[self.me]
    }

    fn player(&self, e: Entity) -> Option<Row<'_>> {
        self.players.iter().find(|p| p.entity == e).copied()
    }

    /// Trading happens after the roll, when a person is on turn.
    fn can_trade(&self) -> bool {
        !self.me().ai && matches!(self.phase, Phase::AfterMove { .. })
    }

    /// A person on turn may sell their own squares any time.
    fn can_sell(&self, sq: Entity, deeds: &Deeds) -> bool {
        let me = self.me();
        !me.ai && deeds.owner(sq) == Some(me.entity) && !matches!(self.phase, Phase::GameOver { .. })
    }
}

fn draw(
    mut commands: Commands,
    roots: Query<Entity, With<PanelRoot>>,
    art: Res<Art>,
    ui: Res<UiState>,
    turn: Res<Turn>,
    phase: Res<Phase>,
    log: Res<GameLog>,
    eliminated: Res<Eliminated>,
    players: Query<(Entity, &Player, &Money, &Status, &OnSquare, Has<Ai>)>,
    deeds: Deeds,
    track: Track,
    scrolled: Res<PanelScrolled>,
    offers: Query<(Entity, &TradeOffer, &OfferedBy)>,
    before_roll: Option<Res<BuyBeforeRoll>>,
) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    let mut list: Vec<Row> = players
        .iter()
        .map(|(entity, player, money, status, _, ai)| Row { entity, player, money: money.0, status: *status, ai })
        .collect();
    list.sort_by_key(|p| p.player.seat);
    let Some(me) = list.iter().position(|p| p.entity == turn.player) else { return };
    let money_of = |p: Entity| list.iter().find(|r| r.entity == p).map_or(0, |r| r.money);
    let trade_ok = ui.draft.as_ref().and_then(Draft::trade).is_some_and(|t| trade_valid(&t, turn.player, &deeds, money_of(turn.player), money_of(t.partner)));
    // Like cards in a hand: horses by stable, then trainers, services and the Zrušen distanc card.
    let in_order = |p: Entity| {
        let mut cards: Vec<Entity> = track.squares().chain([deeds.cancel_card()]).filter(|&c| deeds.owner(c) == Some(p)).collect();
        cards.sort_by_key(|&c| match deeds.square(c) {
            Some(Field::Horse(_)) => 0,
            Some(Field::Trainer(_)) => 1,
            Some(_) => 2,
            None => 3,
        });
        cards
    };
    let at = players.get(turn.player).map(|p| p.4.0).unwrap();
    let view = View {
        owned: list.iter().map(|p| in_order(p.entity)).collect(),
        at,
        for_sale: for_sale(&phase, before_roll.is_some(), at, &deeds),
        offers: offers
            .iter()
            .filter(|(_, TradeOffer(t), by)| trade_possible(t, by.0, &deeds))
            .map(|(e, TradeOffer(t), by)| (e, by.0, t, trade_affordable(t, money_of(by.0), money_of(t.partner))))
            .collect(),
        players: list,
        me,
        phase: &phase,
        eliminated: &eliminated.0,
        trade_ok,
        log: &log.0,
    };

    commands
        .spawn((PanelRoot, BoardItem, Node { width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }))
        .with_children(|screen| {
            // The design-sized frame lines up with the world-space board.
            screen.spawn(Node { width: Val::Px(DESIGN.x), height: Val::Px(DESIGN.y), ..default() }).with_children(|frame| {
                // Everything at a glance: players with their property, the turn, trading, the log.
                frame
                    .spawn((
                        PanelColumn,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(900.0),
                            top: Val::Px(12.0),
                            width: Val::Px(488.0),
                            height: Val::Px(818.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(8.0),
                            // A fallback: rather scrolled than cut off.
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                        RelativeCursorPosition::default(),
                        ScrollPosition(Vec2::new(0.0, scrolled.0)),
                    ))
                    .with_children(|p| {
                        players_plank(p, &view, ui.draft.as_ref().filter(|_| view.can_trade()), &deeds, &art);
                        turn_plank(p, &view, &deeds, &art, ui.draft.is_some());
                        offers_plank(p, &view, &deeds, &art);
                        // The whole history in the space left, newest at the bottom (where it opens).
                        let (node, background, border) = plank();
                        p.spawn((
                            Node { flex_grow: 1.0, min_height: Val::Px(60.0), overflow: Overflow::scroll_y(), row_gap: Val::Px(2.0), ..node },
                            background,
                            border,
                            Scrolls,
                            RelativeCursorPosition::default(),
                            ScrollPosition(Vec2::new(0.0, f32::MAX)),
                        ))
                        .with_children(|c| {
                            for line in view.log {
                                c.spawn(art.text(line.clone(), 13.0, MUTED));
                            }
                        });
                    });
                // Ending early is out of the way and asks first.
                if !matches!(*phase, Phase::GameOver { .. }) {
                    frame
                        .spawn(Node { position_type: PositionType::Absolute, left: Val::Px(900.0), top: Val::Px(838.0), ..row(8.0) })
                        .with_children(|r| finish_early(r, &art, ui.confirm_finish));
                }
                // Over the middle of the board: the trade being made or answered,
                // else the card of the chosen square.
                let over_board = Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(DESIGN.x / 2.0 + geometry::CENTER.x - geometry::FRAME),
                    top: Val::Px(DESIGN.y / 2.0 - geometry::FRAME),
                    width: Val::Px(2.0 * geometry::FRAME),
                    height: Val::Px(2.0 * geometry::FRAME),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(6.0),
                    overflow: Overflow::clip(),
                    ..default()
                };
                if let Some(draft) = ui.draft.as_ref().filter(|_| view.can_trade()) {
                    frame.spawn((BoardOverlay, over_board)).with_children(|o| trade_dialog(o, &view, draft, &deeds, &art));
                } else if let Some(sq) = ui.selected {
                    let owner = deeds.owner(sq).and_then(|o| view.player(o)).map(|r| r.player);
                    frame
                        .spawn((BoardOverlay, over_board))
                        .with_children(|o| {
                            square_card(o, &art, deeds.field(sq), deeds.races(sq), owner);
                            o.spawn(row(6.0)).with_children(|r| {
                                if view.can_sell(sq, &deeds) {
                                    let price = deeds.field(sq).price().unwrap_or_default();
                                    let label = format!("Prodat bance za {}", super::money(price));
                                    button(r, &art, label, deeds.can_sell_to_bank(turn.player, sq).then_some(Click::SellToBank(sq)), ButtonStyle::Normal);
                                    if deeds.races(sq) > Races::None {
                                        button(r, &art, "Vrátit dostih", Some(Click::SellRace(sq)), ButtonStyle::Normal);
                                    }
                                }
                                button(r, &art, "Zavřít", Some(Click::CloseCard), ButtonStyle::Normal);
                            });
                        });
                }
            });
        });
}

/// A plank that keeps its size when the panel gets crowded.
fn fixed((node, background, border): (Node, BackgroundColor, BorderColor)) -> (Node, BackgroundColor, BorderColor) {
    (Node { flex_shrink: 0.0, ..node }, background, border)
}

fn players_plank(p: &mut Parent, v: &View, draft: Option<&Draft>, deeds: &Deeds, art: &Art) {
    p.spawn(fixed(plank())).with_children(|c| {
        for (k, &Row { player, money, status, ai, .. }) in v.players.iter().enumerate() {
            let on_turn = k == v.me;
            // One row a player: pawn; name, money and banknotes; the deeds, overlapping
            // like a hand of cards when there are many.
            c.spawn(Node { flex_wrap: FlexWrap::NoWrap, align_items: AlignItems::Start, ..row(6.0) }).with_children(|r| {
                r.spawn((Node { width: Val::Px(20.0), height: Val::Px(20.0), flex_shrink: 0.0, ..default() }, ImageNode::new(art.pawn.clone()).with_color(player.color)));
                r.spawn(Node { width: Val::Px(CASH_WIDTH), flex_shrink: 0.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() }).with_children(|col| {
                    col.spawn(row(6.0)).with_children(|line| {
                        let name = if ai { format!("{} (PC)", player.name) } else { player.name.clone() };
                        line.spawn(art.bold(name, 15.0, if on_turn { GOLD } else { TEXT }));
                        line.spawn(art.bold(super::money(money), 15.0, if money < 0 { DANGER } else { TEXT }));
                        let tag = match status {
                            Status::Free => None,
                            Status::Distanc => Some("DISTANC".to_string()),
                            Status::Sitting { turns, doping: true } => Some(format!("DOPING ({turns})")),
                            Status::Sitting { turns, doping: false } => Some(format!("STOJÍ ({turns})")),
                        };
                        if let Some(t) = tag {
                            line.spawn(art.bold(t, 12.0, DANGER));
                        }
                    });
                    if money > 0 {
                        cash(col, art, money);
                    }
                });
                let owned = &v.owned[k];
                let cards = owned.len();
                if cards == 0 {
                    return;
                }
                // One card, then a step per further card, all within the hand's width.
                let step = if cards > 1 { ((HAND_WIDTH - DEED.x) / (cards - 1) as f32).min(DEED.x + 3.0) } else { 0.0 };
                r.spawn((Hand { step }, RelativeCursorPosition::default(), Node { flex_wrap: FlexWrap::NoWrap, ..row(3.0) })).with_children(|r| {
                    for (j, &sq) in owned.iter().enumerate() {
                        let overlap = if j == 0 { 0.0 } else { DEED.x + 3.0 - step };
                        // The Zrušen distanc card is played by choice, while waiting in Distanc before the roll.
                        let usable = k == v.me && !ai && status == Status::Distanc && matches!(v.phase, Phase::PreRoll);
                        let card = match deeds.square(sq) {
                            Some(field) => (MiniCard::deed(art, field, deeds.races(sq)), Some(Click::Select(sq))),
                            None => (MiniCard::cancel_distanc(art), usable.then_some(Click::UseCancelCard)),
                        };
                        // While trading, the cards of both sides go in and out of the deal.
                        let (click, picked) = match draft {
                            Some(d) if d.partner.is_some() && (k == v.me || Some(v.players[k].entity) == d.partner) => {
                                (Some(Click::DraftToggle(sq)), d.give.contains(&sq) || d.take.contains(&sq))
                            }
                            _ => (card.1, card.1 == Some(Click::UseCancelCard)),
                        };
                        card.0.spawn(r, art, sq, click, picked, overlap);
                    }
                });
            });
        }
        for player in v.eliminated {
            c.spawn(row(8.0)).with_children(|r| {
                r.spawn((Node { width: Val::Px(24.0), height: Val::Px(24.0), ..default() }, ImageNode::new(art.pawn.clone()).with_color(player.color.with_alpha(0.3))));
                r.spawn(art.text(format!("{} – zbankrotoval", player.name), 14.0, MUTED));
            });
        }
    });
}

fn turn_plank(p: &mut Parent, v: &View, deeds: &Deeds, art: &Art, trading: bool) {
    p.spawn(fixed(plank())).with_children(|c| {
        let Row { entity, player: me, money, status, ai } = v.me();
        if let Phase::GameOver { winner } = v.phase {
            let text = winner.and_then(|w| v.player(w)).map_or("Konec hry.".to_string(), |w| format!("Vítězem je {}!", w.player.name));
            c.spawn(art.bold(text, 26.0, GOLD));
            button(c, art, "Nová hra", Some(Click::NewGameScreen), ButtonStyle::Primary);
            return;
        }
        if ai {
            c.spawn(art.text(format!("{} (počítač) hraje…", me.name), 17.0, MUTED));
            return;
        }
        c.spawn(art.bold(format!("Na tahu: {}", me.name), 18.0, GOLD));
        // Bought with a click, or left alone.
        if let Some(sq) = v.for_sale {
            let price = deeds.field(sq).price().unwrap_or_default();
            let label = format!("Koupit {} za {}", deeds.field(sq).name(), super::money(price));
            button(c, art, label, (money >= price).then_some(Click::Buy), ButtonStyle::Primary);
        }
        match v.phase {
            Phase::PreRoll => {
                c.spawn(row(8.0)).with_children(|r| {
                    button(r, art, "Hodit kostkou", Some(Click::Roll), ButtonStyle::Primary);
                    if status == Status::Distanc && deeds.holds_cancel_card(entity) {
                        button(r, art, "Použít Zrušen distanc", Some(Click::UseCancelCard), ButtonStyle::Normal);
                    }
                });
                if status == Status::Distanc {
                    c.spawn(art.text("Jsi na distancu – máš tři pokusy hodit 6.", 15.0, TEXT));
                }
            }
            Phase::AfterMove { .. } => {
                if let Some(horse) = race_horse(v.phase, v.me().entity, v.at, deeds) {
                    let (races, cost) = (deeds.races(horse), deeds.field(horse).stable().map_or(0, Stable::race_cost));
                    c.spawn(art.bold(format!("Dostihy pro koně {}:", deeds.field(horse).name()), 15.0, TEXT));
                    c.spawn(row(6.0)).with_children(|r| {
                        for n in races.buyable() {
                            let total = cost * n as i64;
                            let label = if races == Races::Four { format!("Hlavní dostih {}", super::money(total)) } else { format!("+{n} {}", super::money(total)) };
                            button(r, art, label, (money >= total).then_some(Click::BuyRaces(n)), ButtonStyle::Primary);
                        }
                    });
                }
                if money < 0 {
                    c.spawn(art.bold(format!("Dluh {}! Prodej majetek – klikni na své pole na desce.", super::money(-money)), 15.0, DANGER));
                    button(c, art, "Doprodat automaticky", Some(Click::Liquidate), ButtonStyle::Danger);
                }
                c.spawn(row(8.0)).with_children(|r| {
                    button(r, art, "Ukončit tah", v.phase.can_end_turn(money).then_some(Click::EndTurn), ButtonStyle::Primary);
                    if !trading {
                        button(r, art, "Obchodovat s hráči…", Some(Click::Trade(true)), ButtonStyle::Normal);
                    }
                });
            }
            Phase::GameOver { .. } => {}
        }
    });
}

/// A small button for ending the game now; the richest player wins.
fn finish_early(r: &mut Parent, art: &Art, confirm: bool) {
    if confirm {
        r.spawn(art.bold("Opravdu ukončit hru a vyhodnotit majetek?", 13.0, TEXT));
        button(r, art, "Ano, vyhodnotit", Some(Click::FinishGame), ButtonStyle::Danger);
        button(r, art, "Ne", Some(Click::ConfirmFinish(false)), ButtonStyle::Normal);
    } else {
        r.spawn((Button, Click::ConfirmFinish(true))).with_children(|b| {
            b.spawn(art.text("Hrajete na čas? Vyhodnotit hru…", 13.0, MUTED));
        });
    }
}

/// A small card in a player's hand: a square's deed or the kept Náhoda card.
struct MiniCard {
    header: Color,
    picture: ImageNode,
    /// The picture is a photo (bigger) rather than an icon.
    photo: bool,
    name: String,
    tokens: Vec<Handle<Image>>,
}

impl MiniCard {
    /// A square like the deeds in front of you at the table: stable colour,
    /// photo or icon, name and race tokens.
    fn deed(art: &Art, field: &Field, races: Races) -> MiniCard {
        let (picture, photo) = match field {
            Field::Horse(h) => (ImageNode::new(art.photo(h)), true),
            f => (ImageNode::from_atlas_image(art.icons.clone(), art.icon(f).unwrap()), false),
        };
        let tokens = match races {
            Races::Main => vec![art.main_race.clone()],
            r => vec![art.race.clone(); r.level()],
        };
        MiniCard { header: field.stable().map_or(WOOD_DARK, Stable::color), picture, photo, name: field.name(), tokens }
    }

    fn cancel_distanc(art: &Art) -> MiniCard {
        let picture = ImageNode::from_atlas_image(art.icons.clone(), art.icon(&Field::Nahoda).unwrap());
        MiniCard { header: Color::srgb(0.72, 0.18, 0.14), picture, photo: false, name: "Zrušen distanc".into(), tokens: Vec::new() }
    }

    /// `picked` lifts the card with a gold edge; `overlap` tucks it under the previous one.
    fn spawn(self, r: &mut Parent, art: &Art, of: Entity, click: Option<Click>, picked: bool, overlap: f32) {
        let mut card = r.spawn((
            HandCard { of, lift: 0.0, shift: 0.0, tilt: 0.0 },
            UiTransform::default(),
            ZIndex::default(),
            // Each card casts a shadow on the one under it.
            BoxShadow::new(Color::BLACK.with_alpha(0.35), Val::Px(-2.0), Val::Px(1.0), Val::ZERO, Val::Px(4.0)),
            Node {
                width: Val::Px(DEED.x),
                height: Val::Px(DEED.y),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                margin: UiRect { left: Val::Px(-overlap), top: Val::Px(if picked { -6.0 } else { 0.0 }), ..default() },
                border: UiRect::all(Val::Px(if picked { 3.0 } else { 1.0 })),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(CREAM),
            BorderColor::all(if picked { GOLD } else { INK }),
        ));
        if let Some(click) = click {
            // Else a click would press all the cards stacked under this one.
            card.insert((Button, bevy::ui::FocusPolicy::Block, click));
        }
        card.with_children(|c| {
            c.spawn((Node { width: Val::Percent(100.0), height: Val::Px(6.0), ..default() }, BackgroundColor(self.header)));
            let size = if self.photo { Vec2::new(38.0, 36.0) } else { Vec2::splat(26.0) };
            let margin = UiRect::all(Val::Px(if self.photo { 0.0 } else { 5.0 }));
            c.spawn((Node { width: Val::Px(size.x), height: Val::Px(size.y), margin, ..default() }, self.picture));
            c.spawn((art.bold(self.name, 8.0, INK), TextLayout { linebreak: LineBreak::NoWrap, ..default() }));
            c.spawn(Node { column_gap: Val::Px(1.0), ..default() }).with_children(|t| {
                for image in self.tokens {
                    t.spawn((Node { width: Val::Px(9.0), height: Val::Px(9.0), ..default() }, ImageNode::new(image)));
                }
            });
        });
    }
}

/// A player's cards in a row, overlapping by `step` when there are many.
#[derive(Component)]
struct Hand {
    step: f32,
}

/// A card in a hand, eased towards where the hovered card puts it.
#[derive(Component)]
struct HandCard {
    /// The square, or the Zrušen distanc card.
    of: Entity,
    /// 1 for the hovered card, which rises over the others.
    lift: f32,
    /// Sideways, away from the hovered card.
    shift: f32,
    tilt: f32,
}

/// The card under the cursor rises and grows over the stack while its
/// neighbours lean aside, like a hand of cards held up. Which card that is
/// follows from the cursor over the hand's resting layout, so the moving
/// cards never fight over the hover.
fn fan_hands(
    time: Res<Time>,
    hands: Query<(&Hand, &RelativeCursorPosition, &Children)>,
    mut cards: Query<(&mut HandCard, &mut UiTransform, &mut ZIndex, &mut BoxShadow)>,
    mut hovered_before: Local<Option<Entity>>,
) {
    let ease = 1.0 - (-14.0 * time.delta_secs()).exp();
    for (hand, cursor, children) in &hands {
        let n = children.len();
        let width = DEED.x + hand.step * (n.max(1) - 1) as f32;
        let hovered = cursor
            .normalized
            .filter(|_| cursor.cursor_over())
            .map(|p| if hand.step > 0.0 { (((p.x + 0.5) * width / hand.step) as usize).min(n - 1) } else { 0 });
        for (j, child) in children.iter().enumerate() {
            let Ok((mut card, mut transform, mut z, mut shadow)) = cards.get_mut(child) else { continue };
            let (lift, side) = match hovered {
                Some(h) if h == j => (1.0, 0.0),
                Some(h) => (0.0, (j as f32 - h as f32).signum() / (j as f32 - h as f32).abs().sqrt()),
                None => (0.0, 0.0),
            };
            // A panel rebuilt under the cursor keeps its hovered card up.
            let k = if hovered.is_some() && *hovered_before == Some(card.of) && card.is_added() { 1.0 } else { ease };
            card.lift += (lift - card.lift) * k;
            card.shift += (side * if side > 0.0 { 6.0 } else { 3.0 } - card.shift) * k;
            card.tilt += (side * 0.06 - card.tilt) * k;
            *transform = UiTransform {
                translation: Val2::px(card.shift, -4.0 * card.lift),
                scale: Vec2::splat(1.0 + 0.22 * card.lift),
                rotation: Rot2::radians(card.tilt),
            };
            *z = ZIndex(if card.lift > 0.02 { 1 } else { 0 });
            if let Some(s) = shadow.0.first_mut() {
                s.blur_radius = Val::Px(4.0 + 8.0 * card.lift);
                s.y_offset = Val::Px(1.0 + 4.0 * card.lift);
            }
            if hovered == Some(j) {
                *hovered_before = Some(card.of);
            }
        }
    }
}

/// Size of a deed card in the players' list.
const DEED: Vec2 = Vec2::new(44.0, 62.0);
/// Widths of a player's banknotes and hand of cards.
const CASH_WIDTH: f32 = 196.0;
const HAND_WIDTH: f32 = 226.0;

/// The trade being put together, over the middle of the board.
fn trade_dialog(p: &mut Parent, v: &View, d: &Draft, deeds: &Deeds, art: &Art) {
    let (node, _, _) = plank();
    p.spawn((Node { width: Val::Px(460.0), ..node }, BackgroundColor(WOOD_DARK), BorderColor::all(GOLD))).with_children(|c| draft(c, v, d, deeds, art));
}

/// Standing offers, one line each: the partner may accept, the proposer
/// withdraw. Computers deal with theirs themselves.
fn offers_plank(p: &mut Parent, v: &View, deeds: &Deeds, art: &Art) {
    if v.offers.is_empty() {
        return;
    }
    let (node, background, border) = plank();
    // Shares the space left with the history, keeping room for three offers.
    let node = Node { flex_shrink: 1.0, min_height: Val::Px(110.0), overflow: Overflow::scroll_y(), ..node };
    p.spawn((node, background, border, Scrolls, RelativeCursorPosition::default())).with_children(|c| {
        // The count tells when some are scrolled out of view.
        c.spawn(art.bold(format!("Nabídky obchodu ({})", v.offers.len()), 14.0, GOLD));
        for &(offer, proposer, t, affordable) in &v.offers {
            let (Some(a), Some(b)) = (v.player(proposer), v.player(t.partner)) else { continue };
            c.spawn(Node { justify_content: JustifyContent::SpaceBetween, width: Val::Percent(100.0), flex_wrap: FlexWrap::NoWrap, ..row(6.0) }).with_children(|r| {
                r.spawn(Node { flex_shrink: 1.0, ..default() }).with_children(|n| {
                    n.spawn(art.text(format!("{} pro {}: {}", a.player.name, b.player.name, describe_trade(t, deeds)), 13.0, TEXT));
                });
                r.spawn(Node { flex_shrink: 0.0, ..row(4.0) }).with_children(|r| {
                    if !b.ai {
                        // Greyed out while whoever pays is short of money.
                        button(r, art, "Přijmout", affordable.then_some(Click::AcceptTrade(offer)), ButtonStyle::Normal);
                    }
                    if !a.ai {
                        button(r, art, "Stáhnout", Some(Click::CancelTrade(offer)), ButtonStyle::Normal);
                    }
                });
            });
        }
    });
}

/// « and » buttons moving the price towards one side or the other.
fn price_arrows(c: &mut Parent, art: &Art, click: fn(i64) -> Click, left_pays: &str, right_pays: &str) {
    c.spawn(Node { flex_wrap: FlexWrap::NoWrap, ..row(6.0) }).with_children(|r| {
        for (delta, label) in [(-1000, "« 1000"), (-100, "« 100"), (100, "100 »"), (1000, "1000 »")] {
            button(r, art, label, Some(click(delta)), ButtonStyle::Normal);
        }
    });
    c.spawn(art.text(format!("« víc platí {left_pays}, » víc platí {right_pays}"), 12.0, MUTED));
}

fn draft(c: &mut Parent, v: &View, d: &Draft, deeds: &Deeds, art: &Art) {
    let me = v.me().entity;
    c.spawn(row(6.0)).with_children(|r| {
        r.spawn(art.bold("Obchod – s kým?", 16.0, GOLD));
        for other in v.players.iter().filter(|p| p.entity != me) {
            let style = if d.partner == Some(other.entity) { ButtonStyle::Selected } else { ButtonStyle::Normal };
            button(r, art, other.player.name.clone(), Some(Click::DraftPartner(other.entity)), style);
        }
    });
    if let Some(partner) = d.partner.and_then(|p| v.player(p)) {
        let b = partner.player.name.as_str();
        c.spawn(art.text(format!("Klikni na karty v panelu – své, které dáš, a hráče {b}, které chceš."), 14.0, TEXT));
        c.spawn(art.bold(format!("Dáváš: {}", trade_side(&d.give, d.money, deeds)), 14.0, TEXT));
        c.spawn(art.bold(format!("Dostaneš: {}", trade_side(&d.take, -d.money, deeds)), 14.0, TEXT));
        let money = match d.money {
            0 => "bez doplatku".to_string(),
            m if m > 0 => format!("doplácíš {}", super::money(m)),
            m => format!("{b} doplácí {}", super::money(-m)),
        };
        c.spawn(art.bold(format!("Peníze: {money}"), 15.0, GOLD));
        price_arrows(c, art, Click::DraftMoney, b, "ty");
        c.spawn(art.text("Stáj s dostihy se prodává jen celá.", 12.0, MUTED));
    }
    c.spawn(row(8.0)).with_children(|r| {
        button(r, art, "Navrhnout obchod", v.trade_ok.then_some(Click::ProposeTrade), ButtonStyle::Primary);
        button(r, art, "Zavřít", Some(Click::Trade(false)), ButtonStyle::Normal);
    });
}
