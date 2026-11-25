//! Look tags for the parts of a UI node `LookColour` does not reach: its
//! `BorderColor` ([`LookBorder`]), the stops of its `BackgroundGradient`
//! ([`LookGradient`]), its `ImageNode` tint ([`LookImage`]) and its shadows
//! (`TextShadow` / `BoxShadow`, [`LookShadow`]). They follow `LookColour`'s rules: spawn code puts
//! the tag next to the game's own colours, and the systems here (in
//! `LookSet`, `PostUpdate`) write the active pack's colours on the frame
//! the tag spawns and again on every `LookChanged`, which a clear
//! (`look.clear`, `look.preview.clear`) also sends. With no pack every
//! colour is the role's default, so a tag whose role defaults equal the
//! spawned literals changes nothing. Presentation only: the sim never
//! queries these.
//!
//! A card (`ui::card::card_with`: fill, gradient, border) takes three tags:
//!
//! ```ignore
//! commands.spawn((
//!     card_with(&CARD, node),
//!     LookColour::new("card"),
//!     LookBorder::new("rim").alpha(0.9),
//!     LookGradient::two(LookTint::new("cardtop"), LookTint::new("cardbottom")),
//! ));
//! ```

use bevy::prelude::*;

use super::apply::LookChanged;
use super::runtime::ActiveLook;

/// A palette role and what a tag writes for it: one colour. Every tag here
/// (`LookBorder`, `LookImage`, `LookShadow`, `LookGradient` stops) holds
/// one, and `LookColour` follows the same two opt-ins (`keep_alpha`; the
/// lightening is `LookTint` only).
///
/// - [`alpha`](Self::alpha): the colour is written at this alpha; without
///   it the role's own alpha is kept, as `LookColour` does.
/// - [`keep_alpha`](Self::keep_alpha): retint **RGB only** and leave
///   whatever alpha the component has now. For an entity whose alpha a game
///   system animates (a score pop, a banner, a fade) under a static tag:
///   without it a look change repaints the role at full alpha for a frame.
///   It wins over `alpha`. The spawn-time alpha (the game's own) is the one
///   kept on the frame the tag is added.
/// - [`toward_white`](Self::toward_white): the role colour mixed `w` (0 to
///   1) of the way toward white in sRGB, then the alpha rule: a lightened
///   stop (the light centre of a horizon line, a gloss).
/// - [`default_is`](Self::default_is): the literal colour the game spawns
///   this part with. When the active look's colour for the role **is** the
///   contract default (the default look, or a pack that leaves the role
///   alone), exactly that literal is written, with no float round trip
///   through the lightening and the alpha, so the default look stays
///   bit-identical to the spawn code. Without it a lightened tint computes
///   its default colour from the role.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LookTint {
    pub role: &'static str,
    pub alpha: Option<f32>,
    pub white: f32,
    pub keep_alpha: bool,
    pub default: Option<Color>,
}

impl LookTint {
    pub const fn new(role: &'static str) -> Self {
        Self {
            role,
            alpha: None,
            white: 0.0,
            keep_alpha: false,
            default: None,
        }
    }
    pub const fn alpha(mut self, alpha: f32) -> Self {
        self.alpha = Some(alpha);
        self
    }
    /// Retint RGB only; the component's own alpha is kept (see the type).
    pub const fn keep_alpha(mut self) -> Self {
        self.keep_alpha = true;
        self
    }
    /// The role colour mixed `w` of the way toward white (sRGB, `0.0..=1.0`).
    pub const fn toward_white(mut self, w: f32) -> Self {
        self.white = w;
        self
    }
    /// The literal the game spawns this part with: written as is while the
    /// role is at its contract default.
    pub const fn default_is(mut self, colour: Color) -> Self {
        self.default = Some(colour);
        self
    }

    /// The colour this tint wants, ignoring `keep_alpha` (there is no
    /// current alpha to keep): the role colour, lightened, at `alpha`.
    pub fn resolve(&self, look: &ActiveLook) -> Color {
        let c = look.colour(self.role);
        if let Some(d) = self.default
            && look.contract.default_colour(self.role) == Some(c)
        {
            return d;
        }
        let c = if self.white > 0.0 {
            let s = c.to_srgba();
            let w = self.white.min(1.0);
            Color::srgba(
                s.red + (1.0 - s.red) * w,
                s.green + (1.0 - s.green) * w,
                s.blue + (1.0 - s.blue) * w,
                s.alpha,
            )
        } else {
            c
        };
        self.alpha.map_or(c, |a| c.with_alpha(a))
    }

    /// What to write over a component currently holding `current`: as
    /// [`resolve`](Self::resolve), with `current`'s alpha under `keep_alpha`.
    pub fn resolve_over(&self, look: &ActiveLook, current: Color) -> Color {
        let c = self.resolve(look);
        if self.keep_alpha {
            c.with_alpha(current.alpha())
        } else {
            c
        }
    }
}

/// Colours all four sides of the entity's `BorderColor` from a role.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct LookBorder(pub LookTint);

impl LookBorder {
    pub const fn new(role: &'static str) -> Self {
        Self(LookTint::new(role))
    }
    pub const fn alpha(self, alpha: f32) -> Self {
        Self(self.0.alpha(alpha))
    }
    /// Keep the component's own alpha (`LookTint::keep_alpha`).
    pub const fn keep_alpha(self) -> Self {
        Self(self.0.keep_alpha())
    }
}

/// Tints the entity's `ImageNode::color` from a role. `LookColour` does not
/// reach it on purpose: a button carries an image and a background, and each
/// has its own tag. Do not tag an image whose colour a system writes from
/// state (a lit or spent heart): that system reads the role itself.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct LookImage(pub LookTint);

