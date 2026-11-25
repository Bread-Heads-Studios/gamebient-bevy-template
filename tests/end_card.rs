//! The end-card slot fills from the active pack and rebuilds on a swap.

use bevy::prelude::*;
use gamebient_game::look::{
    ActiveLook, EndCardPart, EndCardSlot, EndCardStyle, EndCardStyleMissing, LookChanged,
    LookPlugin, Part, Selection,
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
    app.add_plugins(LookPlugin {
        contract: &theme::LOOK,
    });
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
    q.iter(app.world())
        .find(|(p, _)| p.0 == part)
        .unwrap()
        .1
        .0
        .clone()
}

#[test]
fn no_pack_leaves_the_slot_empty() {
    let mut app = app(Selection::Default);
    let slot = app.world_mut().spawn((Node::default(), EndCardSlot)).id();
    app.update();
    assert!(
        app.world()
            .get::<Children>(slot)
            .is_none_or(|c| c.is_empty())
    );
    assert_eq!(
        app.world().get::<Node>(slot).unwrap().display,
        Display::None
    );
}

#[test]
fn a_pack_without_an_end_card_leaves_the_slot_empty() {
    let mut app = app(Selection::Embedded("night-shift".into()));
    let slot = app.world_mut().spawn((Node::default(), EndCardSlot)).id();
    app.update();
    assert!(
        app.world()
            .get::<Children>(slot)
            .is_none_or(|c| c.is_empty())
    );
}

#[test]
fn the_sample_sponsor_fills_the_slot() {
    let mut app = app(Selection::Embedded("sample-sponsor".into()));
    app.world_mut().spawn((Node::default(), EndCardSlot));
    app.update();
    assert_eq!(
        part_list(&mut app),
        vec![Part::Image, Part::Headline, Part::Body, Part::Url]
    );
    assert_eq!(text_of(&mut app, Part::Headline), "THANKS FOR PLAYING");
    assert_eq!(text_of(&mut app, Part::Url), "https://example.com");
    let look = app.world().resource::<ActiveLook>();
    let want = look.colour("endcardhead");
    let mut q = app.world_mut().query::<(&EndCardPart, &TextColor)>();
    let got = q
        .iter(app.world())
        .find(|(p, _)| p.0 == Part::Headline)
        .unwrap()
        .1
        .0;
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

/// A slot with no `EndCardStyle` would silently stay empty: the plugin says
/// so (once), and says nothing when the style is there or there is no slot.
#[test]
fn a_slot_without_a_style_is_warned_about_once() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.init_asset::<Image>();
    app.insert_resource(Selection::Default);
    app.add_plugins(LookPlugin {
        contract: &theme::LOOK,
    });
    app.update();
    assert!(
        app.world().get_resource::<EndCardStyleMissing>().is_none(),
        "no slot, nothing to warn about"
    );
    app.world_mut().spawn((Node::default(), EndCardSlot));
    app.update();
    assert!(app.world().get_resource::<EndCardStyleMissing>().is_some());
}

#[test]
fn a_slot_with_a_style_is_not_warned_about() {
    let mut app = app(Selection::Default);
    app.world_mut().spawn((Node::default(), EndCardSlot));
    app.update();
    app.update();
    assert!(app.world().get_resource::<EndCardStyleMissing>().is_none());
}

// --- The wrapper (`end_card_slot`) ----------------------------------------

use gamebient_game::look::{EndCardWrapper, end_card_slot};

fn display(app: &App, e: Entity) -> Display {
    app.world().get::<Node>(e).unwrap().display
}

/// The wrapper and its slot of the one `end_card_slot` in the world.
fn wrapper_and_slot(app: &mut App) -> (Entity, Entity) {
    let wrapper = app
        .world_mut()
        .query_filtered::<Entity, With<EndCardWrapper>>()
        .single(app.world())
        .unwrap();
    let slot = app
        .world_mut()
        .query_filtered::<Entity, With<EndCardSlot>>()
        .single(app.world())
        .unwrap();
    (wrapper, slot)
}

#[test]
fn the_wrapper_is_hidden_from_spawn_with_its_width() {
    let mut app = app(Selection::Default);
    app.world_mut().spawn(end_card_slot(420.0));
    let (wrapper, slot) = wrapper_and_slot(&mut app);
    // Before any frame: hidden, so a column's row_gap never sees it.
    assert_eq!(display(&app, wrapper), Display::None);
    let node = app.world().get::<Node>(wrapper).unwrap();
    assert_eq!(node.width, Val::Px(420.0));
    assert_eq!(
        app.world().get::<ChildOf>(slot).unwrap().parent(),
        wrapper,
        "the slot is the wrapper's child"
    );
    // And after frames under the default look (an empty slot).
    for _ in 0..3 {
        app.update();
        assert_eq!(display(&app, wrapper), Display::None);
        assert_eq!(display(&app, slot), Display::None);
    }
}

