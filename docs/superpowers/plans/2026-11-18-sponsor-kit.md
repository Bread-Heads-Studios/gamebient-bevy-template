# Sponsor Demo Kit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Look Pack's `end_card` shows on the game-over screen, a starter kit lets anyone build a pitch pack, and Voidrunner ships the fictional FIZZIX Energy sponsor pack.

**Architecture:** The template's verbatim-copied `src/look/` gains `end_card.rs`: a game puts an `EndCardSlot` marker on an empty UI node and inserts an `EndCardStyle` resource; a `PostUpdate` system fills every slot from the active pack's `end_card` on spawn and on `LookChanged`. The validator rejects `TODO:` text. Voidrunner copies `src/look/`, places the slot on its game-over card and adds `assets/looks/fizzix/`.

**Tech Stack:** Rust 2024, Bevy 0.18 UI, serde_json, Python 3 (`qrcode` package, pinned), rsvg-convert.

Spec: `/Users/kelliott/Gamebient/colecovisiongx/docs/plans/2026-11-17-sponsor-kit-design.md` (workspace root, not in this repo).

## Global Constraints

- `src/look/` contains no reference to `crate::ui`, `crate::game` or `crate::display`; it is copied verbatim into games. Voidrunner's `src/look/` must be byte-identical to the template's after Task 5 (`diff -r` clean).
- `SCHEMA_VERSION` stays 3. No manifest schema change.
- End-card palette roles (undotted UI roles): `endcard` (panel fill), `endcardhead`, `endcardbody`, `endcardurl`.
- End-card layout: one row; image 112 px tall, aspect kept, left; text column right with headline (display font), body, URL (body font). Missing parts are skipped. URL is plain text, never a link.
- No pack, or a pack without `end_card`: the slot has no children and takes no space.
- End card is presentation only: sim systems never read it; replays and checksums are unchanged under every pack.
- `TODO:` anywhere in pack text (name, creator, strings, text recipe params, end_card headline/body/url) is a validation **error**.
- Pack folders hold only `.png` and `.ogg` plus `look.json`; SVG sources live outside pack folders.
- FIZZIX: `v` 3, `id` "fizzix", `name` "FIZZIX Energy", `kind` "sponsor", `creator` "Bread Heads Studios"; URL `https://example.com/fizzix`; end card headline "REFUEL WITH FIZZIX", body "Code VOID20 - 20% off your first case", image "qr.png"; title art "logo.png"; no glow, no audio.
- Built-in packs ship clean: zero validation problems (errors or warnings).
- CI per repo: `cargo test && cargo clippy -- -D warnings && cargo fmt --check`. Template rollout tests (`tools/test_rollouts.sh` etc.) must keep passing; any new `ci.yml` step goes after "Cutter tests".
- Commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Repos and branches

- Template: worktree `libs/gamebient-bevy-template/.claude/worktrees/sponsor-kit`, branch `sponsor-kit` from `origin/main` (c759399). Tasks 1–4.
- Voidrunner: worktree `games/voidrunner/.claude/worktrees/sponsor-kit`, branch `sponsor-kit` from `origin/main` (c6981b0). Tasks 5–6. Created by the controller after the template PR merges.

---

### Task 1: Validator rejects `TODO:` text (template)

**Files:**
- Modify: `src/look/validate.rs` (`Check::text`, the end_card URL check, tests module)

**Interfaces:**
- Produces: any `Check::text` call fails on a string containing `TODO:`; `end_card.url` fails on `TODO:` too. Error message: `"still a TODO: placeholder; fill it in"`.

- [ ] **Step 1: Write the failing tests** in `validate.rs`'s `mod tests` (uses the existing `pack()` helper and `TEST` contract):

```rust
    #[test]
    fn todo_placeholders_are_errors_everywhere_text_goes() {
        let cases = [
            r#"{"v":1,"id":"demo","name":"TODO: name","kind":"sponsor"}"#,
            r#"{"v":1,"id":"demo","name":"D","creator":"TODO: you","kind":"sponsor"}"#,
            r#"{"v":1,"id":"demo","name":"D","kind":"sponsor","end_card":{"headline":"TODO: headline"}}"#,
            r#"{"v":1,"id":"demo","name":"D","kind":"sponsor","end_card":{"headline":"H","body":"TODO: body"}}"#,
            r#"{"v":1,"id":"demo","name":"D","kind":"sponsor","end_card":{"headline":"H","url":"https://example.com/TODO:"}}"#,
        ];
        for json in cases {
            let r = validate(&pack(json), &TEST, None);
            assert!(!r.ok(), "{json} should fail");
            assert!(
                r.problems.iter().any(|p| p.message.contains("TODO:")),
                "{json}: {:#?}",
                r.problems
            );
        }
    }

    #[test]
    fn todo_in_a_string_slot_is_an_error() {
        // Use a string key the TEST contract declares (look at `TEST.strings`
        // above and substitute its first key for "title" if it differs).
        let json = r#"{"v":1,"id":"demo","name":"D","kind":"sponsor","strings":{"title":"TODO: title"}}"#;
        let r = validate(&pack(json), &TEST, None);
        assert!(r.problems.iter().any(|p| p.at.contains("title") && p.message.contains("TODO:")));
    }

    #[test]
    fn the_word_todo_without_a_colon_is_fine() {
        let json = r#"{"v":1,"id":"demo","name":"TODO LIST","kind":"sponsor"}"#;
        assert!(validate(&pack(json), &TEST, None).ok());
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib look::validate::tests::todo -- --nocapture`
Expected: the first two tests FAIL (packs currently validate); the third passes.

