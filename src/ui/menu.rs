use bevy::prelude::*;

use crate::game::audio::SfxEvent;
use crate::game::input::GameInput;
use crate::game::scoring::GameData;
use crate::game::states::GameState;
use crate::ui::backdrop::spawn_backdrop;
use crate::ui::card::{S1, S2, S4, S6, card, card_with, chip_node, intro};
use crate::ui::fonts::{BODY, DISPLAY};
use crate::ui::how_to_play::{SeenHowToPlay, start_target};
use crate::ui::theme::{self, BADGE, CHIP, CORAL, MUTED, PAPER, RAIL, SIGNAL, TITLE_SHADOW};
use crate::ui::transition::{Pulse, ScreenFade};

#[derive(Component)]
pub struct MenuRoot;

/// Title text, sized to fit the game's width whatever its ratio.
pub const TITLE: &str = "GAMEBIENT GAME";
/// One line under the title. TEMPLATE NOTE: the game's own tagline.
pub const TAGLINE: &str = "A COLECOVISION GX CARTRIDGE";
/// The start prompt always names the button. TEMPLATE NOTE: a themed verb
/// ("PRESS ENTER - CLOCK IN").
pub const START_PROMPT: &str = "PRESS ENTER TO PLAY";
/// Controls line under the start prompt. ASCII only.
pub const CONTROLS: &str = "Arrows / WASD: Move  |  Z: A  X: B  |  Esc: Pause";
/// Game-over headline. TEMPLATE NOTE: a themed artefact's title ("SHIFT
/// REPORT", "LAST SLICE").
pub const GAME_OVER_HEADLINE: &str = "GAME OVER";
pub const CONTINUE_PROMPT: &str = "PRESS ENTER TO CONTINUE";

/// The shared prompt pulse: about 0.5 Hz, never fully gone.
pub const PROMPT_PULSE: Pulse = Pulse {
    speed: 3.0,
    min: 0.35,
    max: 1.0,
};

/// Text of the best-score chip.
pub fn best_label(best: u32) -> String {
    if best == 0 {
        "BEST --".to_string()
    } else {
        format!("BEST {best}")
    }
}

/// Spawns the title screen, in the house anatomy: an animated backdrop, the
/// lockup in the upper ~45%, and the prompt rail on a plate at the bottom.
///
/// TEMPLATE NOTE: artwork variant: if your game has full-bleed title art
/// (logotype baked in), replace the lockup's text with an `ImageNode` and
/// preload the handle at Startup so it never pops in:
///
/// ```ignore
/// #[derive(Resource)]
/// pub struct TitleArtwork(pub Handle<Image>);
/// pub fn preload_title_artwork(mut commands: Commands, assets: Res<AssetServer>) {
///     commands.insert_resource(TitleArtwork(assets.load("title.png")));
/// }
/// ```
pub fn spawn_menu(mut commands: Commands, data: Res<GameData>) {
    let best = best_label(data.high_score);
    commands
        .spawn((
            MenuRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
        ))
        .with_children(|root| {
            spawn_backdrop(root, false);

            // Lockup: display face with depth, an accent rule, one tagline.
            root.spawn(Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(8.0),
                width: Val::Percent(100.0),
                height: Val::Percent(44.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(S2),
                ..default()
            })
            .with_children(|lockup| {
                lockup.spawn((
                    Text::new(TITLE),
                    DISPLAY.fitted(TITLE, 88.0),
                    TextColor(PAPER),
                    TextShadow {
                        offset: Vec2::new(0.0, 6.0),
                        color: TITLE_SHADOW,
                    },
                ));
                lockup.spawn((
                    Node {
                        width: Val::Px(S6 + S2),
                        height: Val::Px(4.0),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BackgroundColor(CORAL),
                ));
                lockup.spawn((
                    Text::new(TAGLINE),
                    BODY.fitted(TAGLINE, 20.0),
                    TextColor(SIGNAL),
                ));
            });

            // Prompt rail: themed plate, pulsing verb prompt, one controls
            // line and the best score.
            root.spawn(Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(S4),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|rail_row| {
                rail_row
                    .spawn(card_with(
                        &RAIL,
                        Node {
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(Val::Px(S4), Val::Px(S2)),
                            row_gap: Val::Px(S1 + 4.0),
                            ..default()
                        },
                    ))
                    .with_children(|rail| {
                        rail.spawn((
                            Text::new(START_PROMPT),
                            BODY.fitted(START_PROMPT, 28.0),
                            TextColor(PAPER),
                            PROMPT_PULSE,
                        ));
                        rail.spawn(Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(S2),
                            ..default()
                        })
                        .with_children(|line| {
                            line.spawn(card_with(&CHIP, chip_node())).with_child((
                                Text::new(best),
                                BODY.font(15.0),
                                TextColor(SIGNAL),
                            ));
                            line.spawn((
                                Text::new(CONTROLS),
                                BODY.fitted(CONTROLS, 16.0),
                                TextColor(MUTED),
                            ));
                        });
                    });
            });
        });
}

