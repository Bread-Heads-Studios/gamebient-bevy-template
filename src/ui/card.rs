//! The card primitive: one themed surface for every panel a game shows
//! (game over, pause, level intros, how-to-play panels, banners).
//!
//! A game picks its look once, as `CardStyle` consts in `src/ui/theme.rs`
//! (paper, chalkboard, plaque, glass, label...), and every card is built
//! from [`card`] or [`card_with`], so they share a border, radius, shadow
//! and an 8 px spacing scale. [`CardIntro`] adds the house entrance: an
//! ease-out scale and rise over at most 250 ms.
//!
//! [`CardRoles`] + [`CardLookExt::card_look`] tag a card for Look Packs
//! (fill, gradient, border, shadow from palette roles), only for the parts
//! its style has.
//!
//! Copied verbatim into games (it names `src/look/`'s tags); theme in
//! `theme.rs`, not here.

use bevy::ecs::system::EntityCommands;
use bevy::prelude::*;

use crate::look::{LookBorder, LookColour, LookGradient, LookShadow, LookTint};

/// The 8 px spacing scale. Paddings, gaps and margins use these.
pub const S1: f32 = 8.0;
pub const S2: f32 = 16.0;
pub const S3: f32 = 24.0;
pub const S4: f32 = 32.0;
pub const S6: f32 = 48.0;

/// `steps` units of the 8 px scale, as a `Val`.
pub const fn space(steps: f32) -> Val {
    Val::Px(8.0 * steps)
}

/// Shadow geometry shared by every card: a soft drop below the surface.
pub const SHADOW_Y: f32 = 8.0;
pub const SHADOW_BLUR: f32 = 28.0;

/// A game's card look. Build these as consts in `theme.rs`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardStyle {
    /// Flat fill, drawn under the gradient (and alone when there is none).
    pub fill: Color,
    /// Optional top-to-bottom gradient over the fill.
    pub gradient: Option<[Color; 2]>,
    /// Border colour: the game's accent.
    pub border: Color,
    /// Border width in UI pixels (the house range is 2-3).
    pub border_px: f32,
    /// Corner radius in UI pixels.
    pub radius: f32,
    /// Drop-shadow colour, or `None` for a flat card.
    pub shadow: Option<Color>,
}

/// The default card layout: a centred column padded on the spacing scale.
pub fn card_node() -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        padding: UiRect::axes(Val::Px(S4), Val::Px(S3)),
        row_gap: Val::Px(S2),
        ..default()
    }
}

/// Layout for a pill (chip, badge): one line, tight padding.
pub fn chip_node() -> Node {
    Node {
        align_items: AlignItems::Center,
        padding: UiRect::axes(Val::Px(S2), Val::Px(S1 / 2.0)),
        ..default()
    }
}

/// A card with the default layout ([`card_node`]).
pub fn card(style: &CardStyle) -> impl Bundle {
    card_with(style, card_node())
}

/// A card with a layout of your own; the style sets its border and radius.
pub fn card_with(style: &CardStyle, mut node: Node) -> impl Bundle {
    node.border = UiRect::all(Val::Px(style.border_px));
    node.border_radius = BorderRadius::all(Val::Px(style.radius));
    let gradient = style.gradient.map_or_else(Vec::new, |[top, bottom]| {
        vec![Gradient::from(LinearGradient::to_bottom(vec![
            ColorStop::new(top, Val::Percent(0.0)),
            ColorStop::new(bottom, Val::Percent(100.0)),
        ]))]
    });
    let shadow = style.shadow.map_or_else(
        || BoxShadow(Vec::new()),
        |color| {
            BoxShadow::new(
                color,
                Val::Px(0.0),
                Val::Px(SHADOW_Y),
                Val::Px(0.0),
                Val::Px(SHADOW_BLUR),
            )
        },
    );
    (
        node,
        BackgroundColor(style.fill),
        BackgroundGradient(gradient),
        BorderColor::all(style.border),
        shadow,
    )
}

/// The palette roles a [`CardStyle`]'s parts take under a Look Pack. Build
/// these as consts in `theme.rs` next to the style, each tint's alpha the
/// style's own so the role's default reproduces the style exactly (a role
/// whose default is the style's colour needs no alpha when the alphas
/// agree).
///
/// - `fill`: `BackgroundColor` (a `LookColour`; `alpha` and `keep_alpha`
///   are used, the lightening is not).
/// - `border`: `BorderColor`, all four sides.
/// - `gradient`: the two stops (top, bottom), for a style with a gradient.
/// - `shadow`: the colour of the card's `BoxShadow`, for a style with one.
///
/// A part the style lacks gets no tag whatever the roles say, and a part
/// the roles leave out keeps the style's colour (no tag either), so a card
/// never carries a tag that cannot do anything.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardRoles {
    pub fill: LookTint,
    pub border: LookTint,
    pub gradient: Option<[LookTint; 2]>,
    pub shadow: Option<LookTint>,
}

