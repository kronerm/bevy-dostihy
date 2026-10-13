//! Static game data: the board layout and the Finance / Náhoda decks.
//! The rules spawn the board from this; they never index into it.

use bevy::color::Color;

pub const START_MONEY: i64 = 30_000;
pub const START_BONUS: i64 = 4000;
/// Paid on top of the start bonus for a dice move stopping right on Start.
pub const START_LANDING_BONUS: i64 = 1000;
/// Throws per turn to get a 6 and leave Distanc.
pub const DISTANC_TRIES: usize = 3;
pub const TRAINER_PRICE: i64 = 4000;
pub const SERVICE_PRICE: i64 = 3000;
/// A trainer's fee for each trainer its owner has.
pub const TRAINER_FEE: i64 = 1000;
/// A service's fee times the dice; more when one owns both services.
pub const SERVICE_FEE: i64 = 80;
pub const BOTH_SERVICES_FEE: i64 = 200;

/// Banknotes of the board game with their colours, highest first.
pub const BILLS: [(i64, Color); 7] = [
    (10_000, Color::srgb_u8(120, 146, 176)),
    (2000, Color::srgb_u8(118, 166, 230)),
    (1000, Color::srgb_u8(150, 206, 240)),
    (400, Color::srgb_u8(204, 204, 90)),
    (200, Color::srgb_u8(150, 200, 90)),
    (100, Color::srgb_u8(240, 146, 70)),
    (20, Color::srgb_u8(230, 124, 186)),
];

/// Pawn colours offered at game setup.
pub const PAWNS: [(&str, Color); 6] = [
    ("Červený", Color::srgb_u8(214, 40, 40)),
    ("Žlutý", Color::srgb_u8(240, 196, 25)),
    ("Zelený", Color::srgb_u8(46, 160, 67)),
    ("Modrý", Color::srgb_u8(40, 90, 220)),
    ("Fialový", Color::srgb_u8(150, 70, 200)),
    ("Bílý", Color::srgb_u8(235, 235, 235)),
];

/// The eight colour groups of horses, in board order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stable {
    Orange,
    Brown,
    LightBlue,
    Green,
    Pink,
    Yellow,
    DarkGreen,
    Purple,
}

impl Stable {
    /// Cost of entering one race – and of the main race – for a horse of this stable.
    pub fn race_cost(self) -> i64 {
        match self {
            Stable::Orange | Stable::Brown => 1000,
            Stable::LightBlue | Stable::Green => 2000,
            Stable::Pink | Stable::Yellow => 3000,
            Stable::DarkGreen | Stable::Purple => 4000,
        }
    }

