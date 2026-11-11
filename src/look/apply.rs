//! Getting a look onto the screen. Spawn code tags entities with
//! `LookColour` / `LookText` next to the game's own colour or text; the
//! systems here (in `PostUpdate`, so before the frame renders) replace
//! them from the active pack on the frame they spawn and again on every
//! `LookChanged` (hot reload). Untagged entities keep what spawn gave them.

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
/// `BackgroundColor`, `Sprite` or `StandardMaterial` it finds there.
#[derive(Component, Debug, Clone, Copy)]
pub struct LookColour {
    pub role: &'static str,
    pub alpha: Option<f32>,
    /// For a `StandardMaterial`: emissive = the colour times this.
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
                m.emissive = LinearRgba::from(c) * s;
            }
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