- [ ] **Step 3: Implement.** In `Check::text`, after the existing printable-ASCII check:

```rust
    fn text(&mut self, at: &str, s: &str, max: usize) {
        if !is_text(s, max) {
            self.error(
                at,
                format!("must be printable ASCII, at most {max} characters"),
            );
        }
        self.no_todo(at, s);
    }
    /// A starter pack marks every field `TODO: ...`; none may ship.
    fn no_todo(&mut self, at: &str, s: &str) {
        if s.contains("TODO:") {
            self.error(at, "still a TODO: placeholder; fill it in".to_string());
        }
    }
```

(Match `error`'s actual parameter type; if it takes `impl Into<String>` drop `.to_string()`.) In the `end_card` block, after the URL https check, add `c.no_todo("end_card.url", url);` inside an `if let Some(url) = &card.url { … }` (restructure the existing `if let … &&` so both checks run).

- [ ] **Step 4: Run tests**

Run: `cargo test --lib look::validate && cargo test --test looks`
Expected: PASS (built-in packs contain no `TODO:`).

- [ ] **Step 5: Docs.** In `docs/look-packs.md`, under the field table, add one line: "Any text containing `TODO:` is an error, so an unfinished copy of the sponsor-kit starter cannot load."

- [ ] **Step 6: Commit**

```bash
git add src/look/validate.rs docs/look-packs.md
git commit -m "look: reject TODO: placeholders in pack text"
```

---

### Task 2: `EndCardSlot` presentation (template `src/look/end_card.rs`)

**Files:**
- Create: `src/look/end_card.rs`
- Modify: `src/look/mod.rs` (module, re-exports, system registration)
- Test: `tests/end_card.rs` (new integration test)

**Interfaces:**
- Produces (re-exported from `crate::look`):
  - `#[derive(Component, Default)] pub struct EndCardSlot;`
  - `#[derive(Component)] pub struct EndCardPart(pub Part);` on each built child (lets tests and games find parts).
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum Part { Image, Headline, Body, Url }`
  - `#[derive(Resource, Clone)] pub struct EndCardStyle { pub display: TextFont, pub body: TextFont, pub display_advance_em: f32, pub body_advance_em: f32, pub text_width: f32, pub headline_size: f32, pub body_size: f32, pub url_size: f32 }`
  - `pub const IMAGE_HEIGHT: f32 = 112.0;`
  - `pub const ROLES: [&str; 4] = ["endcard", "endcardhead", "endcardbody", "endcardurl"];`
  - `pub fn parts(card: &EndCard, has_image: bool) -> Vec<Part>`
  - `pub fn fit(advance_em: f32, text: &str, max_size: f32, width: f32, lines: f32) -> f32`
  - system `pub fn fill_slots(...)`, registered by `LookPlugin` in `PostUpdate`, `.before(LookSet)`, `.run_if(resource_exists::<EndCardStyle>)`.

- [ ] **Step 1: Write the pure-logic unit tests** at the bottom of the new `src/look/end_card.rs` (write the tests first; the file will not compile until Step 3 adds the code, which is the expected failure):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn card(body: bool, url: bool, image: bool) -> EndCard {
        EndCard {
            headline: "REFUEL".into(),
            body: body.then(|| "Code X".into()),
            url: url.then(|| "https://example.com".into()),
            image: image.then(|| "qr.png".into()),
        }
    }

    #[test]
    fn every_part_in_order() {
        assert_eq!(
            parts(&card(true, true, true), true),
            vec![Part::Image, Part::Headline, Part::Body, Part::Url]
        );
    }

    #[test]
    fn missing_fields_skip_only_their_part() {
        assert_eq!(parts(&card(false, true, true), true), vec![Part::Image, Part::Headline, Part::Url]);
        assert_eq!(parts(&card(true, false, true), true), vec![Part::Image, Part::Headline, Part::Body]);
        assert_eq!(parts(&card(true, true, false), true), vec![Part::Headline, Part::Body, Part::Url]);
        // Named but not decoded (headless, or a bad PNG): no image part.
        assert_eq!(parts(&card(true, true, true), false), vec![Part::Headline, Part::Body, Part::Url]);
    }

    #[test]
    fn fit_keeps_short_text_at_its_size_and_shrinks_long_text() {
        assert_eq!(fit(0.6, "SHORT", 28.0, 400.0, 1.0), 28.0);
        let long = "W".repeat(64);
        let size = fit(0.77, &long, 28.0, 400.0, 1.0);
        assert!(64.0 * 0.77 * size <= 400.0, "{size}");
        assert!(size >= 1.0);
    }

    #[test]
    fn two_lines_allow_twice_the_text() {
        let text = "W".repeat(100);
        assert!(fit(0.6, &text, 18.0, 300.0, 2.0) > fit(0.6, &text, 18.0, 300.0, 1.0));
    }

    #[test]
    fn empty_text_does_not_divide_by_zero() {
        assert_eq!(fit(0.6, "", 20.0, 300.0, 1.0), 20.0);
    }
}
```

- [ ] **Step 2: Write the failing integration test** `tests/end_card.rs`:

```rust
//! The end-card slot fills from the active pack and rebuilds on a swap.

