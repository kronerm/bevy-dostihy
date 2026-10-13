//! RULES.md is the rulebook people edit; these checks keep its numbers and
//! card texts the same as the game's. When one fails, bring the two together
//! (see AGENTS.md) – the test doesn't say which one is right.

use dostihy::data::*;
use dostihy::rules::MAX_TRADE_OFFERS;

const RULES: &str = include_str!("../RULES.md");

/// "30 000" → 30000.
fn number(cell: &str) -> Option<i64> {
    cell.replace([' ', '\u{a0}'], "").parse().ok()
}

/// Cells of the table rows in a `## section`.
fn rows(section: &str) -> Vec<Vec<&str>> {
    let start = RULES.find(&format!("## {section}\n")).unwrap_or_else(|| panic!("RULES.md has a \"## {section}\" section"));
    let body = &RULES[start..];
    let body = &body[..body[3..].find("\n## ").map_or(body.len(), |i| i + 3)];
    body.lines()
        .filter(|l| l.starts_with('|') && !l.starts_with("|--") && !l.starts_with("|:-"))
        .skip(1)
        .map(|l| l.trim_matches('|').split('|').map(str::trim).collect())
        .collect()
}

fn stable_name(stable: Stable) -> &'static str {
    match stable {
        Stable::Orange => "oranžová",
        Stable::Brown => "hnědá",
        Stable::LightBlue => "světle modrá",
        Stable::Green => "zelená",
        Stable::Pink => "růžová",
        Stable::Yellow => "žlutá",
        Stable::DarkGreen => "tmavě zelená",
        Stable::Purple => "fialová",
    }
}

#[test]
fn numbers_match() {
    let constants = [
        ("START_MONEY", START_MONEY),
        ("START_BONUS", START_BONUS),
        ("START_LANDING_BONUS", START_LANDING_BONUS),
        ("TRAINER_PRICE", TRAINER_PRICE),
        ("SERVICE_PRICE", SERVICE_PRICE),
        ("TRAINER_FEE", TRAINER_FEE),
        ("SERVICE_FEE", SERVICE_FEE),
        ("BOTH_SERVICES_FEE", BOTH_SERVICES_FEE),
        ("DISTANC_TRIES", DISTANC_TRIES as i64),
        ("MAX_TRADE_OFFERS", MAX_TRADE_OFFERS as i64),
    ];
    let table = rows("Čísla");
    assert_eq!(table.len(), constants.len(), "one row per constant");
    for (name, value) in constants {
        let row = table.iter().find(|r| r[1] == format!("`{name}`")).unwrap_or_else(|| panic!("RULES.md lists {name}"));
        assert_eq!(number(row[2]), Some(value), "{name}");
    }
}

#[test]
fn board_matches() {
    let table = rows("Hrací plán");
    assert_eq!(table.len(), BOARD.len(), "one row per square");
    for (i, (field, row)) in BOARD.iter().zip(&table).enumerate() {
        assert_eq!((number(row[0]), row[1]), (Some(i as i64), field.name().as_str()), "square {i}");
        let price = match field {
            Field::Vet(fee) => Some(*fee),
            f => f.price(),
        };
        assert_eq!(number(row[3]), price, "price of {}", row[1]);
        if let Field::Horse(h) = field {
            assert_eq!(row[2], stable_name(h.stable), "stable of {}", h.name);
            assert_eq!(number(row[4]), Some(h.stable.race_cost()), "race cost of {}", h.name);
            let fees: Vec<Option<i64>> = row[5..11].iter().map(|c| number(c)).collect();
            assert_eq!(fees, h.fees.map(Some), "fees of {}", h.name);
        }
    }
}

#[test]
fn cards_match() {
    let finance = FINANCE.map(|c| c.0);
    let nahoda = NAHODA.map(|c| c.0);
    for (deck, texts) in [("Finance", finance), ("Náhoda", nahoda)] {
        let start = RULES.find(&format!("\nKarty {deck}:\n")).unwrap_or_else(|| panic!("RULES.md lists the {deck} cards"));
        let listed: Vec<&str> = RULES[start + 1..].lines().skip(2).take_while(|l| l.starts_with("- „")).map(|l| l.trim_start_matches("- „").trim_end_matches('“')).collect();
        assert_eq!(listed, texts, "{deck} cards, in deck order");
    }
}