impl CardRoles {
    /// A card with a fill and a border.
    pub const fn new(fill: LookTint, border: LookTint) -> Self {
        Self {
            fill,
            border,
            gradient: None,
            shadow: None,
        }
    }
    pub const fn gradient(mut self, top: LookTint, bottom: LookTint) -> Self {
        self.gradient = Some([top, bottom]);
        self
    }
    pub const fn shadow(mut self, shadow: LookTint) -> Self {
        self.shadow = Some(shadow);
        self
    }
}

/// The look tags of one card: always a fill and a border, a gradient and a
/// shadow only where the style has one and the roles name one.
#[derive(Clone, Debug, PartialEq)]
pub struct CardLook {
    pub fill: LookColour,
    pub border: LookBorder,
    pub gradient: Option<LookGradient>,
    pub shadow: Option<LookShadow>,
}

/// The tags for a card drawn with `style` (`card_with`), coloured from
/// `roles`.
pub fn card_look(style: &CardStyle, roles: &CardRoles) -> CardLook {
    CardLook {
        fill: roles.fill.into(),
        border: LookBorder(roles.border),
        gradient: style
            .gradient
            .and(roles.gradient)
            .map(|[top, bottom]| LookGradient::two(top, bottom)),
        shadow: style.shadow.and(roles.shadow).map(LookShadow),
    }
}

/// Adds a card's look tags to the entity just spawned with
/// [`card`] / [`card_with`]:
///
/// ```ignore
/// commands
///     .spawn((card_with(&CARD, node), intro()))
///     .card_look(&CARD, &CARD_ROLES)
///     .with_children(|card| { /* ... */ });
/// ```
///
/// Not a bundle on purpose: a bundle cannot leave a tag out, and a
/// `LookGradient` on a card without a gradient (or a `LookShadow` without
/// a shadow) is a tag that does nothing.
pub trait CardLookExt {
    fn card_look(&mut self, style: &CardStyle, roles: &CardRoles) -> &mut Self;
}

impl CardLookExt for EntityCommands<'_> {
    fn card_look(&mut self, style: &CardStyle, roles: &CardRoles) -> &mut Self {
        let tags = card_look(style, roles);
        self.insert((tags.fill, tags.border));
        if let Some(g) = tags.gradient {
            self.insert(g);
        }
        if let Some(s) = tags.shadow {
            self.insert(s);
        }
        self
    }
}

impl CardLookExt for EntityWorldMut<'_> {
    fn card_look(&mut self, style: &CardStyle, roles: &CardRoles) -> &mut Self {
        let tags = card_look(style, roles);
        self.insert((tags.fill, tags.border));
        if let Some(g) = tags.gradient {
            self.insert(g);
        }
        if let Some(s) = tags.shadow {
            self.insert(s);
        }
        self
    }
}

/// The house card entrance: scale up from `from_scale` and rise `rise_px`
/// into place with an ease-out over `duration` seconds (at most 0.25).
///
/// Presentation only: driven by `Time<Real>` in `animate_card_intro`, and it
/// animates `UiTransform`, never `font_size` (every new size allocates a
/// glyph atlas that is never freed).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct CardIntro {
    pub elapsed: f32,
    pub duration: f32,
    pub from_scale: f32,
    pub rise_px: f32,
}

/// Longest card entrance the house rules allow, in seconds.
pub const MAX_INTRO_SECS: f32 = 0.25;

impl Default for CardIntro {
    fn default() -> Self {
        Self {
            elapsed: 0.0,
            duration: 0.22,
            from_scale: 0.94,
            rise_px: 18.0,
        }
    }
}

impl CardIntro {
    /// Progress through the entrance, 0 to 1, eased out (cubic).
    pub fn eased(&self) -> f32 {
        let duration = self.duration.clamp(f32::EPSILON, MAX_INTRO_SECS);
        let t = (self.elapsed / duration).clamp(0.0, 1.0);
        1.0 - (1.0 - t).powi(3)
    }

    pub fn is_done(&self) -> bool {
        self.eased() >= 1.0
    }

    /// The card's pose at this point of the entrance.
    pub fn transform(&self) -> UiTransform {
        let e = self.eased();
        UiTransform {
            translation: Val2::px(0.0, self.rise_px * (1.0 - e)),
            scale: Vec2::splat(self.from_scale + (1.0 - self.from_scale) * e),
            ..default()
        }
    }
}

/// [`CardIntro`] plus its first-frame pose, so the card never flashes at
/// full size before the animation system sees it.
pub fn intro() -> impl Bundle {
    let intro = CardIntro::default();
    (intro, intro.transform())
}