impl LookImage {
    pub const fn new(role: &'static str) -> Self {
        Self(LookTint::new(role))
    }
    pub const fn alpha(self, alpha: f32) -> Self {
        Self(self.0.alpha(alpha))
    }
    /// Keep the component's own alpha (`LookTint::keep_alpha`).
    pub const fn keep_alpha(self) -> Self {
        Self(self.0.keep_alpha())
    }
}

/// Colours the entity's shadow from a role: `TextShadow::color`, and the
/// colour of every `ShadowStyle` in a `BoxShadow`, whichever the entity has
/// (offsets, blur and spread stay as spawned). One tag, one role: an entity
/// with both shadows in different colours takes the colour a game system
/// writes instead.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct LookShadow(pub LookTint);

impl LookShadow {
    pub const fn new(role: &'static str) -> Self {
        Self(LookTint::new(role))
    }
    pub const fn alpha(self, alpha: f32) -> Self {
        Self(self.0.alpha(alpha))
    }
    /// Keep the component's own alpha (`LookTint::keep_alpha`).
    pub const fn keep_alpha(self) -> Self {
        Self(self.0.keep_alpha())
    }
}

/// Colours the stops of the entity's `BackgroundGradient`: stop `i` of
/// every gradient in it (linear, radial or conic) takes entry `i`. `None`,
/// or a stop past the end of the list, keeps its spawned colour (a
/// structural near-black stop, say). Only colours are written; stop
/// positions, angles and the gradient's shape stay as spawned.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct LookGradient(pub Vec<Option<LookTint>>);

impl LookGradient {
    /// One entry per stop, in order.
    pub fn new(stops: impl IntoIterator<Item = Option<LookTint>>) -> Self {
        Self(stops.into_iter().collect())
    }
    /// A two-stop gradient (`card_with`'s top-to-bottom fill).
    pub fn two(first: LookTint, last: LookTint) -> Self {
        Self(vec![Some(first), Some(last)])
    }
    /// Every stop from one role, stop `i` at `alphas[i]` (a scrim or a
    /// vignette that fades one colour out).
    pub fn role(role: &'static str, alphas: &[f32]) -> Self {
        Self(
            alphas
                .iter()
                .map(|a| Some(LookTint::new(role).alpha(*a)))
                .collect(),
        )
    }
}

/// The colour of each stop of `g`, in order.
fn stop_colours(g: &mut Gradient) -> Vec<&mut Color> {
    match g {
        Gradient::Linear(l) => l.stops.iter_mut().map(|s| &mut s.color).collect(),
        Gradient::Radial(r) => r.stops.iter_mut().map(|s| &mut s.color).collect(),
        Gradient::Conic(c) => c.stops.iter_mut().map(|s| &mut s.color).collect(),
    }
}

pub fn retint_border(
    look: Res<ActiveLook>,
    mut changed: MessageReader<LookChanged>,
    mut q: Query<(Ref<LookBorder>, &mut BorderColor)>,
) {
    let all = changed.read().count() > 0;
    for (tag, mut border) in &mut q {
        if all || tag.is_added() {
            let t = &tag.0;
            let want = BorderColor {
                top: t.resolve_over(&look, border.top),
                right: t.resolve_over(&look, border.right),
                bottom: t.resolve_over(&look, border.bottom),
                left: t.resolve_over(&look, border.left),
            };
            if *border != want {
                *border = want;
            }
        }
    }
}

pub fn retint_gradient(
    look: Res<ActiveLook>,
    mut changed: MessageReader<LookChanged>,
    mut q: Query<(Ref<LookGradient>, &mut BackgroundGradient)>,
) {
    let all = changed.read().count() > 0;
    for (tag, mut gradient) in &mut q {
        if !(all || tag.is_added()) {
            continue;
        }
        // Compare first: `DerefMut` marks the gradient changed (a re-layout).
        let mut want = gradient.clone();
        for g in &mut want.0 {
            for (colour, tint) in stop_colours(g).into_iter().zip(&tag.0) {
                if let Some(t) = tint {
                    *colour = t.resolve_over(&look, *colour);
                }
            }
        }
        if *gradient != want {
            *gradient = want;
        }
    }
}

pub fn retint_image(
    look: Res<ActiveLook>,
    mut changed: MessageReader<LookChanged>,
    mut q: Query<(Ref<LookImage>, &mut ImageNode)>,
) {
    let all = changed.read().count() > 0;
    for (tag, mut image) in &mut q {
        if all || tag.is_added() {
            let want = tag.0.resolve_over(&look, image.color);
            if image.color != want {
                image.color = want;
            }
        }
    }
}

pub fn retint_text_shadow(
    look: Res<ActiveLook>,
    mut changed: MessageReader<LookChanged>,
    mut q: Query<(Ref<LookShadow>, &mut TextShadow)>,
) {
    let all = changed.read().count() > 0;
    for (tag, mut shadow) in &mut q {
        if all || tag.is_added() {
            let want = tag.0.resolve_over(&look, shadow.color);
            if shadow.color != want {
                shadow.color = want;
            }
        }
    }
}

pub fn retint_box_shadow(
    look: Res<ActiveLook>,
    mut changed: MessageReader<LookChanged>,
    mut q: Query<(Ref<LookShadow>, &mut BoxShadow)>,
) {
    let all = changed.read().count() > 0;
    for (tag, mut shadow) in &mut q {
        if !(all || tag.is_added()) {
            continue;
        }
        // Compare first: `DerefMut` marks the shadow changed.
        if shadow
            .0
            .iter()
            .any(|s| s.color != tag.0.resolve_over(&look, s.color))
        {
            for s in &mut shadow.0 {
                s.color = tag.0.resolve_over(&look, s.color);
            }
        }
    }
}