use bevy::prelude::*;
use gamebient_game::look::{
    ActiveLook, EndCardPart, EndCardSlot, EndCardStyle, LookChanged, LookPlugin, Part, Selection,
};
use gamebient_game::ui::theme;

fn style() -> EndCardStyle {
    EndCardStyle {
        display: TextFont::from_font_size(28.0),
        body: TextFont::from_font_size(18.0),
        display_advance_em: 0.6,
        body_advance_em: 0.6,
        text_width: 400.0,
        headline_size: 28.0,
        body_size: 18.0,
        url_size: 16.0,
    }
}

fn app(selection: Selection) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.init_asset::<Image>();
    app.insert_resource(selection);
    app.insert_resource(style());
    app.add_plugins(LookPlugin { contract: &theme::LOOK });
    app
}

fn part_list(app: &mut App) -> Vec<Part> {
    let mut q = app.world_mut().query::<&EndCardPart>();
    let mut v: Vec<Part> = q.iter(app.world()).map(|p| p.0).collect();
    v.sort_by_key(|p| *p as u8);
    v
}

fn text_of(app: &mut App, part: Part) -> String {
    let mut q = app.world_mut().query::<(&EndCardPart, &Text)>();
    q.iter(app.world()).find(|(p, _)| p.0 == part).unwrap().1.0.clone()
}

#[test]
fn no_pack_leaves_the_slot_empty() {
    let mut app = app(Selection::Default);
    let slot = app.world_mut().spawn((Node::default(), EndCardSlot)).id();
    app.update();
    assert!(app.world().get::<Children>(slot).is_none_or(|c| c.is_empty()));
}

#[test]
fn a_pack_without_an_end_card_leaves_the_slot_empty() {
    let mut app = app(Selection::Embedded("night-shift".into()));
    let slot = app.world_mut().spawn((Node::default(), EndCardSlot)).id();
    app.update();
    assert!(app.world().get::<Children>(slot).is_none_or(|c| c.is_empty()));
}

#[test]
fn the_sample_sponsor_fills_the_slot() {
    let mut app = app(Selection::Embedded("sample-sponsor".into()));
    app.world_mut().spawn((Node::default(), EndCardSlot));
    app.update();
    assert_eq!(part_list(&mut app), vec![Part::Image, Part::Headline, Part::Body, Part::Url]);
    assert_eq!(text_of(&mut app, Part::Headline), "THANKS FOR PLAYING");
    assert_eq!(text_of(&mut app, Part::Url), "https://example.com");
    let look = app.world().resource::<ActiveLook>();
    let want = look.colour("endcardhead");
    let mut q = app.world_mut().query::<(&EndCardPart, &TextColor)>();
    let got = q.iter(app.world()).find(|(p, _)| p.0 == Part::Headline).unwrap().1.0;
    assert_eq!(got, want);
}

#[test]
fn a_swap_to_no_card_empties_the_slot() {
    let mut app = app(Selection::Embedded("sample-sponsor".into()));
    app.world_mut().spawn((Node::default(), EndCardSlot));
    app.update();
    assert!(!part_list(&mut app).is_empty());
    let contract = app.world().resource::<ActiveLook>().contract;
    app.insert_resource(ActiveLook::default_for(contract));
    app.world_mut().write_message(LookChanged);
    app.update();
    assert!(part_list(&mut app).is_empty());
}
```

If `Image` decoding needs `ImagePlugin` for `Assets<Image>` in 0.18, add whatever minimal registration makes `load_assets` decode `sample-sponsor/logo.png` (the image part depends on it). Do not drop the image assertion.

- [ ] **Step 3: Implement `src/look/end_card.rs`:**

```rust
//! A pack's sponsor end card, drawn where the game puts an `EndCardSlot`.
//! The game owns the slot's place and the `EndCardStyle` (fonts, sizes,
//! column width); this module only fills the slot from the active pack, on
//! spawn and on every `LookChanged`. With no `end_card` the slot stays
//! empty and takes no space. Presentation only: the sim never reads it.
//! The URL is shown as text; the game never opens it.

use bevy::prelude::*;

use super::apply::{LookAssets, LookChanged};
use super::manifest::EndCard;
use super::runtime::ActiveLook;

/// Height of the card's image, in UI pixels. Width follows the image.
pub const IMAGE_HEIGHT: f32 = 112.0;
/// Panel fill, headline, body and URL colours. Each game declares these
/// palette roles in its contract.
pub const ROLES: [&str; 4] = ["endcard", "endcardhead", "endcardbody", "endcardurl"];
const GAP: f32 = 16.0;
const PAD: f32 = 16.0;

/// Put on an empty UI node; it is filled with the end card.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct EndCardSlot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Image,
    Headline,
    Body,
    Url,
}

/// Marks each node the card is built from.
#[derive(Component, Debug, Clone, Copy)]
pub struct EndCardPart(pub Part);

/// The game's fonts and sizes for the card. Insert it to show end cards.
#[derive(Resource, Debug, Clone)]
pub struct EndCardStyle {
    pub display: TextFont,
    pub body: TextFont,
    /// Worst-case average advance per character, in ems (`ui::fit::Face`).
    pub display_advance_em: f32,
    pub body_advance_em: f32,
    /// Width of the text column, in UI pixels.
    pub text_width: f32,
    pub headline_size: f32,
    pub body_size: f32,
    pub url_size: f32,
}

