//! New-game screen: pick who plays which pawn.

use super::board::despawn;
use super::widgets::*;
use super::*;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<SetupState>()
        .add_systems(OnEnter(AppState::Setup), |mut s: ResMut<SetupState>| s.set_changed())
        .add_systems(OnExit(AppState::Setup), despawn::<SetupRoot>)
        .add_systems(Update, draw.run_if(resource_changed::<SetupState>).run_if(in_state(AppState::Setup)));
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Seat {
    Empty,
    Human,
    Computer,
}

#[derive(Resource)]
pub struct SetupState {
    seats: [Seat; PAWNS.len()],
    pub rules: HouseRules,
}

impl Default for SetupState {
    fn default() -> Self {
        let mut seats = [Seat::Empty; PAWNS.len()];
        seats[0] = Seat::Human;
        seats[1] = Seat::Computer;
        SetupState { seats, rules: HouseRules { parking_pot: true, buy_before_roll: false } }
    }
}

impl SetupState {
    pub fn cycle(&mut self, k: usize) {
        self.seats[k] = match self.seats[k] {
            Seat::Empty => Seat::Human,
            Seat::Human => Seat::Computer,
            Seat::Computer => Seat::Empty,
        };
    }

    pub fn players(&self) -> Vec<PlayerSpec> {
        PAWNS
            .iter()
            .zip(self.seats)
            .filter(|(_, seat)| *seat != Seat::Empty)
            .map(|(&(name, color), seat)| PlayerSpec { name: name.into(), color, ai: seat == Seat::Computer })
            .collect()
    }
}

#[derive(Component)]
struct SetupRoot;

fn draw(mut commands: Commands, roots: Query<Entity, With<SetupRoot>>, art: Res<Art>, original: Res<crate::original::Original>, assets: Res<AssetServer>, setup: Res<SetupState>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    let art = &*art;
    commands
        .spawn((
            SetupRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(FELT_DARK),
        ))
        .with_children(|root| {
            root.spawn(art.bold("DOSTIHY & SÁZKY", 64.0, GOLD));
            root.spawn(line(6.0)).with_children(|r| {
                for horse in BOARD.iter().filter_map(|f| if let Field::Horse(h) = f { Some(h) } else { None }).step_by(3) {
                    r.spawn((Node { width: Val::Px(68.0), height: Val::Px(66.0), ..default() }, ImageNode::new(art.photo(horse))));
                }
            });
            root.spawn(art.text("Klikni u každé figurky, kdo s ní bude hrát.", 20.0, TEXT));
            root.spawn(plank()).with_children(|c| {
                for (k, (&(name, color), seat)) in PAWNS.iter().zip(setup.seats).enumerate() {
                    c.spawn(line(14.0)).with_children(|r| {
                        r.spawn((Node { width: Val::Px(30.0), height: Val::Px(30.0), ..default() }, ImageNode::new(art.pawn.clone()).with_color(color)));
                        r.spawn(Node { width: Val::Px(110.0), ..default() }).with_children(|n| {
                            n.spawn(art.bold(name, 20.0, TEXT));
                        });
                        let (label, style) = match seat {
                            Seat::Empty => ("— nehraje —", ButtonStyle::Normal),
                            Seat::Human => ("Člověk", ButtonStyle::Selected),
                            Seat::Computer => ("Počítač", ButtonStyle::Primary),
                        };
                        button(r, art, label, Some(Click::SetupSlot(k)), style);
                    });
                }
            });
            root.spawn(art.bold("Domácí pravidla", 18.0, GOLD));
            let rules = [
                ("Pokuty se platí na parkoviště, kdo tam stoupne, bere je.", setup.rules.parking_pot, Click::ToggleParkingPot),
                ("Volné pole, na kterém stojíš, můžeš koupit i před hodem.", setup.rules.buy_before_roll, Click::ToggleBuyBeforeRoll),
            ];
            for (text, on, click) in rules {
                root.spawn(line(10.0)).with_children(|r| {
                    r.spawn(art.text(text, 16.0, TEXT));
                    let (label, style) = if on { ("Ano", ButtonStyle::Selected) } else { ("Ne", ButtonStyle::Normal) };
                    button(r, art, label, Some(click), style);
                });
            }
            let enough = setup.players().len() >= 2;
            button(root, art, "Začít hru", enough.then_some(Click::StartGame), ButtonStyle::Primary);
            if !enough {
                root.spawn(art.text("Hrají alespoň dva hráči.", 16.0, MUTED));
            }
            // The loading screen waited for it.
            if original.missing(&assets) {
                root.spawn(art.text("Grafiku z původní hry se nepodařilo stáhnout – hraje se bez ní.", 15.0, DANGER));
            }
        });
}

/// A row that stays on one line. The screen has room for everything, and a
/// wrapping row sized to its content still wrapped its last item, overlapping
/// what came below.
fn line(gap: f32) -> Node {
    Node { flex_wrap: FlexWrap::NoWrap, ..row(gap) }
}
