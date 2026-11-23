//! Getting a look onto the screen. Spawn code tags entities with
//! `LookColour` / `LookText` next to the game's own colour or text; the
//! systems here (in `PostUpdate`, so before the frame renders) replace
//! them from the active pack on the frame they spawn and again on every
//! `LookChanged` (hot reload). Untagged entities keep what spawn gave them.
//!
//! The `StandardMaterial` retint needs `bevy_pbr`. A 2D game built without
//! it sets the `gx_look_no_pbr` cfg from its own `build.rs`
//! (`println!("cargo::rustc-cfg=gx_look_no_pbr");`), which leaves that
//! retint out; `embed.rs` declares the cfg. Everything else here, the
//! `ColorMaterial` retint included, needs only `bevy_sprite_render`.

use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;

use super::runtime::ActiveLook;

/// Sent after the active pack is replaced (native hot reload).
#[derive(Message, Debug, Clone, Copy)]
pub struct LookChanged;

/// Colours the entity from palette role `role`. On an entity with a
/// `TextColor` (every UI `Text`, which Bevy also gives a transparent
/// `BackgroundColor`) it colours the text only. Otherwise it colours the
/// `BackgroundColor`, `Sprite`, `ColorMaterial` (`MeshMaterial2d`) or
/// `StandardMaterial` (`MeshMaterial3d`) it finds there.
#[derive(Component, Debug, Clone, Copy)]
pub struct LookColour {
    pub role: &'static str,
    pub alpha: Option<f32>,
    /// For a `StandardMaterial`: emissive RGB = the colour's linear RGB
    /// times this (times the pack's `glow` for the role, which can only dim
    /// it). The emissive's **alpha is left as the material was built**:
    /// Bevy reads emissive alpha as the exposure weight, not opacity, so
    /// scaling it would change a weak glow's look under the default pack.
    /// Ignored everywhere else (`ColorMaterial` has no emissive).
    pub emissive: Option<f32>,
}

impl LookColour {
    pub const fn new(role: &'static str) -> Self {
        Self {
            role,
            alpha: None,
            emissive: None,
        }
    }
    pub const fn alpha(mut self, alpha: f32) -> Self {
        self.alpha = Some(alpha);
        self
    }
    pub const fn emissive(mut self, strength: f32) -> Self {
        self.emissive = Some(strength);
        self
    }
    pub fn resolve(&self, look: &ActiveLook) -> Color {
        let c = look.colour(self.role);
        match self.alpha {
            Some(a) => c.with_alpha(a),
            None => c,
        }
    }
}