/// The parts `card` shows, in order. `has_image`: its image decoded.
pub fn parts(card: &EndCard, has_image: bool) -> Vec<Part> {
    let mut v = Vec::new();
    if card.image.is_some() && has_image {
        v.push(Part::Image);
    }
    v.push(Part::Headline);
    if card.body.is_some() {
        v.push(Part::Body);
    }
    if card.url.is_some() {
        v.push(Part::Url);
    }
    v
}

/// Largest whole size up to `max_size` at which `text` fits `lines` lines
/// of `width` (the same arithmetic as `ui::fit`, which this folder may not
/// name).
pub fn fit(advance_em: f32, text: &str, max_size: f32, width: f32, lines: f32) -> f32 {
    let glyphs = text.chars().count().max(1) as f32;
    (width * lines / (glyphs * advance_em)).min(max_size).floor().max(1.0)
}

fn font(base: &TextFont, size: f32) -> TextFont {
    TextFont {
        font_size: size,
        ..base.clone()
    }
}

#[allow(clippy::too_many_arguments)]
pub fn fill_slots(
    mut commands: Commands,
    look: Res<ActiveLook>,
    assets: Res<LookAssets>,
    style: Res<EndCardStyle>,
    mut changed: MessageReader<LookChanged>,
    slots: Query<(Entity, Ref<EndCardSlot>)>,
) {
    let all = changed.read().count() > 0;
    for (slot, tag) in &slots {
        if !(all || tag.is_added()) {
            continue;
        }
        let mut e = commands.entity(slot);
        e.despawn_related::<Children>();
        e.insert(BackgroundColor(Color::NONE));
        let Some(card) = look.manifest.as_ref().and_then(|m| m.end_card.as_ref()) else {
            continue;
        };
        let image = card.image.as_deref().and_then(|f| assets.image(f));
        let parts = parts(card, image.is_some());
        e.insert((
            BackgroundColor(look.colour(ROLES[0])),
            BorderRadius::all(Val::Px(12.0)),
        ));
        e.with_children(|row| {
            if let Some(handle) = image.filter(|_| parts.contains(&Part::Image)) {
                row.spawn((
                    EndCardPart(Part::Image),
                    ImageNode::new(handle),
                    Node {
                        height: Val::Px(IMAGE_HEIGHT),
                        ..default()
                    },
                ));
            }
            row.spawn(Node {
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                width: Val::Px(style.text_width),
                row_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|col| {
                let line = |text: &str, part: Part, base: &TextFont, adv: f32, size: f32, lines: f32, role: &'static str| {
                    (
                        EndCardPart(part),
                        Text::new(text),
                        font(base, fit(adv, text, size, style.text_width, lines)),
                        TextColor(look.colour(role)),
                    )
                };
                col.spawn(line(&card.headline, Part::Headline, &style.display, style.display_advance_em, style.headline_size, 1.0, ROLES[1]));
                if let Some(body) = &card.body {
                    col.spawn(line(body, Part::Body, &style.body, style.body_advance_em, style.body_size, 2.0, ROLES[2]));
                }
                if let Some(url) = &card.url {
                    col.spawn(line(url, Part::Url, &style.body, style.body_advance_em, style.url_size, 1.0, ROLES[3]));
                }
            });
        });
    }
}
```

The slot node's own layout (row, padding, gap) is set by the system too, so games only spawn `(Node::default(), EndCardSlot)`: insert
`Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(GAP), padding: UiRect::all(Val::Px(PAD)), ..default() }` when a card is present, and `Node { display: Display::None, ..default() }` when not (so an empty slot takes no space even if the game gave it margins). Adjust API names to Bevy 0.18 if they differ (`despawn_related::<Children>()` is the 0.16+ way to drop children; `BorderRadius` may be a `Node` field `border_radius` in 0.18 as `menu.rs` uses — follow `menu.rs`). Keep clippy clean (split the closure into a helper fn if it trips `too_many_arguments`).

- [ ] **Step 4: Register in `src/look/mod.rs`:** `pub mod end_card;`, re-export `pub use end_card::{EndCardPart, EndCardSlot, EndCardStyle, Part};`, and in `LookPlugin::build` add

```rust
            .add_systems(
                PostUpdate,
                end_card::fill_slots
                    .before(LookSet)
                    .run_if(resource_exists::<end_card::EndCardStyle>),
            )
```

Update the module doc's file list if it has one.

- [ ] **Step 5: Run tests**

Run: `cargo test --lib look::end_card && cargo test --test end_card && cargo clippy --all-targets -- -D warnings`
Expected: PASS. The integration tests will fail until `theme::LOOK` declares the `endcard*` roles (`look.colour` returns `MISSING` otherwise) — Task 3 adds them; if `the_sample_sponsor_fills_the_slot` only fails on the colour assertion, add the four roles to `PALETTE` in `src/ui/theme.rs` now (values in Task 3 Step 1) and include that in this commit.

- [ ] **Step 6: Commit**

```bash
git add src/look/end_card.rs src/look/mod.rs tests/end_card.rs src/ui/theme.rs
git commit -m "look: end-card slot filled from the active pack"
```

---

### Task 3: Template wiring, game-over slot, docs (template)

**Files:**
- Modify: `src/ui/theme.rs` (palette roles), `src/ui/mod.rs` (insert `EndCardStyle`), `src/ui/menu.rs` (`spawn_game_over` slot), `docs/look-packs.md`, `AGENTS.md`

**Interfaces:**
- Consumes: `crate::look::{EndCardSlot, EndCardStyle}`, `crate::look::end_card::ROLES`.
- Produces: `pub fn end_card_style() -> EndCardStyle` in `src/ui/theme.rs` (games copy the pattern).

- [ ] **Step 1: Palette roles.** Add to `PALETTE` in `src/ui/theme.rs` (if not already added in Task 2):

```rust
    // Sponsor end card (`look::end_card`), on the game-over card.
    ("endcard", Color::srgba(0.035, 0.043, 0.094, 0.0)),
    ("endcardhead", PAPER),
    ("endcardbody", MUTED),
    ("endcardurl", SIGNAL),
