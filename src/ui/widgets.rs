//! Reusable UI pieces: text, buttons, banknotes and cards.

use super::*;
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::text::{FontSize, FontWeight};

pub type Parent<'a> = ChildSpawnerCommands<'a>;

impl Art {
    pub fn text(&self, s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
        (Text::new(s), self.font(FontWeight::NORMAL, size), TextColor(color))
    }

    pub fn bold(&self, s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
        (Text::new(s), self.font(FontWeight::BOLD, size), TextColor(color))
    }

    /// Bold text placed in the world, on the board.
    pub fn label(&self, s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
        (Text2d::new(s), self.font(FontWeight::BOLD, size), TextColor(color))
    }

    fn font(&self, weight: FontWeight, size: f32) -> TextFont {
        TextFont { font: self.font.clone(), font_size: FontSize::Px(size), weight, ..default() }
    }
}

pub fn row(gap: f32) -> Node {
    Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(gap), row_gap: Val::Px(gap), align_items: AlignItems::Center, ..default() }
}

/// A wooden panel section.
pub fn plank() -> (Node, BackgroundColor, BorderColor) {
    (
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(WOOD),
        BorderColor::all(WOOD_LIGHT),
    )
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum ButtonStyle {
    Primary,
    Normal,
    Selected,
    Danger,
}

impl ButtonStyle {
    fn colors(self, enabled: bool) -> (Color, Color) {
        if !enabled {
            return (Color::srgb(0.4, 0.3, 0.22), Color::srgb(0.4, 0.3, 0.22));
        }
        match self {
            ButtonStyle::Primary => (GOLD, Color::srgb(1.0, 0.9, 0.45)),
            ButtonStyle::Normal => (WOOD_DARK, Color::srgb(0.45, 0.25, 0.1)),
            ButtonStyle::Selected => (Color::srgb(0.2, 0.45, 0.25), Color::srgb(0.28, 0.58, 0.33)),
            ButtonStyle::Danger => (DANGER, Color::srgb(1.0, 0.4, 0.35)),
        }
    }
}

/// A button; `None` as the action renders it disabled.
pub fn button(p: &mut Parent, art: &Art, label: impl Into<String>, action: Option<Click>, style: ButtonStyle) {
    let enabled = action.is_some();
    let text = match (enabled, style) {
        (false, _) => MUTED,
        (true, ButtonStyle::Primary) => INK,
        _ => TEXT,
    };
    let mut b = p.spawn((
        Node {
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(style.colors(enabled).0),
        BorderColor::all(WOOD_DARK),
    ));
    if let Some(action) = action {
        // Spawned as a plain node, it got `FocusPolicy::Pass`: a button must
        // keep the click from reaching what lies under it.
        b.insert((Button, bevy::ui::FocusPolicy::Block, action, style));
    }
    b.with_children(|b| {
        b.spawn(art.bold(label, 15.0, text));
    });
}

pub fn hover_buttons(mut q: Query<(&Interaction, &ButtonStyle, &mut BackgroundColor), Changed<Interaction>>) {
    for (interaction, style, mut bg) in &mut q {
        let (normal, hover) = style.colors(true);
        bg.0 = if *interaction == Interaction::None { normal } else { hover };
    }
}

/// Text colour readable on a square's colour.
pub fn ink_on(field: &Field) -> Color {
    match field.stable() {
        Some(Stable::Brown | Stable::Pink | Stable::DarkGreen | Stable::Purple) => Color::WHITE,
        _ => INK,
    }
}

pub fn bill_color(value: i64) -> Color {
    BILLS.iter().find(|(v, _)| *v <= value.max(20)).map_or(BILLS[6].1, |b| b.1)
}

/// A small banknote.
pub fn banknote(p: &mut Parent, art: &Art, value: i64) {
    p.spawn((
        Node {
            width: Val::Px(32.0),
            height: Val::Px(17.0),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(bill_color(value)),
        BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.5)),
    ))
    .with_children(|b| {
        b.spawn(art.bold(if value >= 1000 { format!("{}k", value / 1000) } else { value.to_string() }, 10.0, INK));
    });
}

/// Cash as a pile of banknotes, largest first.
pub fn cash(p: &mut Parent, art: &Art, amount: i64) {
    p.spawn(row(3.0)).with_children(|r| {
        let mut rest = amount.max(0);
        for (value, _) in BILLS {
            let n = rest / value;
            rest %= value;
            if n > 0 {
                // A note and its count stay on one line.
                r.spawn(Node { flex_wrap: FlexWrap::NoWrap, ..row(2.0) }).with_children(|pair| {
                    banknote(pair, art, value);
                    if n > 1 {
                        pair.spawn(art.bold(format!("×{n}"), 12.0, TEXT));
                    }
                });
            }
        }
    });
}

/// The card of a square, like the deeds of the board game: horses show photo and fee table.
pub fn square_card(p: &mut Parent, art: &Art, field: &Field, races: Races, owner: Option<&Player>) {
    p.spawn((
        Node {
            width: Val::Px(240.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(3.0),
            padding: UiRect::all(Val::Px(8.0)),
            border: UiRect::all(Val::Px(3.0)),
            border_radius: BorderRadius::all(Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(CREAM),
        BorderColor::all(INK),
    ))
    .with_children(|c| {
        let header = field.stable().map_or(WOOD_DARK, Stable::color);
        c.spawn((
            Node { width: Val::Percent(100.0), justify_content: JustifyContent::Center, padding: UiRect::all(Val::Px(4.0)), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() },
            BackgroundColor(header),
        ))
        .with_children(|h| {
            h.spawn(art.bold(field.name(), 18.0, if field.stable().is_some() { ink_on(field) } else { Color::WHITE }));
        });
        match field {
            Field::Horse(horse) => {
                c.spawn((Node { width: Val::Px(135.0), height: Val::Px(130.0), ..default() }, ImageNode::new(art.photo(horse))));
                c.spawn(art.bold(format!("Cena {}", money(horse.price)), 15.0, INK));
                let labels = ["Prohlídka stáje", "1 dostih", "2 dostihy", "3 dostihy", "4 dostihy", "Hlavní dostih"];
                for (level, (label, fee)) in labels.iter().zip(horse.fees).enumerate() {
                    let current = level == races.level() && owner.is_some();
                    c.spawn((
                        Node { width: Val::Percent(100.0), justify_content: JustifyContent::SpaceBetween, padding: UiRect::horizontal(Val::Px(4.0)), ..default() },
                        BackgroundColor(if current { GOLD.with_alpha(0.8) } else { Color::NONE }),
                    ))
                    .with_children(|r| {
                        r.spawn(art.text(*label, 13.0, INK));
                        r.spawn(art.bold(money(fee), 13.0, INK));
                    });
                }
                c.spawn(art.text(format!("Každý dostih stojí {}", money(horse.stable.race_cost())), 12.0, INK));
            }
            _ => {
                if let Some(icon) = art.icon(field) {
                    c.spawn((Node { width: Val::Px(64.0), height: Val::Px(64.0), ..default() }, ImageNode::from_atlas_image(art.icons.clone(), icon)));
                }
                if let Some(price) = field.price() {
                    c.spawn(art.bold(format!("Cena {}", money(price)), 15.0, INK));
                }
                c.spawn((art.text(describe(field), 13.0, INK), TextLayout { justify: Justify::Center, ..default() }));
            }
        }
        let owner_text = owner.map_or("Volné – patří bance".to_string(), |o| format!("Majitel: {}", o.name));
        c.spawn(art.bold(owner_text, 13.0, owner.map_or(INK, |o| o.color.darker(0.15))));
    });
}

fn describe(field: &Field) -> String {
    match field {
        Field::Trainer(_) => format!("Majitel dostane {} za každou svou trenérskou licenci.", money(TRAINER_FEE)),
        Field::Service(_) => format!("Majitel dostane {SERVICE_FEE}× hod kostkou, se Stájemi i Přepravou {BOTH_SERVICES_FEE}×."),
        Field::Distanc => "Každý tah máš tři pokusy hodit šestku; tvoji koně mezitím nevydělávají na dostizích.".into(),
        Field::Doping => "Jeden tah nehraješ a tvoji koně nevydělávají na dostizích.".into(),
        Field::Finance | Field::Nahoda => "Táhneš kartu a řídíš se jí.".into(),
        Field::Vet(fee) => format!("Zaplatíš bance {}.", money(*fee)),
        Field::Start => format!("Při průchodu dostaneš {}, když na něm zastavíš hodem kostkou, ještě {} navíc.", money(START_BONUS), money(START_LANDING_BONUS)),
        Field::Parking => "Neutrální pole.".into(),
        Field::Horse(_) => String::new(),
    }
}
