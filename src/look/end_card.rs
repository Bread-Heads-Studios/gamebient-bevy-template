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

/// Height of the card's image, in UI pixels. Width follows the image's
/// aspect, up to `IMAGE_MAX_WIDTH`.
pub const IMAGE_HEIGHT: f32 = 112.0;
/// Widest the image may draw. A wider image is scaled down to fit, aspect
/// kept; games budget the text column against this, not the height.
pub const IMAGE_MAX_WIDTH: f32 = 2.0 * IMAGE_HEIGHT;
/// Panel fill, headline, body and URL colours. Each game declares these
/// palette roles in its contract.
pub const ROLES: [&str; 4] = ["endcard", "endcardhead", "endcardbody", "endcardurl"];
const GAP: f32 = 16.0;
const PAD: f32 = 16.0;

/// Put on an empty UI node; it is filled with the end card. The system
/// replaces the slot's `Node` on every rebuild, so games spawn
/// `(Node::default(), EndCardSlot)` and put any margins on a wrapper node.
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

/// Inserted (once) when an `EndCardSlot` exists and the game never inserted
/// an `EndCardStyle`: the slot would silently stay empty and a pack's
/// sponsor card never show. `warn_missing_style` logs a warning when it
/// inserts this; tests read it.
#[derive(Resource, Debug, Clone, Copy)]
pub struct EndCardStyleMissing;

/// Warns once per run about a slot with no `EndCardStyle`.
pub fn warn_missing_style(mut commands: Commands, slots: Query<(), With<EndCardSlot>>) {
    if !slots.is_empty() {
        warn!(
            "an EndCardSlot exists but the game never inserted an EndCardStyle \
             (theme::end_card_style()): a pack's sponsor end card will not show"
        );
        commands.insert_resource(EndCardStyleMissing);
    }
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

/// The drawn size of an image of `w` x `h` pixels: `IMAGE_HEIGHT` tall, at
/// most `IMAGE_MAX_WIDTH` wide, aspect kept (a wider image is scaled down
/// as a whole, never squashed). Bevy clamps each axis of a node
/// independently, so the size is resolved here and both axes are explicit.
pub fn image_box(w: f32, h: f32) -> (f32, f32) {
    if w <= 0.0 || h <= 0.0 {
        return (IMAGE_HEIGHT, IMAGE_HEIGHT);
    }
    let scale = (IMAGE_HEIGHT / h).min(IMAGE_MAX_WIDTH / w);
    (w * scale, h * scale)
}

/// Largest whole size up to `max_size` at which `text` fits `lines` lines
/// of `width` (the same arithmetic as `ui::fit`, which this folder may not
/// name).
pub fn fit(advance_em: f32, text: &str, max_size: f32, width: f32, lines: f32) -> f32 {
    let glyphs = text.chars().count().max(1) as f32;
    (width * lines / (glyphs * advance_em))
        .min(max_size)
        .floor()
        .max(1.0)
}

/// One text line of the card: its tag, content, fitted font and colour.
struct Line<'a> {
    part: Part,
    text: &'a str,
    base: &'a TextFont,
    advance_em: f32,
    size: f32,
    lines: f32,
    role: &'static str,
}

fn line_bundle(
    l: Line,
    style: &EndCardStyle,
    look: &ActiveLook,
) -> (EndCardPart, Text, TextFont, TextColor) {
    let size = fit(l.advance_em, l.text, l.size, style.text_width, l.lines);
    (
        EndCardPart(l.part),
        Text::new(l.text),
        TextFont {
            font_size: size,
            ..l.base.clone()
        },
        TextColor(look.colour(l.role)),
    )
}

pub fn fill_slots(
    mut commands: Commands,
    look: Res<ActiveLook>,
    assets: Res<LookAssets>,
    style: Res<EndCardStyle>,
    images: Option<Res<Assets<Image>>>,
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
        let Some(card) = look.manifest.as_ref().and_then(|m| m.end_card.as_ref()) else {
            e.insert((
                Node {
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ));
            continue;
        };
        let image = card.image.as_deref().and_then(|f| assets.image(f));
        let shown = parts(card, image.is_some());
        e.insert((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(GAP),
                padding: UiRect::all(Val::Px(PAD)),
                border_radius: BorderRadius::all(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(look.colour(ROLES[0])),
        ));
        e.with_children(|row| {
            if let Some(handle) = image.filter(|_| shown.contains(&Part::Image)) {
                let node = match images.as_ref().and_then(|a| a.get(&handle)) {
                    Some(img) => {
                        let (w, h) = image_box(img.width() as f32, img.height() as f32);
                        Node {
                            width: Val::Px(w),
                            height: Val::Px(h),
                            ..default()
                        }
                    }
                    // Size unknown: height fixed, width follows the image, capped.
                    None => Node {
                        height: Val::Px(IMAGE_HEIGHT),
                        max_width: Val::Px(IMAGE_MAX_WIDTH),
                        ..default()
                    },
                };
                row.spawn((EndCardPart(Part::Image), ImageNode::new(handle), node));
            }
            row.spawn(Node {
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                width: Val::Px(style.text_width),
                row_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|col| {
                col.spawn(line_bundle(
                    Line {
                        part: Part::Headline,
                        text: &card.headline,
                        base: &style.display,
                        advance_em: style.display_advance_em,
                        size: style.headline_size,
                        lines: 1.0,
                        role: ROLES[1],
                    },
                    &style,
                    &look,
                ));
                if let Some(body) = &card.body {
                    col.spawn(line_bundle(
                        Line {
                            part: Part::Body,
                            text: body,
                            base: &style.body,
                            advance_em: style.body_advance_em,
                            size: style.body_size,
                            lines: 2.0,
                            role: ROLES[2],
                        },
                        &style,
                        &look,
                    ));
                }
                if let Some(url) = &card.url {
                    col.spawn(line_bundle(
                        Line {
                            part: Part::Url,
                            text: url,
                            base: &style.body,
                            advance_em: style.body_advance_em,
                            size: style.url_size,
                            lines: 1.0,
                            role: ROLES[3],
                        },
                        &style,
                        &look,
                    ));
                }
            });
        });
    }
}

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
    fn image_box_keeps_aspect_within_the_height_and_max_width() {
        assert_eq!(image_box(128.0, 64.0), (224.0, 112.0));
        assert_eq!(image_box(100.0, 100.0), (112.0, 112.0));
        // Wider than 2:1: scaled down as a whole, aspect kept.
        let (w, h) = image_box(400.0, 100.0);
        assert_eq!(w, IMAGE_MAX_WIDTH);
        assert!((w / h - 4.0).abs() < 1e-4 && h < IMAGE_HEIGHT);
        assert_eq!(image_box(0.0, 0.0), (IMAGE_HEIGHT, IMAGE_HEIGHT));
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
        assert_eq!(
            parts(&card(false, true, true), true),
            vec![Part::Image, Part::Headline, Part::Url]
        );
        assert_eq!(
            parts(&card(true, false, true), true),
            vec![Part::Image, Part::Headline, Part::Body]
        );
        assert_eq!(
            parts(&card(true, true, false), true),
            vec![Part::Headline, Part::Body, Part::Url]
        );
        // Named but not decoded (headless, or a bad PNG): no image part.
        assert_eq!(
            parts(&card(true, true, true), false),
            vec![Part::Headline, Part::Body, Part::Url]
        );
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