```

The panel default is fully transparent (the template card already has a surface); a pack sets `endcard` to give the card its own plate.

- [ ] **Step 2: Failing test** in `theme.rs`'s `look_contract_tests`:

```rust
    #[test]
    fn the_contract_declares_every_end_card_role() {
        for role in crate::look::end_card::ROLES {
            assert!(PALETTE.iter().any(|(r, _)| *r == role), "{role} missing from PALETTE");
        }
    }

    #[test]
    fn the_end_card_column_fits_beside_its_image() {
        let s = end_card_style();
        let row = crate::look::end_card::IMAGE_HEIGHT + 16.0 + s.text_width + 2.0 * 16.0;
        assert!(row <= crate::ui::fit::ui_width() * 0.9, "{row}");
    }
```

Run `cargo test --lib look_contract_tests` → FAIL (`end_card_style` missing).

- [ ] **Step 3: Implement `end_card_style`** in `theme.rs`:

```rust
/// The end card's fonts and sizes (`look::end_card`): the column takes what
/// is left of the card beside a square image.
pub fn end_card_style() -> crate::look::EndCardStyle {
    let width = crate::ui::fit::ui_width();
    crate::look::EndCardStyle {
        display: fonts::DISPLAY.font(28.0),
        body: fonts::BODY.font(18.0),
        display_advance_em: fonts::DISPLAY.advance_em,
        body_advance_em: fonts::BODY.advance_em,
        text_width: (width * 0.9 - crate::look::end_card::IMAGE_HEIGHT - 16.0 - 32.0).min(480.0),
        headline_size: 28.0,
        body_size: 18.0,
        url_size: 16.0,
    }
}
```

Insert it in `UiPlugin::build` (`src/ui/mod.rs`): `.insert_resource(theme::end_card_style())`.

- [ ] **Step 4: Game-over slot.** In `menu.rs` `spawn_game_over`, inside the card after the best chip/badge and before the demo chip, add:

```rust
                    // A sponsor pack's end card (`look::end_card`); empty otherwise.
                    card.spawn((Node::default(), crate::look::EndCardSlot));
```

- [ ] **Step 5: Run everything**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check && tools/test_rollouts.sh`
Expected: PASS. (Run any other `tools/test_*.sh` that `ci.yml` runs; read `.github/workflows/ci.yml` for the list.)

- [ ] **Step 6: Manual check (native).** `GX_LOOK=sample-sponsor cargo run`, play until game over, confirm the card appears under the score with the Acme logo, headline, body and URL. Then `cargo run` (no pack): game-over unchanged. Record the result in the report; if a display is not available, say so.

- [ ] **Step 7: Docs.**
  - `docs/look-packs.md`: change the `frame`, `end_card` table row to: "`frame`: cabinet marquee/bezel art (checked now; shown from a later release). `end_card`: a sponsor card shown on the game-over screen where the game places an `EndCardSlot`: image (112 px tall) beside headline, body and URL (plain text, never a link). Colours from the roles `endcard`, `endcardhead`, `endcardbody`, `endcardurl`." Add a short "End card" section showing the JSON and naming `docs/sponsor-kit/` (Task 4).
  - `AGENTS.md` Look Packs paragraph: add one sentence: "A game shows sponsor end cards by declaring the four `endcard*` palette roles, inserting `EndCardStyle` (see `theme::end_card_style`) and spawning `(Node::default(), look::EndCardSlot)` on its game-over card."

- [ ] **Step 8: Commit**

```bash
git add src/ui/theme.rs src/ui/mod.rs src/ui/menu.rs docs/look-packs.md AGENTS.md
git commit -m "ui: sponsor end card on the game-over screen"
```

---

### Task 4: Sponsor kit (template `docs/sponsor-kit/`, tools)

**Files:**
- Create: `docs/sponsor-kit/README.md`, `docs/sponsor-kit/starter/look.json`, `docs/sponsor-kit/starter/logo.png`, `docs/sponsor-kit/art/logo.svg`, `tools/render_logo.sh`, `tools/make_qr.py`, `tools/requirements-kit.txt`
- Test: `tests/sponsor_kit.rs`

**Interfaces:**
- `tools/render_logo.sh <in.svg> <out.png> [height=256]` → PNG via `rsvg-convert -h <height>`; exits non-zero with a message if rsvg-convert is missing.
- `tools/make_qr.py <url> <out.png> [--size 224]` → square black-on-white QR PNG, URL must start with `https://`.

- [ ] **Step 1: Failing test** `tests/sponsor_kit.rs`:

