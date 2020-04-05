use super::{
    camera::{Camera2D, CameraController},
    Color, Context, Vertex2D,
};
use crate::core::{space::SpaceReadAccess, storage, Container, TransformFeature};

use glium::{backend::Facade, index::PrimitiveType, uniform};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub enum ShapeStyle {
    Fill(Color),
    Outline(Color),
}

/// A flat-colored convex polygon shape, rendered using the ShapeRenderer system.
/// When creating multiple identical shapes, it is preferable to create one and clone it,
/// as this reuses the same vertex buffer for all clones.
/// Concavity will not result in an error but will be rendered incorrectly.
#[derive(Clone)]
pub struct Shape {
    // TODO: turn these back to pub(self) once we move the new system here
    pub(crate) verts: Arc<glium::VertexBuffer<Vertex2D>>,
    pub(crate) color: Color,
    pub(crate) primitive_type: PrimitiveType,
}

impl Shape {
    /// Create a new Shape from a set of points.
    pub fn new<F: Facade + ?Sized>(facade: &F, points: &[[f32; 2]], style: ShapeStyle) -> Self {
        let points_as_verts: Vec<Vertex2D> = points.iter().map(|p| Vertex2D::from(*p)).collect();
        let (color, primitive_type) = match style {
            ShapeStyle::Fill(c) => (c, PrimitiveType::TriangleFan),
            ShapeStyle::Outline(c) => (c, PrimitiveType::LineLoop),
        };
        Shape {
            verts: Arc::new(
                glium::VertexBuffer::new(facade, points_as_verts.as_slice())
                    .expect("Failed to create vertex buffer"),
            ),
            color,
            primitive_type,
        }
    }

    pub fn set_color(&mut self, color: Color) {
        self.color = color;
    }

    /// Create an axis-aligned square Shape with the given side length.
    pub fn new_square<F: Facade + ?Sized>(facade: &F, width: f32, style: ShapeStyle) -> Self {
        let hw = width * 0.5;
        Self::new(facade, &[[-hw, -hw], [hw, -hw], [hw, hw], [-hw, hw]], style)
    }

    /// Create an axis-aligned rectangle Shape with the given dimensions.
    pub fn new_rect<F: Facade + ?Sized>(
        facade: &F,
        width: f32,
        height: f32,
        style: ShapeStyle,
    ) -> Self {
        let hw = width * 0.5;
        let hh = height * 0.5;
        Self::new(facade, &[[-hw, -hh], [hw, -hh], [hw, hh], [-hw, hh]], style)
    }

    /// Create a polygonal approximation of a circle with the given radius and number of points.
    pub fn new_circle<F: Facade + ?Sized>(
        facade: &F,
        radius: f32,
        point_count: u32,
        style: ShapeStyle,
    ) -> Self {
        let angle_incr = 2.0 * std::f32::consts::PI / point_count as f32;
        let pts: Vec<[f32; 2]> = (0..point_count)
            .map(|i| {
                let angle = angle_incr * i as f32;
                [radius * angle.cos(), radius * angle.sin()]
            })
            .collect();
        Self::new(facade, pts.as_slice(), style)
    }

    /// Create a Shape that matches the given Collider.
    /// Circle colliders are approximated with a polygon.
    pub fn from_collider<F: Facade + ?Sized>(
        facade: &F,
        coll: &crate::physics2d::Collider,
        style: ShapeStyle,
    ) -> Self {
        use crate::physics2d::ColliderShape;
        match coll.shape() {
            ColliderShape::Circle { r } => {
                let pts: Vec<[f32; 2]> =
                    CIRCLE_VERTS.iter().map(|p| [r * p[0], r * p[1]]).collect();
                Self::new(facade, pts.as_slice(), style)
            }
            ColliderShape::Rect { hw, hh } => Self::new(
                facade,
                &[[-hw, -hh], [*hw, -hh], [*hw, *hh], [-hw, *hh]],
                style,
            ),
        }
    }
}

pub type ShapeFeature = Container<storage::DenseVecStorage<Shape>>;
impl ShapeFeature {
    pub fn draw<S: glium::Surface, C: CameraController>(
        &self,
        space: &SpaceReadAccess,
        trs: &TransformFeature,
        target: &mut S,
        camera: &Camera2D<C>,
    ) {
        let view = camera.view_matrix();

        for (shape, tr) in space.iter().overlay(self.iter()).and(trs.iter()) {
            let model = tr.0.into_homogeneous_matrix();
            let mv = view * model;
            let mv_uniform = [
                [mv.cols[0].x, mv.cols[0].y, mv.cols[0].z],
                [mv.cols[1].x, mv.cols[1].y, mv.cols[1].z],
                [mv.cols[2].x, mv.cols[2].y, mv.cols[2].z],
            ];

            let uniforms = glium::uniform! {
                model_view: mv_uniform,
                color: shape.color,
            };
            target
                .draw(
                    &*shape.verts,
                    glium::index::NoIndices(shape.primitive_type),
                    &Context::get().shaders.ortho_2d,
                    &uniforms,
                    &Default::default(),
                )
                .expect("Drawing failed");
        }
    }
}

const CIRCLE_VERTS_COUNT: u32 = 16;

lazy_static::lazy_static! {
    /// All circles are the same so we can precalculate their vertices
    static ref CIRCLE_VERTS: Vec<[f32; 2]> = {
        let angle_incr = 2.0 * std::f32::consts::PI / CIRCLE_VERTS_COUNT as f32;
        (0..CIRCLE_VERTS_COUNT).map(|i| {
            let angle = angle_incr * i as f32;
            [angle.cos(), angle.sin()]
        }).collect()
    };
}
