//! The demo bundle's end card (`GameState::DemoEnd`): the run stopped at the
//! content cap, not at a game over. Hosts put the buy button next to it; this
//! card names no price and no URL.

use bevy::prelude::*;

use crate::game::scoring::GameData;
use crate::ui::backdrop::spawn_backdrop;
use crate::ui::card::{S1, S2, S4, S6, card, card_with, chip_node, intro};
use crate::ui::fonts::{BODY, DISPLAY};
use crate::ui::menu::{MenuRoot, PROMPT_PULSE, TITLE, best_label};
use crate::ui::theme::{self, BADGE, CHIP, CORAL, MUTED, PAPER, RAIL, SIGNAL, TITLE_SHADOW};

pub const DEMO_HEADLINE: &str = "DEMO OVER";
pub const RESTART_PROMPT: &str = "PRESS ENTER TO RESTART";
/// TEMPLATE NOTE: name the unit the full game has more of ("MORE HOLES",
/// "EVERY SONG", "ALL FIVE SHIFTS").
pub const FULL_GAME_LINE: &str = "THE FULL GAME HAS MORE LEVELS";
/// The chip on the title card in the demo bundle.
pub const DEMO_TAG: &str = "DEMO";

/// "OWN <TITLE> TO KEEP PLAYING".
pub fn own_line(title: &str) -> String {
    format!("OWN {} TO KEEP PLAYING", title.to_ascii_uppercase())
}

/// The DEMO chip; `spawn_menu` calls it under the tagline when `demo::DEMO`.
pub fn spawn_demo_tag(parent: &mut ChildSpawnerCommands) {
    parent.spawn(card_with(&BADGE, chip_node())).with_child((
        Text::new(DEMO_TAG),
        BODY.font(16.0),
        TextColor(CORAL),
    ));
}

pub fn spawn_demo_end(mut commands: Commands, data: Res<GameData>) {
    let own = own_line(TITLE);
    let score = data.score.to_string();
    let best = best_label(data.high_score);
    commands
        .spawn((
            MenuRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(S4),
                ..default()
            },
        ))
        .with_children(|root| {
            spawn_backdrop(root, true);
            root.spawn((card(&theme::CARD), intro()))
                .with_children(|card| {
                    card.spawn((
                        Text::new(DEMO_HEADLINE),
                        DISPLAY.fitted(DEMO_HEADLINE, 40.0),
                        TextColor(CORAL),
                    ));
                    card.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(S2),
                        padding: UiRect::horizontal(Val::Px(S6)),
                        ..default()
                    })
                    .with_children(|body| {
                        body.spawn((
                            Text::new(own.clone()),
                            DISPLAY.fitted(&own, 32.0),
                            TextColor(PAPER),
                            TextShadow {
                                offset: Vec2::new(0.0, 4.0),
                                color: TITLE_SHADOW,
                            },
                        ));
                        body.spawn((
                            Text::new(FULL_GAME_LINE),
                            BODY.fitted(FULL_GAME_LINE, 18.0),
                            TextColor(SIGNAL),
                        ));
                        body.spawn((Text::new("SCORE"), BODY.font(16.0), TextColor(MUTED)));
                        body.spawn((Text::new(score), DISPLAY.font(64.0), TextColor(PAPER)));
                    });
                    card.spawn(card_with(&CHIP, chip_node())).with_child((
                        Text::new(best),
                        BODY.font(16.0),
                        TextColor(SIGNAL),
                    ));
                });
            root.spawn(card_with(
                &RAIL,
                Node {
                    padding: UiRect::axes(Val::Px(S4), Val::Px(S2)),
                    margin: UiRect::top(Val::Px(S1)),
                    ..default()
                },
            ))
            .with_child((
                Text::new(RESTART_PROMPT),
                BODY.fitted(RESTART_PROMPT, 24.0),
                TextColor(PAPER),
                PROMPT_PULSE,
            ));
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_line_names_the_game_in_caps_and_ascii() {
        assert_eq!(
            own_line("Gamebient Game"),
            "OWN GAMEBIENT GAME TO KEEP PLAYING"
        );
        assert!(own_line(crate::ui::menu::TITLE).is_ascii());
    }

    #[test]
    fn copy_is_ascii() {
        for s in [DEMO_HEADLINE, RESTART_PROMPT, FULL_GAME_LINE, DEMO_TAG] {
            assert!(s.is_ascii(), "{s}");
        }
    }
}