```rust
//! The sponsor-kit starter pack is complete except for its TODO: fields.

use gamebient_game::look::validate::validate;
use gamebient_game::look::{Severity, pack};
use gamebient_game::ui::theme;

#[test]
fn the_starter_fails_only_on_its_todo_placeholders() {
    let files = pack::read_dir(std::path::Path::new("docs/sponsor-kit/starter")).expect("starter reads");
    let r = validate(&files, &theme::LOOK, Some("starter"));
    let errors: Vec<_> = r.problems.iter().filter(|p| p.severity == Severity::Error).collect();
    assert!(!errors.is_empty(), "the starter must not load as-is");
    for e in &errors {
        assert!(e.message.contains("TODO:"), "non-TODO error: {} {}", e.at, e.message);
    }
    assert!(errors.iter().any(|e| e.at == "end_card.headline"));
    assert!(
        r.problems.iter().all(|p| p.severity == Severity::Error),
        "no warnings: {:#?}",
        r.problems
    );
}
```

Run: `cargo test --test sponsor_kit` → FAIL (folder missing).

- [ ] **Step 2: Starter pack.** `docs/sponsor-kit/starter/look.json`:

```json
{
  "v": 3,
  "id": "starter",
  "name": "TODO: brand name",
  "kind": "sponsor",
  "creator": "TODO: who made this pack",
  "palette": {
    "ink": "#101010",
    "signal": "#ffffff",
    "endcard": "#101010",
    "endcardhead": "#ffffff",
    "endcardbody": "#cccccc",
    "endcardurl": "#ffffff"
  },
  "strings": {
    "title": "TODO: TITLE"
  },
  "end_card": {
    "headline": "TODO: HEADLINE",
    "body": "TODO: one line offer or thanks",
    "url": "https://example.com/TODO:",
    "image": "logo.png"
  }
}
```

`docs/sponsor-kit/art/logo.svg`: a 512x256 placeholder (rounded rect, "YOUR LOGO" in a generic sans; no real brand). Render `docs/sponsor-kit/starter/logo.png` with `tools/render_logo.sh docs/sponsor-kit/art/logo.svg docs/sponsor-kit/starter/logo.png 128`. Note: `pack::read_dir` skips `README.md`, so no README inside `starter/`.

- [ ] **Step 3: Tools.**
  - `tools/render_logo.sh` (bash, `set -euo pipefail`): checks `command -v rsvg-convert`, runs `rsvg-convert -h "${3:-256}" -o "$2" "$1"`, prints the output's size in bytes.
  - `tools/make_qr.py`: argparse `url`, `out`, `--size` (default 224); rejects non-`https://`; `import qrcode` (from `tools/requirements-kit.txt`, pinned `qrcode==8.0` plus `pillow` pinned to the version `pip index versions pillow` shows as current minus nothing — pick an exact version and pin it); `qrcode.QRCode(error_correction=ERROR_CORRECT_M, border=2)`, `make(fit=True)`, image resized to `--size` square with NEAREST; save PNG. Friendly message if `qrcode` is not installed: `pip install -r tools/requirements-kit.txt`.
  - Run both once to prove they work (make a QR into a scratch dir, delete it).

- [ ] **Step 4: `docs/sponsor-kit/README.md`** (one page, plain words):
  1. What a pitch pack is: a Look Pack, `kind: "sponsor"`, JSON + PNG, no code.
  2. What a brand can change: palette colours, strings, logo art in the game's image slots, glow (dim only), the end card, and shapes in slots the game's contract lists. What it can't: gameplay, hitboxes, scores, timing (the replay test proves it).
  3. Steps: copy `starter/` to the game's `assets/looks/<id>/`, set `id` to the folder name, fill every `TODO:` (the pack will not load until none remain), swap colours for the game's own roles (read its `src/ui/theme.rs` `PALETTE` / `STRINGS`), render the logo with `tools/render_logo.sh`, make a QR with `tools/make_qr.py` if wanted, run `cargo test --test looks`.
  4. Preview: native `--look <id>` or `GX_LOOK=<id>`; web `?look=<id>&mute=1`.
  5. Limits: point at `src/look/validate.rs` `limits` (file count, PNG size/dimensions, string lengths) and the 64 KiB web-embed cut (larger packs are fetched on demand).
  6. Before a pitch: trademark and web search on the brand name; no real brand marks unless licensed; an honest URL (`https://example.com/...` until a sponsor signs); no `TODO:` left.
  7. Example: Voidrunner's `assets/looks/fizzix/`.

- [ ] **Step 5: Run tests**

Run: `cargo test --test sponsor_kit && cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add docs/sponsor-kit tools/render_logo.sh tools/make_qr.py tools/requirements-kit.txt tests/sponsor_kit.rs
git commit -m "docs: sponsor kit with starter pack, logo and QR tools"
```

**Controller after Task 4:** final branch review, push, PR "Look Packs step 4: sponsor end card + kit", wait for CI green, merge with `gh pr merge --merge`. Then create the Voidrunner worktree.

---

### Task 5: Voidrunner adopts the end card