#[test]
fn the_wrapper_shows_when_a_sponsor_pack_fills_the_slot_and_hides_on_clear() {
    let mut app = app(Selection::Embedded("sample-sponsor".into()));
    app.world_mut().spawn(end_card_slot(420.0));
    let (wrapper, slot) = wrapper_and_slot(&mut app);
    // Hidden until the first frame has filled the slot, shown in that same
    // frame (the wrapper follows the fill before layout).
    assert_eq!(display(&app, wrapper), Display::None);
    app.update();
    assert!(!part_list(&mut app).is_empty());
    assert_eq!(display(&app, wrapper), Display::Flex);
    assert_ne!(display(&app, slot), Display::None);
    assert_eq!(
        app.world().get::<Node>(wrapper).unwrap().width,
        Val::Px(420.0),
        "showing it keeps its width"
    );

    // A clear empties the slot and hides the wrapper, in one frame.
    let contract = app.world().resource::<ActiveLook>().contract;
    app.insert_resource(ActiveLook::default_for(contract));
    app.world_mut().write_message(LookChanged);
    app.update();
    assert!(part_list(&mut app).is_empty());
    assert_eq!(display(&app, wrapper), Display::None);

    // And a pack again shows it.
    app.insert_resource(ActiveLook::resolve(
        contract,
        Selection::Embedded("sample-sponsor".into()),
    ));
    app.world_mut().write_message(LookChanged);
    app.update();
    assert_eq!(display(&app, wrapper), Display::Flex);
}

#[test]
fn a_wrapper_with_no_style_stays_hidden() {
    // No `EndCardStyle`: `fill_slots` never runs, the slot keeps its own
    // default node and no children, and the wrapper must not show.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.init_asset::<Image>();
    app.insert_resource(Selection::Embedded("sample-sponsor".into()));
    app.add_plugins(LookPlugin {
        contract: &theme::LOOK,
    });
    app.world_mut().spawn(end_card_slot(420.0));
    let (wrapper, _) = wrapper_and_slot(&mut app);
    app.update();
    assert_eq!(display(&app, wrapper), Display::None);
}

#[test]
fn a_bare_slot_still_works_without_a_wrapper() {
    let mut app = app(Selection::Embedded("sample-sponsor".into()));
    let slot = app.world_mut().spawn((Node::default(), EndCardSlot)).id();
    app.update();
    assert!(!part_list(&mut app).is_empty());
    assert_ne!(display(&app, slot), Display::None);
}

/// A real, headless UI layout: a column with a `row_gap`, a 50 px block, the
/// end card wrapper, another block. Returns the column's height in px.
fn column_height(selection: Selection) -> f32 {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        bevy::state::app::StatesPlugin,
        bevy::input::InputPlugin,
        bevy::window::WindowPlugin::default(),
        bevy::transform::TransformPlugin,
        bevy::camera::CameraPlugin,
        bevy::text::TextPlugin,
        bevy::ui::UiPlugin,
    ))
    .init_asset::<Image>()
    .init_asset::<TextureAtlasLayout>()
    .init_asset::<Mesh>()
    .init_asset::<ColorMaterial>()
    .add_plugins(bevy::picking::DefaultPickingPlugins)
    .insert_resource(selection)
    .insert_resource(style())
    .add_plugins(LookPlugin {
        contract: &theme::LOOK,
    });
    app.world_mut().spawn(Camera2d);
    let block = || Node {
        width: Val::Px(100.0),
        height: Val::Px(50.0),
        ..default()
    };
    let column = app
        .world_mut()
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(32.0),
            width: Val::Px(500.0),
            ..default()
        })
        .id();
    let first = app.world_mut().spawn(block()).id();
    let wrapper = app.world_mut().spawn(end_card_slot(480.0)).id();
    let last = app.world_mut().spawn(block()).id();
    app.world_mut()
        .entity_mut(column)
        .add_children(&[first, wrapper, last]);
    for _ in 0..4 {
        app.update();
    }
    app.world()
        .get::<bevy::ui::ComputedNode>(column)
        .unwrap()
        .size()
        .y
}

/// Under the default look the wrapper takes no space and no gap: the column
/// is exactly as tall as with no end card there (50 + 32 + 50), while a
/// sponsor card in it makes the column taller.
#[test]
fn no_gap_under_the_default_look_in_a_row_gap_column() {
    assert_eq!(column_height(Selection::Default), 132.0);
    assert!(column_height(Selection::Embedded("sample-sponsor".into())) > 132.0 + 32.0);
}
