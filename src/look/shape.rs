//! Build one merged `Mesh` from a Look Pack `shape` (a list of primitives),
//! and count its vertices without building.
//!
//! No ECS, no assets: pure functions over `manifest::Primitive`. The
//! validator (`validate.rs`) rejects bad shapes first; this module also must
//! not panic on bad input, so a primitive that is malformed is **skipped**
//! (it contributes no vertices, and `vertex_count` agrees). Malformed means:
//! wrong `size` arity, a non-finite or non-positive size component, a
//! non-finite `at`/`rot`/`scale`, or a zero `scale` component.
//!
//! `detail` is clamped to `DETAIL_RANGE` (default `DEFAULT_DETAIL`) and sets
//! the segment count around the primitive's axis.
//!
//! Normals: transformed by the inverse-transpose and renormalised. The one
//! exception is Bevy's cone tip, which carries a deliberate zero normal
//! (see `bevy_mesh`'s `cone.rs`); a zero normal stays zero.
//!
//! Vertex counts are closed-form per primitive (every one is verified
//! against the built mesh by the tests at details 3, 12 and 32; none builds
//! to count). With `d` the clamped detail:
//!
//! | prim     | vertices                                                     |
//! |----------|--------------------------------------------------------------|
//! | sphere   | `(d + 1) * (max(d / 2, 2) + 1)` (UV sphere, sectors `d`)     |
//! | cuboid   | `24`                                                         |
//! | cylinder | `4d + 2` (caps `2d`, one side strip `2(d + 1)`)              |
//! | cone     | `2d + 1`                                                     |
//! | torus    | `(d + 1) * (max(d / 2, 3) + 1)` (tube segments at least 3)   |
//! | capsule  | `2d + (d + 1) * 2h`, `h = max(d / 4, 2)` hemisphere stacks   |
//! | quad     | `8` (two faces of 4)                                         |

use std::ops::RangeInclusive;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

use super::manifest::{Prim, Primitive};

/// Segment count when a primitive sets no `detail`.
pub const DEFAULT_DETAIL: u32 = 12;
/// Allowed `detail`; anything outside is clamped into it.
pub const DETAIL_RANGE: RangeInclusive<u32> = 3..=32;

fn detail_of(p: &Primitive) -> u32 {
    p.detail
        .unwrap_or(DEFAULT_DETAIL)
        .clamp(*DETAIL_RANGE.start(), *DETAIL_RANGE.end())
}

fn arity(prim: Prim) -> usize {
    match prim {
        Prim::Sphere => 1,
        Prim::Cuboid => 3,
        Prim::Cylinder | Prim::Cone | Prim::Torus | Prim::Capsule | Prim::Quad => 2,
    }
}

/// False for a primitive that is skipped (see the module docs).
fn is_buildable(p: &Primitive) -> bool {
    p.size.len() == arity(p.prim)
        && p.size.iter().all(|s| s.is_finite() && *s > 0.0)
        && p.at.is_none_or(|v| v.iter().all(|c| c.is_finite()))
        && p.rot.is_none_or(|v| v.iter().all(|c| c.is_finite()))
        && p.scale
            .is_none_or(|v| v.iter().all(|c| c.is_finite() && *c != 0.0))
}

/// Hemisphere stacks for a capsule at detail `d`.
fn capsule_half_stacks(d: u32) -> u32 {
    (d / 4).max(2)
}

fn prim_vertices(p: &Primitive) -> usize {
    if !is_buildable(p) {
        return 0;
    }
    let d = detail_of(p) as usize;
    match p.prim {
        Prim::Sphere => (d + 1) * ((d / 2).max(2) + 1),
        Prim::Cuboid => 24,
        Prim::Cylinder => 4 * d + 2,
        Prim::Cone => 2 * d + 1,
        Prim::Torus => (d + 1) * ((d / 2).max(3) + 1),
        Prim::Capsule => 2 * d + (d + 1) * 2 * capsule_half_stacks(d as u32) as usize,
        Prim::Quad => 8,
    }
}

/// Vertices `build_mesh(shape)` produces, computed without building.
pub fn vertex_count(shape: &[Primitive]) -> usize {
    shape.iter().map(prim_vertices).sum()
}