/// Spawns the game-over screen: the backdrop dimmed, and a card with the
/// score as the hero number and the best as a chip (or a NEW BEST badge).
///
/// TEMPLATE NOTE: make this the game's themed artefact (a receipt, a
/// scorecard, a headstone) by restyling the card in `theme.rs` and the copy
/// above; keep the score the hero.
pub fn spawn_game_over(mut commands: Commands, data: Res<GameData>) {
    let new_best = data.score > 0 && data.score >= data.high_score;
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
                        Text::new(GAME_OVER_HEADLINE),
                        DISPLAY.fitted(GAME_OVER_HEADLINE, 40.0),
                        TextColor(CORAL),
                    ));
                    card.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        padding: UiRect::horizontal(Val::Px(S6)),
                        ..default()
                    })
                    .with_children(|hero| {
                        hero.spawn((Text::new("SCORE"), BODY.font(16.0), TextColor(MUTED)));
                        hero.spawn((
                            Text::new(score),
                            DISPLAY.font(112.0),
                            TextColor(PAPER),
                            TextShadow {
                                offset: Vec2::new(0.0, 6.0),
                                color: TITLE_SHADOW,
                            },
                        ));
                    });
                    if new_best {
                        card.spawn(card_with(&BADGE, chip_node())).with_child((
                            Text::new("NEW BEST"),
                            BODY.font(16.0),
                            TextColor(CORAL),
                        ));
                    } else {
                        card.spawn(card_with(&CHIP, chip_node())).with_child((
                            Text::new(best),
                            BODY.font(16.0),
                            TextColor(SIGNAL),
                        ));
                    }
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
                Text::new(CONTINUE_PROMPT),
                BODY.fitted(CONTINUE_PROMPT, 24.0),
                TextColor(PAPER),
                PROMPT_PULSE,
            ));
        });
}

pub fn despawn_menu(mut commands: Commands, query: Query<Entity, With<MenuRoot>>) {
    for e in &query {
        commands.entity(e).despawn();
    }
}

/// ENTER routes `Menu -> HowToPlay/Playing` (once-per-session gate) and
/// `GameOver -> Menu`, always through the fade.
pub fn menu_input(
    input: Res<GameInput>,
    state: Res<State<GameState>>,
    seen: Res<SeenHowToPlay>,
    mut fade: ResMut<ScreenFade>,
    mut sfx: MessageWriter<SfxEvent>,
) {
    // Canon Confirm: Enter / Space / Z, any face button, pad Start or A.
    if !fade.is_idle() || !input.confirm_just_pressed {
        return;
    }
    let target = match state.get() {
        GameState::Menu => Some(start_target(seen.0)),
        GameState::GameOver => Some(GameState::Menu),
        _ => None,
    };
    if let Some(t) = target
        && fade.request(t)
    {
        sfx.write(SfxEvent::Confirm);
    }
}