/// Replaces the entity's `Text` with string `key` of the active pack.
#[derive(Component, Debug, Clone, Copy)]
pub struct LookText(pub &'static str);

/// Decoded images (by file name) and audio overrides (by cue).
#[derive(Resource, Default, Debug)]
pub struct LookAssets {
    images: Vec<(String, Handle<Image>)>,
    audio: Vec<(String, Handle<AudioSource>)>,
}

impl LookAssets {
    pub fn image(&self, file: &str) -> Option<Handle<Image>> {
        self.images
            .iter()
            .find(|(n, _)| n == file)
            .map(|(_, h)| h.clone())
    }
    pub fn audio(&self, cue: &str) -> Option<Handle<AudioSource>> {
        self.audio
            .iter()
            .find(|(n, _)| n == cue)
            .map(|(_, h)| h.clone())
    }
}

/// Decodes the pack's PNGs and wraps its OGGs. Headless apps (the replay
/// verifier, tests) have no asset stores and get an empty set.
pub fn build_assets(
    look: &ActiveLook,
    images: Option<&mut Assets<Image>>,
    audio: Option<&mut Assets<AudioSource>>,
) -> LookAssets {
    let mut out = LookAssets::default();
    if let Some(images) = images {
        for (name, bytes) in look.files.iter().filter(|(n, _)| n.ends_with(".png")) {
            match Image::from_buffer(
                bytes,
                ImageType::Extension("png"),
                CompressedImageFormats::NONE,
                true,
                ImageSampler::Default,
                RenderAssetUsages::default(),
            ) {
                Ok(image) => out.images.push((name.clone(), images.add(image))),
                Err(e) => error!("look pack: {name}: {e}"),
            }
        }
    }
    if let (Some(audio), Some(manifest)) = (audio, look.manifest.as_ref()) {
        for (cue, file) in &manifest.audio {
            if let Some(bytes) = look.file(file) {
                let source = AudioSource {
                    bytes: bytes.to_vec().into(),
                };
                out.audio.push((cue.clone(), audio.add(source)));
            }
        }
    }
    out
}

pub fn load_assets(
    mut commands: Commands,
    look: Res<ActiveLook>,
    mut images: Option<ResMut<Assets<Image>>>,
    mut audio: Option<ResMut<Assets<AudioSource>>>,
) {
    commands.insert_resource(build_assets(
        &look,
        images.as_deref_mut(),
        audio.as_deref_mut(),
    ));
}

pub fn retint_text(
    look: Res<ActiveLook>,
    mut changed: MessageReader<LookChanged>,
    mut q: Query<(Ref<LookColour>, &mut TextColor)>,
) {
    let all = changed.read().count() > 0;
    for (tag, mut c) in &mut q {
        if all || tag.is_added() {
            c.0 = tag.resolve(&look);
        }
    }
}

pub fn retint_background(
    look: Res<ActiveLook>,
    mut changed: MessageReader<LookChanged>,
    mut q: Query<(Ref<LookColour>, &mut BackgroundColor), Without<TextColor>>,
) {
    let all = changed.read().count() > 0;
    for (tag, mut c) in &mut q {
        if all || tag.is_added() {
            c.0 = tag.resolve(&look);
        }
    }
}

pub fn retint_sprite(
    look: Res<ActiveLook>,
    mut changed: MessageReader<LookChanged>,
    mut q: Query<(Ref<LookColour>, &mut Sprite), Without<TextColor>>,
) {
    let all = changed.read().count() > 0;
    for (tag, mut s) in &mut q {
        if all || tag.is_added() {
            s.color = tag.resolve(&look);
        }
    }
}

/// Mutates the material asset itself: entities sharing a material share
/// the new colour. Give an entity its own material to colour it alone.
/// Needs `bevy_pbr`: compiled out under `gx_look_no_pbr` (module docs).
#[cfg(not(gx_look_no_pbr))]
pub fn retint_material(
    look: Res<ActiveLook>,
    mut changed: MessageReader<LookChanged>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    q: Query<(Ref<LookColour>, &MeshMaterial3d<StandardMaterial>)>,
) {
    let all = changed.read().count() > 0;
    let Some(mut materials) = materials else {
        return;
    };
    for (tag, handle) in &q {
        if !(all || tag.is_added()) {
            continue;
        }
        if let Some(m) = materials.get_mut(&handle.0) {
            let c = tag.resolve(&look);
            m.base_color = c;
            if let Some(s) = tag.emissive {
                // RGB only: the authored alpha (exposure weight) stays.
                let alpha = m.emissive.alpha;
                m.emissive = (LinearRgba::from(c) * s * look.glow(tag.role)).with_alpha(alpha);
            }
        }
    }
}

/// The 2D twin of `retint_material`, for `MeshMaterial2d<ColorMaterial>`.
/// It mutates the material asset itself, so every entity drawing that
/// handle takes the new colour, tagged or not. A `ColorMaterial` handle is
/// often shared (one per colour or per kind): if tagged entities with
/// different roles share one, whichever the query visits last wins, in no
/// set order. Give each role (or each entity coloured alone) its own
/// material. Only `color` is written; `alpha_mode`, `texture` and
/// `uv_transform` stay as spawned, so a translucent `alpha` needs a
/// blending material (`ColorMaterial::from(colour)` is opaque for an opaque
/// colour; `ColorMaterial::default()` blends).
pub fn retint_color_material(
    look: Res<ActiveLook>,
    mut changed: MessageReader<LookChanged>,
    materials: Option<ResMut<Assets<ColorMaterial>>>,
    q: Query<(Ref<LookColour>, &MeshMaterial2d<ColorMaterial>)>,
) {
    let all = changed.read().count() > 0;
    let Some(mut materials) = materials else {
        return;
    };
    for (tag, handle) in &q {
        if !(all || tag.is_added()) {
            continue;
        }
        if let Some(m) = materials.get_mut(&handle.0) {
            m.color = tag.resolve(&look);
        }
    }
}

pub fn retext(
    look: Res<ActiveLook>,
    mut changed: MessageReader<LookChanged>,
    mut q: Query<(Ref<LookText>, &mut Text)>,
) {
    let all = changed.read().count() > 0;
    for (tag, mut text) in &mut q {
        if all || tag.is_added() {
            let s = look.string(tag.0);
            if text.0 != s {
                text.0 = s.to_string();
            }
        }
    }
}

#[cfg(all(test, not(gx_look_no_pbr)))]
mod tests {
    use super::*;
    use crate::look::Contract;
    use crate::look::manifest::MANIFEST_FILE;
    use crate::look::select::Selection;
    use std::collections::BTreeMap;

    const TIER: Color = Color::srgb(0.95, 0.7, 0.2);

    static C: Contract = Contract {
        palette: &[("tier", TIER)],
        strings: &[],
        slots: &[],
        audio: &[],
        fonts: &[],
        glow: &["tier"],
    };

    fn app_with(look: ActiveLook) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<LookChanged>()
            .init_resource::<Assets<StandardMaterial>>()
            .insert_resource(look)
            .add_systems(Update, retint_material);
        app
    }

    fn material(app: &mut App, m: StandardMaterial) -> Handle<StandardMaterial> {
        app.world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(m)
    }

    fn get(app: &App, h: &Handle<StandardMaterial>) -> StandardMaterial {
        app.world()
            .resource::<Assets<StandardMaterial>>()
            .get(h)
            .unwrap()
            .clone()
    }

    /// Under the default look an emissive tag writes back exactly the
    /// `LinearRgba` the game built, alpha included: a weak glow (`0.05`,
    /// attic-excavator's tier0) keeps its exposure weight, whichever way
    /// the game wrote that alpha.
    #[test]
    fn the_default_look_keeps_the_authored_emissive_alpha_included() {
        for (s, alpha) in [(0.05, 1.0), (0.05, 0.05), (1.5, 1.0), (0.4, 0.3)] {
            let authored = (LinearRgba::from(TIER) * s).with_alpha(alpha);
            let mut app = app_with(ActiveLook::default_for(&C));
            let h = material(
                &mut app,
                StandardMaterial {
                    base_color: TIER,
                    emissive: authored,
                    ..default()
                },
            );
            app.world_mut().spawn((
                MeshMaterial3d(h.clone()),
                LookColour::new("tier").emissive(s),
            ));
            app.update();
            let m = get(&app, &h);
            assert_eq!(m.emissive, authored, "strength {s}, alpha {alpha}");
            assert_eq!(m.base_color, TIER);
        }
    }

    /// Under a pack the RGB follows the pack colour times the strength and
    /// the pack's glow; the alpha stays the game's; a clear restores it all.
    #[test]
    fn a_pack_scales_the_emissive_rgb_only_and_a_clear_restores_it() {
        let authored = LinearRgba::from(TIER) * 0.05; // alpha 0.05 too
        let json = r##"{"v":3,"id":"p","name":"P","kind":"palette","palette":{"tier":"#3366ff"},"glow":{"tier":0.5}}"##;
        let files = BTreeMap::from([(MANIFEST_FILE.to_string(), json.as_bytes().to_vec())]);
        let pack = ActiveLook::from_files(&C, files, Selection::Default, None).expect("valid");
        let blue = LinearRgba::from(pack.colour("tier"));
        let mut app = app_with(pack);
        let h = material(
            &mut app,
            StandardMaterial {
                base_color: TIER,
                emissive: authored,
                ..default()
            },
        );
        app.world_mut().spawn((
            MeshMaterial3d(h.clone()),
            LookColour::new("tier").emissive(0.05),
        ));
        app.update();
        let e = get(&app, &h).emissive;
        assert_eq!(e.alpha, authored.alpha);
        assert!((e.blue - blue.blue * 0.05 * 0.5).abs() < 1e-6);
        assert!((e.red - blue.red * 0.05 * 0.5).abs() < 1e-6);

        app.insert_resource(ActiveLook::default_for(&C));
        app.world_mut().write_message(LookChanged);
        app.update();
        assert_eq!(get(&app, &h).emissive, authored);
    }
}