**Files:**
- Copy: template `src/look/` → `src/look/` (verbatim)
- Modify: `src/ui/theme.rs` (palette roles, `end_card_style()`), `src/ui/mod.rs` (insert style), `src/ui/menu.rs` (`spawn_game_over`), `src/ui/fonts.rs` tests
- Test: `tests/end_card.rs` (copy of the template's, adapted to `voidrunner::` paths and a Voidrunner pack — use `fizzix` after Task 6; in this task use an inline pack written to a temp dir or `Selection` with an embedded test pack, see below)

**Interfaces:**
- Consumes: `voidrunner::look::{EndCardSlot, EndCardStyle, EndCardPart, Part}`, `look::end_card::{ROLES, IMAGE_HEIGHT}`.
- Produces: `pub fn end_card_style() -> EndCardStyle` in `src/ui/theme.rs`; `pub fn game_over_heights(card: bool) -> Vec<f32>` (see Step 4).

- [ ] **Step 1: Copy look code.** `cp` every file of the template's `src/look/` (from `origin/main` after the template PR merged: `git -C <template> show origin/main:src/look/<f>`) over Voidrunner's `src/look/`. `diff -r` against the template must be empty. `cargo build` must pass (Voidrunner already matched the template before this step).

- [ ] **Step 2: Palette roles and style.** In Voidrunner `src/ui/theme.rs`, add to the UI (undotted) part of `PALETTE`:

```rust
    // Sponsor end card (`look::end_card`) on the game-over card.
    ("endcard", Color::srgb(0.02, 0.008, 0.07)),
    ("endcardhead", STEEL),
    ("endcardbody", MUTED),
    ("endcardurl", CYAN),
```

Add `end_card_style()` like the template's, using Voidrunner's `fonts::DISPLAY` / `fonts::BODY`, sizes headline 30, body 18, url 16, `text_width = (ui_width() * 0.62).min(520.0)`. Insert it in Voidrunner's `UiPlugin`. Any test listing UI roles (e.g. `tests/look_apply.rs` / `tests/look_ui.rs` role checks, `every_palette_role_is_used`-style tests) must pass; if a test requires every role to be read somewhere, the end-card roles are read by `look::end_card` — extend that test's known readers rather than weakening it.

- [ ] **Step 3: Slot on the game-over card.** In `spawn_game_over` (takes `look: Option<Res<ActiveLook>>` now), compute `let has_card = look.as_deref().is_some_and(|l| l.manifest.as_ref().is_some_and(|m| m.end_card.is_some()));`. Place `card.spawn((Node::default(), EndCardSlot));` after the best chip/badge, before the demo chip. When `has_card`: root `row_gap` S3→S1, card `row_gap` S2→S1, card vertical padding S4→S2, score `DISPLAY.font(96.0)`→`72.0`, and skip the `neon_rule`. Without a card everything stays exactly as today.

- [ ] **Step 4: Height budget test.** Extract the game-over vertical sizes into `pub fn game_over_heights(card: bool) -> Vec<f32>` in `menu.rs` (each entry one stacked element's height in UI px incl. its share of gaps/padding: kicker line, headline block `headline_size * LINE_HEIGHT_EM`, rule (0 with card), "SCORE" line, score line (font size × 1.2), reached line, chip (chip_node height), end card (`IMAGE_HEIGHT + 2*16` when card), demo chip when `demo::DEMO`, rail, and all gaps/paddings). Use these same constants in `spawn_game_over` so the test can't drift. Test in `menu.rs`:

```rust
    #[test]
    fn the_game_over_screen_fits_with_an_end_card() {
        let total: f32 = game_over_heights(true).iter().sum();
        assert!(total <= 720.0 - 16.0, "{total}");
        let plain: f32 = game_over_heights(false).iter().sum();
        assert!(plain <= 720.0, "{plain}");
    }
```

- [ ] **Step 5: Fit test at both widths.** In `src/ui/fonts.rs` tests (or `theme.rs` tests), for `view_w` in `[720.0, 960.0]`: build the style's column width with that view width (factor the width arithmetic into `end_card_text_width(view_w: f32) -> f32` used by `end_card_style()`), then assert that a 64-char headline, 128-char body (2 lines) and 200-char URL fitted with `look::end_card::fit` stay ≥ 6 px and their measured width (`chars * advance_em * size`) ≤ column width × lines. Also assert `IMAGE_HEIGHT + 16 + 32 + end_card_text_width(view_w) <= view_w * 0.9`.

- [ ] **Step 6: Integration test.** Copy template `tests/end_card.rs` to Voidrunner with `voidrunner::` paths. Voidrunner has no built-in pack with an end card until Task 6, so for `fills`/`swap` tests build the look from files: `ActiveLook::from_files(&theme::LOOK, files, Selection::Default, None)` with a `Files` map holding a minimal `look.json` (`{"v":3,"id":"t","name":"T","kind":"sponsor","end_card":{"headline":"H","body":"B","url":"https://example.com"}}`) and insert it as the resource after `LookPlugin` (then `app.world_mut().write_message(LookChanged)` before `update()` so the slot rebuilds). Keep the empty-slot tests on `Selection::Default`.

- [ ] **Step 7: Run CI set**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: PASS, including `tests/looks.rs` (`no_built_in_pack_changes_a_replay`).

- [ ] **Step 8: Commit**

```bash
git add src/look src/ui tests/end_card.rs
git commit -m "Sponsor end card on the game-over screen (template look sync)"
```

---

### Task 6: The FIZZIX pack (Voidrunner)

**Files:**
- Create: `tools/looks/fizzix/logo.svg`, `tools/render_logo.sh`, `tools/make_qr.py`, `tools/requirements-kit.txt` (copies of the template's), `assets/looks/fizzix/look.json`, `assets/looks/fizzix/logo.png`, `assets/looks/fizzix/qr.png`
- Modify: `tests/looks.rs` (`PACKS`, a FIZZIX test), `docs/look-packs.md`

- [ ] **Step 1: Failing test.** Add `"fizzix"` to `PACKS` in `tests/looks.rs` and:

```rust
#[test]
fn fizzix_is_a_complete_sponsor_pack() {
    let files = look::pack::embedded_files("fizzix").expect("fizzix is embedded natively");
    let r = validate(&files, &theme::LOOK, Some("fizzix"));
    assert!(r.ok() && r.problems.is_empty(), "{:#?}", r.problems);
    let m = look::manifest::parse(&files["look.json"]).unwrap();
    assert_eq!(m.kind, look::LookKind::Sponsor);
    let card = m.end_card.expect("an end card");
    assert_eq!(card.headline, "REFUEL WITH FIZZIX");
    assert_eq!(card.url.as_deref(), Some("https://example.com/fizzix"));
    assert_eq!(card.image.as_deref(), Some("qr.png"));
    assert!(m.glow.is_empty() && m.audio.is_empty());
    for slot in ["pickup.drift", "env.pad", "title.art"] {
        assert!(m.slots.contains_key(slot), "{slot}");
    }
}
```

Run: `cargo test --test looks fizzix` → FAIL.

- [ ] **Step 2: Art.**
  - `tools/looks/fizzix/logo.svg`: wordmark "FIZZIX" in heavy italic sans with a lightning-bolt "X" accent, lime `#b6ff2e` on near-black `#0b0d08`, orange `#ff7a1a` outline; transparent background; no real brand marks. Size to match the `title.art` image's existing proportions (check `fae-wilds/title.png` dimensions with `file` and `ui/look_art.rs` for how it's drawn; match its aspect).
  - `tools/render_logo.sh tools/looks/fizzix/logo.svg assets/looks/fizzix/logo.png <height matching fae's title.png>`.
  - `python3 tools/make_qr.py https://example.com/fizzix assets/looks/fizzix/qr.png --size 224`.
  - Keep `logo.png` under ~60 KB (`pngquant`/`oxipng` if available; otherwise a smaller render height).

- [ ] **Step 3: `assets/looks/fizzix/look.json`.** v3, the Global Constraints identity fields. Palette: lime `#b6ff2e`, orange `#ff7a1a`, near-black `#0b0d08` mapped onto Voidrunner's UI roles (void, night, dusk, magenta, cyan, steel, muted, card.*, rail.*, chip.*, badge.*, glitch.border, endcard*) and gameplay hue roles `pickup.drift` (lime), `env.pad` (orange), `player.glow` (lime), `env.trim`/`env.grid` (orange-ish). Read Voidrunner's `PALETTE` for exact role names; use only declared roles. Strings (each within its `max_chars`; the existing fit test enforces placement):
  - `title` "FIZZIX", `tagline` "POWER THE RUN", `item.drift` "FIZZIX CAN", `item.drift.blurb` "Grab for a power boost", `item.pad` "CAN BREAK", `hud.drift` "FIZZ"
  - phase headlines lightly branded, e.g. `phase.corridor` "THE FIZZ ZONE", `phase.combat` "SUGAR RUSH!", `phase.boss` "THE CRASH" (shorten if a limit says so).
  - Slots: `title.art` `{ "image": "logo.png", "frames": 1, "fps": 0 }`; `pickup.drift` shape — a can: `cylinder` body plus two `torus` rims at top and bottom, all inside the slot's extent (`cube(1.0)` → half-extent 1.0 per axis; check `validate.rs` for whether `extent` is half or full size and stay inside it), detail ≤ 12, within `max_vertices` 150 — if a cylinder+two tori exceed 150 vertices at the lowest detail the validator accepts, drop to cylinder + one torus and note it; `env.pad` shape — a lightning bolt of 3–5 flat `cuboid`s rotated in Y, within extent `[1.125, 0.25, 1.5]` and 100 vertices.
  - `end_card` exactly per Global Constraints.
  - No `glow`, no `audio`.

- [ ] **Step 4: Run tests**

Run: `cargo test --test looks && cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: PASS: validates clean, replay unchanged under FIZZIX, strings fit, shapes in budget. Check the pack's total size (`du -b assets/looks/fizzix` / `stat`); it is expected above the 64 KiB web-embed cut and therefore fetched on web.

- [ ] **Step 5: Docs.** `docs/look-packs.md` gets a short "Sponsor demo" section: what FIZZIX shows (title logo, branded strings, can pickup, bolt pad, end card with QR), `?look=fizzix&mute=1`, "fictional brand; run a trademark and web check before any pitch", and a pointer to the template's `docs/sponsor-kit/`.

- [ ] **Step 6: Commit**

```bash
git add tools/looks tools/render_logo.sh tools/make_qr.py tools/requirements-kit.txt assets/looks/fizzix tests/looks.rs docs/look-packs.md
git commit -m "FIZZIX Energy: fictional sponsor demo pack"
```

**Controller after Task 6:** web build (`./build.sh web`), serve `dist/` locally, open `?look=fizzix&mute=1`, screenshot title, HUD and game-over panel; send screenshots to the owner; final review; PR; CI green; merge.
