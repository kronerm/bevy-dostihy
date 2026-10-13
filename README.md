# Dostihy & Sázky

A Rust/Bevy clone of the Czech horse-racing board game, for playing at home:
hot-seat for 2–6 players, any of them optionally played by the computer.

## Setup

```sh
cargo run
```

The graphics and sounds come from the 2005 freeware game *Dostihy 3000
Deluxe* (its licence is in [LICENSE-original.txt](LICENSE-original.txt)) and are not part of this
repository. On first start the game downloads the game's installer, checks it
(BLAKE3) and takes them out of it in memory; a copy of the installer stays in
`assets/` (or put yours there as `assets/Dostihy3000setup.exe`). In a browser
it has to be among the game's assets (see below).

Click a square on the board (or a card in a player's hand) to see its card;
your own cards can be sold to the bank from there. Trade by clicking the
cards in the hands. Hover over a hand to look at its cards.

## In the browser

```sh
./build-wasm.sh
```

Needs the nightly toolchain (see `rust-toolchain.toml`), the
`wasm32-unknown-unknown` target, `wasm-bindgen-cli` of the version in
`Cargo.lock` (`cargo binstall wasm-bindgen-cli@<version>`; the script says
which), `wasm-opt` (binaryen), `tomlq` and `curl`. `FAST=1 ./build-wasm.sh`
builds much quicker but much bigger; drop it for anything you ship.

`web/` then holds the game (`dostihy.js`, `dostihy_bg.wasm`) and `assets/`:
Noto Sans (a browser has no system fonts to find, so the build downloads it;
SIL OFL) and the original game's installer, copied from `assets/` when you have it – whatever
hosts `web/` then redistributes it. Without it the game plays without the
original graphics. Serve the `.wasm` with `Content-Encoding: br` or gzip.

Serve those together, anywhere: the game finds `assets/` next to its
`dostihy.js`, wherever the page embedding it is. The page gives it a canvas
to draw on, `#bevy-canvas-target`, and starts it:

```html
<canvas id="bevy-canvas-target"></canvas>
<script type="module">
  import init from "/games/dostihy/dostihy.js";
  init();
</script>
```

The rules as played here – house rules and the computer's ways included –
are in [RULES.md](RULES.md) (in Czech).

## Layout

- `src/data.rs` – the board, horse cards and Finance / Náhoda cards.
- `src/rules/` – the rules as ECS: squares are a ring of entities linked by
  `Next`/`Prev`, ownership and pawn positions are relationships, player actions
  are events handled by observers. Effects apply instantly and are announced
  as presentation events (`DiceRolled`, `PawnMoved`, `MoneyMoved`, `CardDrawn`).
- `src/ai.rs` – the computer player; it triggers the same events as the buttons.
- `src/ui/` – a loading screen, the cake-shaped board, animations of the presentation events,
  and the side panel.
- `tests/e2e.rs` – computer players play whole games while invariants are checked.
- `tests/rules_doc.rs` – the numbers and card texts in `RULES.md` match the game's.
- `examples/screenshot.rs` – plays a few turns and saves a screenshot.
- `src/original.rs` – downloads the original game's installer and unpacks its
  graphics and sounds as assets.

Card texts, prices and fees were checked against the original board game
(Japan costs 2 800, Genius's stable visit really is 580).

Working on the code (people or agents): see [AGENTS.md](AGENTS.md).

## Licence

The code is free software under the GNU General Public License, version 3
or (at your option) any later version – see [LICENSE](LICENSE).

That doesn't cover what the game takes from elsewhere: the original game's
graphics and sounds stay under its own licence
([LICENSE-original.txt](LICENSE-original.txt)), and Noto Sans in the web
build under the SIL Open Font License.
