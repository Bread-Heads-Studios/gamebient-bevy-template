//! Sprites per kind of Thing: `content/sprites/<Kind>.png` replaces the
//! kind's placeholder box with a textured quad; `content/sprites.json`
//! says how many frames the image holds, how fast to play them, and how
//! to place the quad.
//!
//! Presentation only: nothing here reads or writes sim state, and nothing
//! enters the replay content hash. Everything is skipped when the image,
//! mesh or material stores are absent (the headless verifier). A change
//! (`set_sprite`, `set_sprites_meta`) edits the existing art children in
//! place: no entity is despawned and no parent Thing is touched.
//!
//! Frames are laid out left to right in one image. `anchor` is the point of
//! the image (0..1 from the left and from the bottom) that sits on the
//! Thing's position; the default `[0.5, 0.5]` is the centre.

use std::collections::BTreeMap;

use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::math::Affine2;
use bevy::prelude::*;

use super::files::{self, embedded};
use super::kit::{Kinds, Size};
use super::model::{ContentError, SpriteMeta, SpritesFile};
use super::saved::ContentWarnings;

pub const SPRITE_MAX_BYTES: usize = 64 * 1024;
pub const SPRITE_MAX_SIDE: u32 = 256;

/// The loaded sprite images: kind name, image, and the meta in force.
#[derive(Resource, Default)]
pub struct SpriteImages(pub Vec<(String, Handle<Image>, SpriteMeta)>);

/// The parsed `sprites.json` (empty when the file is missing).
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct SpriteMetaTable(pub BTreeMap<String, SpriteMeta>);

/// The shared quad and one sprite material per kind that has a sprite.
#[derive(Resource)]
pub struct SpriteArt {
    pub quad: Handle<Mesh>,
    /// Indexed by kind.
    pub materials: Vec<Option<Handle<StandardMaterial>>>,
}

/// Marks a Thing's art child with its kind, so a sprite change can find it.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArtOf(pub u16);

pub fn default_meta() -> SpriteMeta {
    SpriteMeta {
        frames: 1,
        fps: 8.0,
        scale: 1.0,
        anchor: None,
    }
}

/// Where a sprite's quad sits relative to its Thing, and how big it is.
pub fn sprite_transform(size: Vec2, meta: &SpriteMeta) -> Transform {
    let wh = size * meta.scale;
    let a = meta.anchor.unwrap_or([0.5, 0.5]);
    Transform {
        translation: Vec3::new((0.5 - a[0]) * wh.x, (0.5 - a[1]) * wh.y, 0.51),
        scale: wh.extend(1.0),
        ..default()
    }
}

/// UV transform that shows `frame` of `frames` equal slices (left to right).
pub fn frame_uv(frame: u16, frames: u16) -> Affine2 {
    let n = f32::from(frames.max(1));
    Affine2::from_scale_angle_translation(
        Vec2::new(1.0 / n, 1.0),
        0.0,
        Vec2::new(f32::from(frame) / n, 0.0),
    )
}

/// Decodes and checks a sprite PNG: at most 64 KB and 256x256, nearest sampling.
pub fn decode_png(png: &[u8]) -> Result<Image, ContentError> {
    if png.len() > SPRITE_MAX_BYTES {
        return Err(ContentError("The sprite is larger than 64 KB.".to_string()));
    }
    let image = Image::from_buffer(
        png,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::nearest(),
        RenderAssetUsages::RENDER_WORLD,
    )
    .map_err(|_| ContentError("The sprite is not a PNG picture that can be read.".to_string()))?;
    if image.width() > SPRITE_MAX_SIDE || image.height() > SPRITE_MAX_SIDE {
        return Err(ContentError(
            "The sprite is bigger than 256 by 256 pixels.".to_string(),
        ));
    }
    Ok(image)
}

fn stores_exist(world: &World) -> bool {
    world.contains_resource::<Assets<Image>>()
        && world.contains_resource::<Assets<Mesh>>()
        && world.contains_resource::<Assets<StandardMaterial>>()
}

fn ensure_art(world: &mut World) {
    if world.contains_resource::<SpriteArt>() {
        return;
    }
    let quad = world
        .resource_mut::<Assets<Mesh>>()
        .add(Rectangle::new(1.0, 1.0));
    let n = world.resource::<Kinds>().0.len();
    world.insert_resource(SpriteArt {
        quad,
        materials: vec![None; n],
    });
}

/// Brings one kind's art in line with its sprite image and meta: the shared
/// material, and every existing art child (a box becomes a quad in place).
fn refresh_kind(world: &mut World, kind: u16) {
    let name = world.resource::<Kinds>().0[usize::from(kind)].0.clone();
    let meta = world
        .resource::<SpriteMetaTable>()
        .0
        .get(&name)
        .cloned()
        .unwrap_or_else(default_meta);
    let mut images = world.resource_mut::<SpriteImages>();
    let Some(entry) = images.0.iter_mut().find(|(n, ..)| *n == name) else {
        return;
    };
    entry.2 = meta.clone();
    let image = entry.1.clone();
    ensure_art(world);
    let existing = world.resource::<SpriteArt>().materials[usize::from(kind)].clone();
    let uv = frame_uv(0, meta.frames);
    let material = match existing {
        Some(h) => {
            if let Some(m) = world.resource_mut::<Assets<StandardMaterial>>().get_mut(&h) {
                m.base_color_texture = Some(image);
                m.uv_transform = uv;
            }
            h
        }
        None => {
            let h = world
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial {
                    base_color_texture: Some(image),
                    unlit: true,
                    alpha_mode: AlphaMode::Mask(0.5),
                    uv_transform: uv,
                    ..default()
                });
            world.resource_mut::<SpriteArt>().materials[usize::from(kind)] = Some(h.clone());
            h
        }
    };
    let quad = world.resource::<SpriteArt>().quad.clone();
    let children: Vec<(Entity, Entity)> = world
        .query::<(Entity, &ArtOf, &ChildOf)>()
        .iter(world)
        .filter(|(_, a, _)| a.0 == kind)
        .map(|(e, _, p)| (e, p.parent()))
        .collect();
    for (child, parent) in children {
        let Some(size) = world.get::<Size>(parent).map(|s| s.0) else {
            continue;
        };
        world.entity_mut(child).insert((
            Mesh3d(quad.clone()),
            MeshMaterial3d(material.clone()),
            sprite_transform(size, &meta),
        ));
    }
}

