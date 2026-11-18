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