/// A primitive's placement: `scale`, then `rot` (XYZ Euler, degrees), then `at`.
pub fn primitive_transform(p: &Primitive) -> Transform {
    let [rx, ry, rz] = p.rot.unwrap_or([0.0; 3]);
    Transform {
        translation: Vec3::from(p.at.unwrap_or([0.0; 3])),
        rotation: Quat::from_euler(
            EulerRot::XYZ,
            rx.to_radians(),
            ry.to_radians(),
            rz.to_radians(),
        ),
        scale: Vec3::from(p.scale.unwrap_or([1.0; 3])),
    }
}

/// The primitive at the origin, untransformed. Positions, normals and UVs
/// plus u32 indices; other attributes are never read.
fn base_mesh(p: &Primitive) -> Mesh {
    let d = detail_of(p);
    let s = &p.size;
    match p.prim {
        Prim::Sphere => Sphere::new(s[0]).mesh().uv(d, (d / 2).max(2)),
        Prim::Cuboid => Cuboid::new(s[0], s[1], s[2]).mesh().build(),
        Prim::Cylinder => Cylinder::new(s[0], s[1]).mesh().resolution(d).build(),
        Prim::Cone => Cone::new(s[0], s[1]).mesh().resolution(d).build(),
        // size is [ring_radius, tube_radius]; Bevy wants (hole, outer).
        Prim::Torus => Torus::new(s[0] - s[1], s[0] + s[1])
            .mesh()
            .major_resolution(d as usize)
            .minor_resolution(((d / 2) as usize).max(3))
            .build(),
        Prim::Capsule => Capsule3d::new(s[0], s[1])
            .mesh()
            .longitudes(d)
            .latitudes(2 * capsule_half_stacks(d))
            .build(),
        Prim::Quad => Plane3d::new(Vec3::Y, Vec2::new(s[0] * 0.5, s[1] * 0.5))
            .mesh()
            .build(),
    }
}

#[derive(Default)]
struct Buffers {
    pos: Vec<[f32; 3]>,
    nor: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    idx: Vec<u32>,
}

impl Buffers {
    /// Append `mesh` transformed by `t`: positions by `t`, normals by the
    /// inverse-transpose and renormalised, winding flipped if `t` mirrors.
    fn push(&mut self, mesh: &Mesh, t: &Transform, flip: bool, negate_normals: bool) {
        let (
            Some(VertexAttributeValues::Float32x3(pos)),
            Some(VertexAttributeValues::Float32x3(nor)),
            Some(VertexAttributeValues::Float32x2(uv)),
        ) = (
            mesh.attribute(Mesh::ATTRIBUTE_POSITION),
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL),
            mesh.attribute(Mesh::ATTRIBUTE_UV_0),
        )
        else {
            return;
        };
        let base = self.pos.len() as u32;
        let mirrored = t.scale.x * t.scale.y * t.scale.z < 0.0;
        let inv = t.scale.recip();
        for ((p, n), uv) in pos.iter().zip(nor).zip(uv) {
            self.pos.push(t.transform_point(Vec3::from(*p)).to_array());
            let mut n = Vec3::from(*n);
            if negate_normals {
                n = -n;
            }
            let n = t.rotation * (n * inv);
            self.nor.push(n.normalize_or_zero().to_array());
            self.uv.push(*uv);
        }
        let Some(indices) = mesh.indices() else {
            return;
        };
        let tris: Vec<u32> = indices.iter().map(|i| i as u32 + base).collect();
        for tri in tris.as_chunks::<3>().0 {
            if flip != mirrored {
                self.idx.extend_from_slice(&[tri[0], tri[2], tri[1]]);
            } else {
                self.idx.extend_from_slice(tri);
            }
        }
    }
}

