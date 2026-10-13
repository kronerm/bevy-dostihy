# Notes for agents

A Rust/Bevy 0.19 clone of the Czech board game Dostihy a sázky. See
`README.md` for the layout of the code.

## RULES.md and the code say the same thing

`RULES.md` (in Czech) is the rulebook: every rule, house rule, number and bit
of gameplay, plus how the computer plays. People edit it by hand, and people
change the rules in code. Keep the two in step, both ways:

- **Changing gameplay in code** (`src/rules/`, `src/data.rs`, `src/ai.rs`,
  or rule texts the UI shows, such as `describe` in `src/ui/widgets.rs`):
  update `RULES.md` in the same change.
- **`RULES.md` was edited** (check `git diff RULES.md`, or `git log -p
  RULES.md` since the code last changed): implement the change in code, with
  tests, and fix any wording that no longer matches. If the edit is unclear
  or contradicts itself, ask rather than guess.
- At the start of a task touching gameplay, skim the relevant part of
  `RULES.md`; if it disagrees with the code, point it out and ask which one
  is right.
- `tests/rules_doc.rs` checks mechanically what it can: the `## Čísla` table
  against the constants, the `## Hrací plán` table against `BOARD`, and the
  card lists against `FINANCE` / `NAHODA`. A new rule constant gets a row in
  `## Čísla` and a line in that test. The prose is yours to keep true.
- Keep `RULES.md` readable for players: Czech, plain words, no code beyond
  the constant names in the table.

## Code style

- ECS first: entities, components, small resources; player actions are
  events handled by observers. No god-structs.
- Enums over integer codes and flags, invalid states unrepresentable – but
  a plain `bool` stays a `bool`.
- Simple, readable, small. Comments say why; match the surrounding code.
- Keep it wasm-safe (no `SystemTime`, threads or filesystem in game code) and
  keep player actions small events with randomness behind `GameRng`: the game
  is meant to run in the browser and online (iroh matchmaking) one day.

## Workflow

- Use the dev profile (`cargo run`, `cargo test`); dependencies are already
  optimised. `--release` rebuilds all of Bevy and takes 15+ minutes.
- `cargo test` and `cargo clippy --all-targets` must pass.
- Screenshots: `cargo run --example screenshot -- out.png [setup|game|mid|rich] [turns] [delay]`.
  After any change to the panel or the board overlays run
  `cargo run --example screenshot -- out.png rich 0`: it builds the worst
  layout and exits 1 with `CUT OFF: …` if anything doesn't fit. Nothing may
  ever be cut off.
- No binary assets go into git. The graphics and sounds come from the 2005
  freeware game *Dostihy 3000 Deluxe* (its licence in `LICENSE-original.txt`): `src/original.rs`
  loads its installer as an ordinary asset from the assets folder
  (`asset_root`; desktop downloads it there when missing, BLAKE3-checked,
  gitignored) and unpacks it in memory. Ask for its pictures and sounds
  (`original(label)`) only once it has loaded – earlier, Bevy loads the whole
  installer again for each. Never commit or redistribute the installer or
  anything taken out of it. Text uses the
  system's sans-serif font (`system_font_discovery`); the web build brings
  Noto Sans, which `build-wasm.sh` downloads (pinned, SHA-256-checked).
- Web build: `FAST=1 ./build-wasm.sh` (see `README.md`). Bevy's features are
  listed one by one in `Cargo.toml` to keep the wasm small; a new Bevy API
  may need one added. `cargo clippy --target wasm32-unknown-unknown` checks
  the wasm side quickly.
- `cargo test -- --ignored` also checks that the installer unpacks into every
  asset the game uses (needs the installer or a download).
- Bevy UI: a node spawned first and given `Button` later keeps
  `FocusPolicy::Pass`, so clicks fall through to what's under it; insert
  `FocusPolicy::Block` with the `Button`.