    pub fn color(self) -> Color {
        match self {
            Stable::Orange => Color::srgb_u8(222, 92, 38),
            Stable::Brown => Color::srgb_u8(140, 78, 36),
            Stable::LightBlue => Color::srgb_u8(54, 168, 226),
            Stable::Green => Color::srgb_u8(132, 186, 52),
            Stable::Pink => Color::srgb_u8(226, 58, 92),
            Stable::Yellow => Color::srgb_u8(232, 160, 36),
            Stable::DarkGreen => Color::srgb_u8(30, 112, 92),
            Stable::Purple => Color::srgb_u8(92, 60, 170),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    Transport,
    Stables,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Horse {
    pub name: &'static str,
    /// Name of the photo among the original game's assets (`horse/<photo>`).
    pub photo: &'static str,
    pub stable: Stable,
    pub price: i64,
    /// Stable visit, 1–4 races, main race of the season.
    pub fees: [i64; 6],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Start,
    Horse(Horse),
    Finance,
    Nahoda,
    Vet(i64),
    Trainer(u8),
    Service(Service),
    Distanc,
    Parking,
    Doping,
}

impl Field {
    pub fn name(&self) -> String {
        match self {
            Field::Start => "Start".into(),
            Field::Horse(h) => h.name.into(),
            Field::Finance => "Finance".into(),
            Field::Nahoda => "Náhoda".into(),
            Field::Vet(_) => "Veterinární vyšetření".into(),
            Field::Trainer(n) => format!("{n}. Trenér"),
            Field::Service(Service::Transport) => "Přeprava".into(),
            Field::Service(Service::Stables) => "Stáje".into(),
            Field::Distanc => "Distanc".into(),
            Field::Parking => "Parkoviště".into(),
            Field::Doping => "Podezření z dopingu".into(),
        }
    }

    /// Purchase price, for squares that can be owned.
    pub fn price(&self) -> Option<i64> {
        match self {
            Field::Horse(h) => Some(h.price),
            Field::Trainer(_) => Some(TRAINER_PRICE),
            Field::Service(_) => Some(SERVICE_PRICE),
            _ => None,
        }
    }

    pub fn stable(&self) -> Option<Stable> {
        match self {
            Field::Horse(h) => Some(h.stable),
            _ => None,
        }
    }
}

const fn h(name: &'static str, photo: &'static str, stable: Stable, price: i64, fees: [i64; 6]) -> Field {
    Field::Horse(Horse { name, photo, stable, price, fees })
}

use Stable::*;

/// The board clockwise from Start.
pub static BOARD: [Field; 40] = [
    Field::Start,
    h("Fantome", "fantome", Orange, 1200, [40, 200, 600, 1800, 3200, 5000]),
    Field::Finance,
    h("Gavora", "gavora", Orange, 1200, [40, 200, 600, 1800, 3200, 5000]),
    Field::Vet(500),
    Field::Trainer(1),
    h("Lady Anne", "ladyanne", Brown, 2000, [120, 600, 1800, 5400, 8000, 11000]),
    Field::Nahoda,
    h("Pasek", "pasek", Brown, 2000, [120, 600, 1800, 5400, 8000, 11000]),
    h("Koran", "koran", Brown, 2400, [160, 800, 2000, 6000, 9000, 12000]),
    Field::Distanc,
    h("Neklan", "neklan", LightBlue, 2800, [200, 1000, 3000, 9000, 12500, 15000]),
    Field::Service(Service::Transport),
    h("Portlancl", "portlancl", LightBlue, 2800, [200, 1000, 3000, 9000, 12500, 15000]),
    h("Japan", "japan", LightBlue, 2800, [240, 1200, 3600, 10000, 14000, 18000]),
    Field::Trainer(2),
    h("Kostrava", "kostrava", Green, 3600, [280, 1400, 4000, 11000, 15000, 19000]),
    Field::Finance,
    h("Lukava", "lukava", Green, 3600, [280, 1400, 4000, 11000, 15000, 19000]),
    h("Melák", "melak", Green, 4000, [320, 1600, 4400, 12000, 16000, 20000]),
    Field::Parking,
    h("Grifel", "grifel", Pink, 4400, [360, 1800, 5000, 14000, 17000, 21000]),
    Field::Nahoda,
    h("Mohyla", "mohyla", Pink, 4400, [360, 1800, 5000, 14000, 17000, 21000]),
    h("Metál", "metal", Pink, 4800, [400, 2000, 6000, 15000, 18000, 22000]),
    Field::Trainer(3),
    h("Tara", "tara", Yellow, 5200, [440, 2200, 6600, 16000, 19500, 23000]),
    h("Furioso", "furioso", Yellow, 5200, [440, 2200, 6600, 16000, 19500, 23000]),
    Field::Service(Service::Stables),
    h("Genius", "genius", Yellow, 5600, [580, 2400, 7200, 17000, 20500, 24000]),
    Field::Doping,
    h("Shagga", "shagga", DarkGreen, 6000, [500, 2600, 7800, 18000, 22000, 25500]),
    h("Dahoman", "dahoman", DarkGreen, 6000, [500, 2600, 7800, 18000, 22000, 25500]),
    Field::Finance,
    h("Gira", "gira", DarkGreen, 6400, [560, 3000, 9000, 20000, 24000, 28000]),
    Field::Trainer(4),
    Field::Nahoda,
    h("Narcius", "narcius", Purple, 7000, [700, 3500, 10000, 22000, 26000, 30000]),
    Field::Vet(1000),
    h("Napoli", "napoli", Purple, 8000, [1000, 4000, 12000, 28000, 34000, 40000]),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinanceCard {
    Pay(i64),
    Receive(i64),
    /// Paid only to owners of a horse with races.
    RaceWinnings(i64),
    FromEachPlayer(i64),
    PerRace { race: i64, main: i64 },
}

pub static FINANCE: [(&str, FinanceCard); 14] = [
    ("Zaplať pojistku 1000,-", FinanceCard::Pay(1000)),
    ("Pokuta za nedodržení předpisů. Zaplať 400,-", FinanceCard::Pay(400)),
    ("Renovuješ všechny stáje. Za každý svůj obsazený dostih zaplať 500,-", FinanceCard::PerRace { race: 500, main: 500 }),
    ("Mimořádný zisk z dostihů. Obdržíš 2000,-", FinanceCard::RaceWinnings(2000)),
    ("Jako dárek k narozeninám obdržíš od každého hráče 200,-", FinanceCard::FromEachPlayer(200)),
    ("Mimořádná prémie 500,-", FinanceCard::Receive(500)),
    ("Obdržíš dotaci 4000,-", FinanceCard::Receive(4000)),
    ("Zaplať dluh 3000,-", FinanceCard::Pay(3000)),
    ("Renovuješ všechny stáje. Za každý svůj dostih zaplať 800,-, za každý hlavní dostih 2300,-", FinanceCard::PerRace { race: 800, main: 2300 }),
    ("Zaplať příspěvek 2000,-", FinanceCard::Pay(2000)),
    ("Nákup materiálu na opravu. Zaplať 100,-", FinanceCard::Pay(100)),
    ("Výhra v loterii 1000,-", FinanceCard::Receive(1000)),
    ("Obdržíš dotaci 2000,-", FinanceCard::Receive(2000)),
    ("Z banky obdržíš přeplatek 3000,-", FinanceCard::Receive(3000)),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Back,
}

/// Squares a Náhoda card can send a pawn to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Landmark {
    Start,
    Finance,
    Trainer,
    Distanc,
    Parking,
    /// The last square before Start (Napoli).
    LastSquare,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    Steps(usize),
    Nearest(Landmark),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NahodaCard {
    /// Walk the pawn; crossing or reaching Start pays the bonus unless `start_bonus` is false.
    Walk { direction: Direction, to: Destination, start_bonus: bool },
    /// Kept by the player until used to leave Distanc.
    CancelDistanc,
    Sit(u8),
}

const fn walk(direction: Direction, to: Destination, start_bonus: bool) -> NahodaCard {
    NahodaCard::Walk { direction, to, start_bonus }
}

use Destination::*;
use Direction::*;
use Landmark::*;

pub static NAHODA: [(&str, NahodaCard); 14] = [
    ("Jdi o 3 pole zpět.", walk(Back, Steps(3), true)),
    ("Získáváš kartu *Zrušen distanc*. Ponech si ji, dokud ji nepoužiješ.", NahodaCard::CancelDistanc),
    ("Postoupíš na nejbližší pole Trenér. Pokud projdeš startem, obdržíš 4000,-", walk(Forward, Nearest(Trainer), true)),
    ("Zdržíš se na 2 kola.", NahodaCard::Sit(2)),
    ("Jdi na Distanc (bez 4000,-).", walk(Forward, Nearest(Distanc), false)),
    ("Zpět na nejbližší pole Finance.", walk(Back, Nearest(Finance), true)),
    ("Zpět na poslední políčko ve hře (kůň Napoli). Obdržíš 4000,-", walk(Back, Nearest(LastSquare), true)),
    ("Zpět na pole Distanc. Obdržíš 4000,- pokud jsi cestou zpět prošel startem.", walk(Back, Nearest(Distanc), true)),
    ("Zpět na nejbližší pole Finance.", walk(Back, Nearest(Finance), true)),
    ("Zpět na Start. Obdržíš 4000,-", walk(Back, Nearest(Start), true)),
    ("Zpět na Start (bez 4000,-).", walk(Back, Nearest(Start), false)),
    ("Zdržíš se na 2 kola.", NahodaCard::Sit(2)),
    ("Zdržíš se na 1 kolo.", NahodaCard::Sit(1)),
    ("Zpět na Parkoviště. Obdržíš 4000,- pokud jsi cestou zpět prošel startem.", walk(Back, Nearest(Parking), true)),
];
