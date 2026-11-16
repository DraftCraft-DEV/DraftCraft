//! 3D solids (AutoCAD's 3DSOLID) made in DraftCraft (docs/solids.md).
//!
//! The document never calls the solid modelling kernel. A solid keeps the exact body in the
//! kernel's native format, where it sits (a placement), and what it looks like (its edges and a
//! triangle mesh, both in the body's own coordinates), worked out by the engine when the body was
//! made or changed. Moving, rotating, scaling and mirroring a solid only change the placement;
//! drawing, snapping and selecting read the edges.

use std::sync::Arc;

use draftcraft_geom::{Mat3, Mat4, Vec2, Vec3};
use serde::{Deserialize, Serialize};

/// What a solid looks like, in its own coordinates.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SolidLook {
    /// Its edges as polylines.
    pub wires: Vec<Vec<Vec3>>,
    /// Its faces as triangles: corner positions, corner index triples (counter-clockwise seen
    /// from outside), and the face each triangle belongs to.
    pub positions: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
    pub tri_face: Vec<u32>,
}

impl SolidLook {
    /// The corners of triangle `t`, `None` for an index outside the positions.
    pub fn triangle(&self, t: &[u32; 3]) -> Option<[Vec3; 3]> {
        let p = |i: u32| self.positions.get(usize::try_from(i).ok()?).copied();
        Some([p(t[0])?, p(t[1])?, p(t[2])?])
    }
}

/// A 3D solid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Solid3d {
    /// The exact body in solidcore's `.smb` format behind the header `DCSMB001`, in the solid's own coordinates.
    /// Not part of the entity's JSON (it is binary, and large); the drawing file keeps it.
    #[serde(skip)]
    pub body: Arc<Vec<u8>>,
    /// Where the body sits: a point `p` of it is at `placement * p` in the drawing.
    pub placement: Mat4,
    #[serde(skip)]
    pub look: Arc<SolidLook>,
    /// Faces, edges and volume as the kernel counted them when the body was made, for the
    /// Properties palette and LIST (no kernel call needed to show them).
    #[serde(default)]
    pub faces: u32,
    #[serde(default)]
    pub edges: u32,
    #[serde(default)]
    pub volume: f64,
    /// Read from an AutoCAD BODY entity (ACIS data from another modeller) and saved back as one;
    /// a solid DraftCraft makes is a 3DSOLID.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub body_entity: bool,
}

impl Solid3d {
    /// A drawing point of the solid's point `p`.
    pub fn world(&self, p: Vec3) -> Vec3 {
        self.placement.apply(p)
    }

    /// Its edges in drawing coordinates.
    pub fn world_wires(&self) -> impl Iterator<Item = Vec<Vec3>> + '_ {
        self.look.wires.iter().map(|w| w.iter().map(|p| self.world(*p)).collect())
    }

    /// The edges seen from above (plan view): segments in the XY plane.
    pub fn plan_segments(&self) -> Vec<(Vec2, Vec2)> {
        let mut out = Vec::new();
        for w in self.world_wires() {
            out.extend(w.windows(2).filter_map(|p| Some((p.first()?.xy(), p.get(1)?.xy()))));
        }
        out
    }

    /// Apply a 2D transform of the drawing (MOVE, ROTATE, SCALE, MIRROR) in XY; Z is left as it
    /// is ([`Solid3d::scale_heights`] scales it).
    pub fn transform(&mut self, m: &Mat3) {
        self.placement = Mat4::from_mat3(m).mul(&self.placement);
        self.volume *= m.det().abs();
    }

    /// Scale the solid's heights by `f` about the height `base_z` (SCALE, with
    /// [`Solid3d::transform`]).
    pub fn scale_heights(&mut self, base_z: f64, f: f64) {
        let mut t = Mat4::IDENTITY;
        t.m[2][2] = f;
        t.m[2][3] = base_z * (1.0 - f);
        self.placement = t.mul(&self.placement);
        self.volume *= f.abs();
    }

    /// Apply a 3D transform of the drawing.
    pub fn transform3(&mut self, m: &Mat4) {
        self.placement = m.mul(&self.placement);
    }

    /// Whether the placement mirrors the body (a negative determinant): its triangles then wind
    /// the other way round in the drawing, and a mesh or a shaded view made from them must swap
    /// two corners of each to keep the faces' outsides outside.
    pub fn mirrored(&self) -> bool {
        let m = &self.placement.m;
        let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
        det < 0.0
    }

    /// A triangle of the look in drawing coordinates, wound so that it faces out of the solid.
    pub fn world_triangle(&self, local: [Vec3; 3]) -> [Vec3; 3] {
        let [a, b, c] = local.map(|p| self.world(p));
        if self.mirrored() { [a, c, b] } else { [a, b, c] }
    }

    /// Whether the placement is something a drawing file can store and the kernel can apply:
    /// finite, affine and not flattening the body.
    pub fn placement_ok(&self) -> bool {
        let m = &self.placement.m;
        let finite = m.iter().flatten().all(|v| v.is_finite());
        let affine = m[3][0] == 0.0 && m[3][1] == 0.0 && m[3][2] == 0.0 && m[3][3] == 1.0;
        let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
        finite && affine && det.abs() > 1e-18
    }
}