/// One merged TriangleList mesh: `ATTRIBUTE_POSITION`, `ATTRIBUTE_NORMAL`,
/// `ATTRIBUTE_UV_0` and u32 indices, always all present (even when empty).
/// Malformed primitives are skipped; see the module docs.
pub fn build_mesh(shape: &[Primitive]) -> Mesh {
    let mut b = Buffers::default();
    for p in shape.iter().filter(|p| is_buildable(p)) {
        let t = primitive_transform(p);
        let mesh = base_mesh(p);
        b.push(&mesh, &t, false, false);
        if p.prim == Prim::Quad {
            // The back face: same quad, reversed winding, negated normals.
            b.push(&mesh, &t, true, true);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, b.pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, b.nor)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, b.uv)
    .with_inserted_indices(Indices::U32(b.idx))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Prim; 7] = [
        Prim::Sphere,
        Prim::Cuboid,
        Prim::Cylinder,
        Prim::Cone,
        Prim::Torus,
        Prim::Capsule,
        Prim::Quad,
    ];

    fn prim(kind: Prim) -> Primitive {
        let size = match kind {
            Prim::Sphere => vec![0.5],
            Prim::Cuboid => vec![1.0, 2.0, 3.0],
            Prim::Torus => vec![0.5, 0.1],
            _ => vec![0.5, 1.0],
        };
        Primitive {
            prim: kind,
            size,
            at: None,
            rot: None,
            scale: None,
            detail: None,
        }
    }

    fn positions(m: &Mesh) -> Vec<[f32; 3]> {
        match m.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(v)) => v.clone(),
            _ => panic!("no positions"),
        }
    }

    fn normals(m: &Mesh) -> Vec<[f32; 3]> {
        match m.attribute(Mesh::ATTRIBUTE_NORMAL) {
            Some(VertexAttributeValues::Float32x3(v)) => v.clone(),
            _ => panic!("no normals"),
        }
    }

    fn bounds(m: &Mesh) -> (Vec3, Vec3) {
        let mut lo = Vec3::splat(f32::MAX);
        let mut hi = Vec3::splat(f32::MIN);
        for p in positions(m) {
            lo = lo.min(Vec3::from(p));
            hi = hi.max(Vec3::from(p));
        }
        (lo, hi)
    }

    #[test]
    fn vertex_count_matches_built_mesh_for_every_prim_and_detail() {
        for kind in ALL {
            for d in [3, 12, 32] {
                let p = Primitive {
                    detail: Some(d),
                    ..prim(kind)
                };
                let m = build_mesh(std::slice::from_ref(&p));
                assert_eq!(
                    vertex_count(std::slice::from_ref(&p)),
                    m.count_vertices(),
                    "{kind:?} detail {d}"
                );
                assert!(m.count_vertices() > 0, "{kind:?} detail {d}");
            }
        }
    }

    #[test]
    fn two_primitives_count_as_the_sum() {
        let s = [prim(Prim::Sphere), prim(Prim::Cuboid)];
        assert_eq!(
            vertex_count(&s),
            vertex_count(&s[..1]) + vertex_count(&s[1..])
        );
        assert_eq!(build_mesh(&s).count_vertices(), vertex_count(&s));
    }

    #[test]
    fn empty_shape_is_an_empty_but_complete_mesh() {
        let m = build_mesh(&[]);
        assert_eq!(m.primitive_topology(), PrimitiveTopology::TriangleList);
        assert_eq!(m.count_vertices(), 0);
        assert!(m.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
        assert!(m.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
        assert!(m.attribute(Mesh::ATTRIBUTE_UV_0).is_some());
        assert_eq!(m.indices().map(|i| i.len()), Some(0));
        assert_eq!(vertex_count(&[]), 0);
    }

    #[test]
    fn built_mesh_has_exactly_the_three_attributes_and_u32_indices() {
        for kind in ALL {
            let m = build_mesh(&[prim(kind)]);
            assert_eq!(m.attributes().count(), 3, "{kind:?}");
            assert!(matches!(m.indices(), Some(Indices::U32(_))), "{kind:?}");
        }
    }

    #[test]
    fn at_moves_the_bounds_centre() {
        let p = Primitive {
            at: Some([1.0, 2.0, 3.0]),
            ..prim(Prim::Cuboid)
        };
        let (lo, hi) = bounds(&build_mesh(&[p]));
        let c = (lo + hi) * 0.5;
        assert!((c - Vec3::new(1.0, 2.0, 3.0)).length() < 1e-5, "{c}");
    }

    #[test]
    fn rot_90_about_z_maps_x_extent_to_y() {
        let p = Primitive {
            rot: Some([0.0, 0.0, 90.0]),
            size: vec![4.0, 1.0, 1.0],
            ..prim(Prim::Cuboid)
        };
        let (lo, hi) = bounds(&build_mesh(&[p]));
        let ext = hi - lo;
        assert!(
            (ext.y - 4.0).abs() < 1e-4 && (ext.x - 1.0).abs() < 1e-4,
            "{ext}"
        );
    }

    #[test]
    fn normals_are_unit_after_non_uniform_scale() {
        for kind in ALL {
            let p = Primitive {
                scale: Some([1.0, 3.0, 0.5]),
                rot: Some([20.0, 40.0, 60.0]),
                ..prim(kind)
            };
            for n in normals(&build_mesh(&[p])) {
                let len = Vec3::from(n).length();
                // Bevy's cone tip deliberately carries a zero normal.
                let cone_tip = kind == Prim::Cone && len == 0.0;
                assert!(cone_tip || (len - 1.0).abs() < 1e-4, "{kind:?} {n:?}");
            }
        }
    }

    #[test]
    fn quad_has_up_and_down_normals() {
        let m = build_mesh(&[prim(Prim::Quad)]);
        let n = normals(&m);
        assert_eq!(n.len(), 8);
        assert!(n.iter().any(|n| n[1] > 0.99));
        assert!(n.iter().any(|n| n[1] < -0.99));
    }

    #[test]
    fn out_of_range_detail_is_clamped_and_counted() {
        for kind in ALL {
            let lo = Primitive {
                detail: Some(0),
                ..prim(kind)
            };
            let lo3 = Primitive {
                detail: Some(3),
                ..prim(kind)
            };
            let hi = Primitive {
                detail: Some(10_000),
                ..prim(kind)
            };
            let hi32 = Primitive {
                detail: Some(32),
                ..prim(kind)
            };
            assert_eq!(
                vertex_count(std::slice::from_ref(&lo)),
                vertex_count(&[lo3])
            );
            assert_eq!(
                vertex_count(std::slice::from_ref(&hi)),
                vertex_count(&[hi32])
            );
            assert_eq!(
                build_mesh(std::slice::from_ref(&lo)).count_vertices(),
                vertex_count(&[lo])
            );
            assert_eq!(
                build_mesh(std::slice::from_ref(&hi)).count_vertices(),
                vertex_count(&[hi])
            );
        }
    }

    #[test]
    fn malformed_primitives_are_skipped_without_panicking() {
        let bad = [
            Primitive {
                size: vec![1.0, 2.0],
                ..prim(Prim::Sphere)
            },
            Primitive {
                size: vec![],
                ..prim(Prim::Cuboid)
            },
            Primitive {
                size: vec![0.0],
                ..prim(Prim::Sphere)
            },
            Primitive {
                size: vec![-1.0, 1.0],
                ..prim(Prim::Cone)
            },
            Primitive {
                size: vec![f32::NAN],
                ..prim(Prim::Sphere)
            },
            Primitive {
                scale: Some([0.0, 1.0, 1.0]),
                ..prim(Prim::Cuboid)
            },
            Primitive {
                at: Some([f32::INFINITY, 0.0, 0.0]),
                ..prim(Prim::Cuboid)
            },
        ];
        assert_eq!(vertex_count(&bad), 0);
        assert_eq!(build_mesh(&bad).count_vertices(), 0);
        let mixed = [bad[0].clone(), prim(Prim::Cuboid)];
        assert_eq!(build_mesh(&mixed).count_vertices(), 24);
        assert_eq!(vertex_count(&mixed), 24);
    }

    #[test]
    fn torus_size_is_ring_then_tube_radius() {
        let p = Primitive {
            size: vec![1.0, 0.25],
            detail: Some(32),
            ..prim(Prim::Torus)
        };
        let (lo, hi) = bounds(&build_mesh(&[p]));
        assert!(
            (hi.x - 1.25).abs() < 1e-4 && (hi.y - 0.25).abs() < 1e-4,
            "{hi}"
        );
        assert!((lo.y + 0.25).abs() < 1e-4);
    }
}
