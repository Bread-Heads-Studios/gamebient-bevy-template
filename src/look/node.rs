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

/// A palette role at an optional alpha: one colour a tag writes. `None`
/// keeps the role's own alpha, as `LookColour` does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LookTint {
    pub role: &'static str,
    pub alpha: Option<f32>,
}

impl LookTint {
    pub const fn new(role: &'static str) -> Self {
        Self { role, alpha: None }
    }
    pub const fn alpha(mut self, alpha: f32) -> Self {
        self.alpha = Some(alpha);
        self
    }
    pub fn resolve(&self, look: &ActiveLook) -> Color {
        let c = look.colour(self.role);
        self.alpha.map_or(c, |a| c.with_alpha(a))
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
            let want = BorderColor::all(tag.0.resolve(&look));
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
                    *colour = t.resolve(&look);
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
            let want = tag.0.resolve(&look);
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
            let want = tag.0.resolve(&look);
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
        let want = tag.0.resolve(&look);
        // Compare first: `DerefMut` marks the shadow changed.
        if shadow.0.iter().any(|s| s.color != want) {
            for s in &mut shadow.0 {
                s.color = want;
            }
        }
    }
}