/// Advances every unfinished [`CardIntro`]. Runs in `Update` (UiPlugin).
pub fn animate_card_intro(
    time: Res<Time<Real>>,
    mut cards: Query<(&mut CardIntro, &mut UiTransform)>,
) {
    let dt = time.delta_secs();
    for (mut intro, mut transform) in &mut cards {
        if intro.is_done() {
            continue;
        }
        intro.elapsed += dt;
        *transform = intro.transform();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spacing_is_on_the_8px_scale() {
        for s in [S1, S2, S3, S4, S6] {
            assert_eq!(s % 8.0, 0.0);
        }
        assert_eq!(space(3.0), Val::Px(S3));
    }

    #[test]
    fn intro_starts_small_and_low_and_ends_at_rest() {
        let mut intro = CardIntro::default();
        let start = intro.transform();
        assert_eq!(start.scale, Vec2::splat(intro.from_scale));
        assert_eq!(start.translation, Val2::px(0.0, intro.rise_px));
        intro.elapsed = intro.duration;
        assert!(intro.is_done());
        assert_eq!(intro.transform(), UiTransform::IDENTITY);
    }

    #[test]
    fn intro_eases_out_and_never_overshoots() {
        let mut intro = CardIntro::default();
        let mut last = 0.0;
        for step in 1..=30 {
            intro.elapsed = step as f32 * 0.01;
            let e = intro.eased();
            assert!(e >= last && e <= 1.0);
            last = e;
        }
        // Ease-out: more than half the travel in the first third.
        intro.elapsed = intro.duration / 3.0;
        assert!(intro.eased() > 0.5);
    }

    #[test]
    fn intro_is_capped_at_250ms() {
        let intro = CardIntro {
            elapsed: MAX_INTRO_SECS,
            duration: 2.0,
            ..default()
        };
        assert!(intro.is_done());
        assert!(CardIntro::default().duration <= MAX_INTRO_SECS);
    }

    #[test]
    fn card_with_applies_border_and_radius() {
        let style = CardStyle {
            fill: Color::BLACK,
            gradient: None,
            border: Color::WHITE,
            border_px: 3.0,
            radius: 12.0,
            shadow: None,
        };
        let mut world = World::new();
        let e = world.spawn(card(&style)).id();
        let node = world.get::<Node>(e).unwrap();
        assert_eq!(node.border, UiRect::all(Val::Px(3.0)));
        assert_eq!(node.border_radius, BorderRadius::all(Val::Px(12.0)));
        assert!(world.get::<BoxShadow>(e).unwrap().0.is_empty());
        assert!(world.get::<BackgroundGradient>(e).unwrap().0.is_empty());
    }

    fn style(gradient: bool, shadow: bool) -> CardStyle {
        CardStyle {
            fill: Color::BLACK,
            gradient: gradient.then_some([Color::BLACK, Color::WHITE]),
            border: Color::WHITE,
            border_px: 2.0,
            radius: 8.0,
            shadow: shadow.then_some(Color::BLACK),
        }
    }

    fn full_roles() -> CardRoles {
        CardRoles::new(LookTint::new("a"), LookTint::new("b").alpha(0.5))
            .gradient(LookTint::new("c"), LookTint::new("d"))
            .shadow(LookTint::new("e"))
    }

    /// The tags `card_look` puts on a card, by entity, through `Commands`.
    fn tags_on(style: &CardStyle, roles: &CardRoles) -> (bool, bool, bool, bool) {
        let mut world = World::new();
        let e = world
            .commands()
            .spawn(card(style))
            .card_look(style, roles)
            .id();
        world.flush();
        (
            world.get::<LookColour>(e).is_some(),
            world.get::<LookBorder>(e).is_some(),
            world.get::<LookGradient>(e).is_some(),
            world.get::<LookShadow>(e).is_some(),
        )
    }

    #[test]
    fn card_look_tags_only_the_parts_the_style_has() {
        let full = full_roles();
        assert_eq!(tags_on(&style(true, true), &full), (true, true, true, true));
        // No gradient in the style: no gradient tag, though the roles name one.
        assert_eq!(
            tags_on(&style(false, true), &full),
            (true, true, false, true)
        );
        // No shadow in the style: no shadow tag.
        assert_eq!(
            tags_on(&style(true, false), &full),
            (true, true, true, false)
        );
        assert_eq!(
            tags_on(&style(false, false), &full),
            (true, true, false, false)
        );
        // Roles that leave a part out keep the style's colour: no tag.
        let bare = CardRoles::new(LookTint::new("a"), LookTint::new("b"));
        assert_eq!(
            tags_on(&style(true, true), &bare),
            (true, true, false, false)
        );
    }

    #[test]
    fn card_look_carries_each_part_its_role_and_alpha() {
        let tags = card_look(&style(true, true), &full_roles());
        assert_eq!(tags.fill.role, "a");
        assert_eq!(tags.border, LookBorder::new("b").alpha(0.5));
        assert_eq!(
            tags.gradient,
            Some(LookGradient::two(LookTint::new("c"), LookTint::new("d")))
        );
        assert_eq!(tags.shadow, Some(LookShadow::new("e")));
        // The fill keeps the tint's alpha and keep_alpha.
        let fill = CardRoles::new(
            LookTint::new("a").alpha(0.3).keep_alpha(),
            LookTint::new("b"),
        );
        let f = card_look(&style(false, false), &fill).fill;
        assert_eq!((f.alpha, f.keep_alpha), (Some(0.3), true));
    }

    #[test]
    fn card_look_works_on_a_world_entity_too() {
        let mut world = World::new();
        let s = style(true, true);
        let mut e = world.spawn(card(&s));
        e.card_look(&s, &full_roles());
        let e = e.id();
        assert!(world.get::<LookGradient>(e).is_some());
        assert!(world.get::<LookShadow>(e).is_some());
    }
}
