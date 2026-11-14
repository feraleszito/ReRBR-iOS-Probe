//! Procedural test geometry, not extracted from game data. Coordinates follow reRBR.

use rerbr_render_core::{camera::FreeFly, coords, Mat4, Vec3};

pub(crate) const EYE: Vec3 = Vec3::new(3.0, 2.5, -4.0);
const TARGET: Vec3 = Vec3::new(0.0, 0.5, 0.0);

#[derive(Debug, Clone, Copy)]
pub(crate) struct Vertex {
    pub(crate) position: [f32; 3],
    pub(crate) color: [f32; 3],
}

pub(crate) struct Mesh {
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) indices: Vec<u16>,
}

impl Mesh {
    fn quad(&mut self, corners: [[f32; 3]; 4], color: [f32; 3]) {
        let first = u16::try_from(self.vertices.len()).expect("small procedural mesh");
        self.vertices
            .extend(corners.map(|position| Vertex { position, color }));
        self.indices
            .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }
}

/// Cube faces point outwards; the ground faces up. They appear clockwise through
/// reRBR's left-handed view/projection and use coords::FRONT_FACE with back culling.
pub(crate) fn floor_and_cube() -> Mesh {
    let mut mesh = Mesh {
        vertices: Vec::with_capacity(28),
        indices: Vec::with_capacity(42),
    };
    mesh.quad(
        [
            [-0.5, 0.0, -0.5],
            [-0.5, 1.0, -0.5],
            [0.5, 1.0, -0.5],
            [0.5, 0.0, -0.5],
        ],
        [0.95, 0.35, 0.18], // Front, -z.
    );
    mesh.quad(
        [
            [0.5, 0.0, 0.5],
            [0.5, 1.0, 0.5],
            [-0.5, 1.0, 0.5],
            [-0.5, 0.0, 0.5],
        ],
        [0.45, 0.20, 0.12], // Back, +z.
    );
    mesh.quad(
        [
            [-0.5, 0.0, 0.5],
            [-0.5, 1.0, 0.5],
            [-0.5, 1.0, -0.5],
            [-0.5, 0.0, -0.5],
        ],
        [0.14, 0.40, 0.62], // Left, -x.
    );
    mesh.quad(
        [
            [0.5, 0.0, -0.5],
            [0.5, 1.0, -0.5],
            [0.5, 1.0, 0.5],
            [0.5, 0.0, 0.5],
        ],
        [0.20, 0.65, 0.90], // Right, +x.
    );
    mesh.quad(
        [
            [-0.5, 1.0, -0.5],
            [-0.5, 1.0, 0.5],
            [0.5, 1.0, 0.5],
            [0.5, 1.0, -0.5],
        ],
        [1.0, 0.80, 0.32], // Top, +y.
    );
    mesh.quad(
        [
            [-0.5, 0.0, 0.5],
            [-0.5, 0.0, -0.5],
            [0.5, 0.0, -0.5],
            [0.5, 0.0, 0.5],
        ],
        [0.20, 0.16, 0.12], // Bottom, -y.
    );
    // Ground is submitted after the cube: depth must keep it behind the cube.
    mesh.quad(
        [
            [-2.0, 0.0, -2.0],
            [-2.0, 0.0, 2.0],
            [2.0, 0.0, 2.0],
            [2.0, 0.0, -2.0],
        ],
        [0.20, 0.32, 0.25],
    );
    mesh
}

/// The eye and target never move. Only the aspect changes with the drawable size.
pub(crate) fn view_projection(aspect: f32) -> Mat4 {
    FreeFly::looking_along(EYE, TARGET - EYE)
        .view_projection(coords::FOV_DEGREES.to_radians(), aspect)
}
