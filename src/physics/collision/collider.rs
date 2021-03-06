/// A component that allows a game object to collide with others.
/// Note that a Pose component must also be present.
#[derive(Clone, Copy, Debug)]
pub struct Collider {
    shape: ColliderShape,
}

/// The physical shape of a collider.
#[derive(Clone, Copy, Debug)]
pub enum ColliderShape {
    Circle {
        r: f64,
    },
    /// The rect collider stores its side lengths halved because this makes
    /// intersection tests easier.
    Rect {
        hw: f64,
        hh: f64,
    },
}

impl Collider {
    /// Create a circle collider from a radius.
    pub fn new_circle(radius: f64) -> Self {
        Collider {
            shape: ColliderShape::Circle { r: radius },
        }
    }

    /// Create a rect collider with both sides set to the same length.
    pub fn new_square(side_length: f64) -> Self {
        Collider::new_rect(side_length, side_length)
    }

    /// Create a rect collider with two different side lengths.
    pub fn new_rect(width: f64, height: f64) -> Self {
        let hw = width / 2.0;
        let hh = height / 2.0;
        Collider {
            shape: ColliderShape::Rect { hw, hh },
        }
    }

    pub fn shape(&self) -> &ColliderShape {
        &self.shape
    }

    pub fn area(&self) -> f64 {
        match self.shape {
            ColliderShape::Circle { r } => std::f64::consts::PI * r * r,
            ColliderShape::Rect { hw, hh } => 4.0 * hw * hh,
        }
    }

    pub fn moment_of_inertia_coef(&self) -> f64 {
        // from https://en.wikipedia.org/wiki/List_of_moments_of_inertia
        match self.shape {
            ColliderShape::Circle { r } => r * r / 2.0,
            ColliderShape::Rect { hw, hh } => (hw * hw + hh * hh) / 3.0,
        }
    }
}