/// Replaces (or adds) the sprite for `kind` from PNG bytes. Takes effect on
/// the next frame for Things already placed. A no-op after validation when
/// there are no asset stores (the verifier).
pub fn set_sprite(world: &mut World, kind: &str, png: &[u8]) -> Result<(), ContentError> {
    let Some(index) = world.get_resource::<Kinds>().and_then(|k| k.index(kind)) else {
        return Err(ContentError(format!("There is no kind called \"{kind}\".")));
    };
    let image = decode_png(png)?;
    if !stores_exist(world) {
        return Ok(());
    }
    let handle = world.resource_mut::<Assets<Image>>().add(image);
    world.init_resource::<SpriteImages>();
    {
        let mut images = world.resource_mut::<SpriteImages>();
        match images.0.iter_mut().find(|(n, ..)| n == kind) {
            Some(entry) => entry.1 = handle,
            None => images.0.push((kind.to_string(), handle, default_meta())),
        }
    }
    world.init_resource::<SpriteMetaTable>();
    refresh_kind(world, index);
    Ok(())
}

/// Replaces `sprites.json`. Entries for kinds the game does not have are
/// kept (they apply if the kind appears) but do nothing now.
pub fn set_sprites_meta(world: &mut World, bytes: &[u8]) -> Result<(), ContentError> {
    let file = SpritesFile::parse(bytes)?;
    world.insert_resource(SpriteMetaTable(file.sprites));
    if !stores_exist(world) {
        return Ok(());
    }
    world.init_resource::<SpriteImages>();
    let named: Vec<String> = world
        .resource::<SpriteImages>()
        .0
        .iter()
        .map(|(n, ..)| n.clone())
        .collect();
    for name in named {
        if let Some(i) = world.resource::<Kinds>().index(&name) {
            refresh_kind(world, i);
        }
    }
    Ok(())
}

/// `Startup` (after `setup_thing_art`): loads the embedded `sprites.json`
/// and every `sprites/<Kind>.png`, when the windowed stores exist. A missing
/// `sprites.json` means default meta for each PNG found.
pub fn load_embedded_sprites(world: &mut World) {
    world.init_resource::<SpriteMetaTable>();
    world.init_resource::<SpriteImages>();
    if let Some(bytes) = embedded("sprites.json") {
        match SpritesFile::parse(bytes) {
            Ok(f) => world.insert_resource(SpriteMetaTable(f.sprites)),
            Err(e) => warn_content(world, format!("content/sprites.json: {e}")),
        }
    }
    if !stores_exist(world) {
        return;
    }
    for (path, bytes) in files::EMBEDDED {
        let Some(kind) = path
            .strip_prefix("sprites/")
            .and_then(|f| f.strip_suffix(".png"))
        else {
            continue;
        };
        if let Err(e) = set_sprite(world, kind, bytes) {
            warn_content(world, format!("content/{path}: {e}"));
        }
    }
}

fn warn_content(world: &mut World, msg: String) {
    warn!("{msg}");
    world.init_resource::<ContentWarnings>();
    world.resource_mut::<ContentWarnings>().0.push(msg);
}

/// `Update`: steps animated sprites. Driven by `Time` (presentation, not
/// the sim clock); one shared material per kind, so every Thing of a kind
/// shows the same frame.
pub fn animate_sprites(
    time: Res<Time>,
    art: Option<Res<SpriteArt>>,
    images: Option<Res<SpriteImages>>,
    kinds: Res<Kinds>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let (Some(art), Some(images), Some(mut materials)) = (art, images, materials) else {
        return;
    };
    let secs = time.elapsed_secs(); // allow-app-clock: presentation animation, not sim
    for (name, _, meta) in &images.0 {
        if meta.frames <= 1 {
            continue;
        }
        let Some(kind) = kinds.index(name) else {
            continue;
        };
        let Some(Some(h)) = art.materials.get(usize::from(kind)) else {
            continue;
        };
        let frame = ((secs * meta.fps) as u32 % u32::from(meta.frames)) as u16;
        let uv = frame_uv(frame, meta.frames);
        if materials.get(h).is_some_and(|m| m.uv_transform != uv)
            && let Some(m) = materials.get_mut(h)
        {
            m.uv_transform = uv;
        }
    }
}

/// What `spawn_level` needs to pick a kind's art: the sprite material and
/// its transform, if the kind has a sprite.
pub fn sprite_for(
    art: Option<&SpriteArt>,
    images: Option<&SpriteImages>,
    kinds: &Kinds,
    kind: u16,
    size: Vec2,
) -> Option<(Handle<Mesh>, Handle<StandardMaterial>, Transform)> {
    let art = art?;
    let material = art.materials.get(usize::from(kind))?.clone()?;
    let name = &kinds.0.get(usize::from(kind))?.0;
    let meta = images?.0.iter().find(|(n, ..)| n == name).map(|e| &e.2)?;
    Some((art.quad.clone(), material, sprite_transform(size, meta)))
}
