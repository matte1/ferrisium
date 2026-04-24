//! Double-precision math primitives used by renderer-agnostic space APIs.
//!
//! These small types avoid coupling `ferrisium_core` to a rendering math crate
//! while keeping positions, velocities, and orientations in `f64`.

/// Three-dimensional vector used for celestial positions and velocities.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3d {
    /// X component.
    pub x: f64,
    /// Y component.
    pub y: f64,
    /// Z component.
    pub z: f64,
}

impl Vec3d {
    /// Zero vector.
    pub const ZERO: Self = Self::splat(0.0);

    /// Creates a vector from explicit components.
    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Creates a vector where all components share the same value.
    #[must_use]
    pub const fn splat(value: f64) -> Self {
        Self::new(value, value, value)
    }

    /// Scales each vector component by the provided scalar.
    #[must_use]
    pub const fn scale(self, scale: f64) -> Self {
        Self::new(self.x * scale, self.y * scale, self.z * scale)
    }
}

/// Double-precision quaternion used for renderer-agnostic frame orientation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuatD {
    /// X component of the vector part.
    pub x: f64,
    /// Y component of the vector part.
    pub y: f64,
    /// Z component of the vector part.
    pub z: f64,
    /// Scalar component.
    pub w: f64,
}

impl QuatD {
    /// Identity rotation.
    pub const IDENTITY: Self = Self::from_xyzw(0.0, 0.0, 0.0, 1.0);

    /// Creates a quaternion from vector and scalar components.
    #[must_use]
    pub const fn from_xyzw(x: f64, y: f64, z: f64, w: f64) -> Self {
        Self { x, y, z, w }
    }

    /// Returns the squared quaternion length.
    #[must_use]
    pub fn length_squared(self) -> f64 {
        self.x.mul_add(
            self.x,
            self.y
                .mul_add(self.y, self.z.mul_add(self.z, self.w * self.w)),
        )
    }

    /// Returns a finite unit quaternion, falling back to identity for invalid input.
    #[must_use]
    pub fn normalized(self) -> Self {
        let length_squared = self.length_squared();
        if !length_squared.is_finite() || length_squared <= 0.0 {
            return Self::IDENTITY;
        }

        let inverse_length = length_squared.sqrt().recip();
        Self::from_xyzw(
            self.x * inverse_length,
            self.y * inverse_length,
            self.z * inverse_length,
            self.w * inverse_length,
        )
    }

    /// Rotates a vector by this quaternion after normalizing the quaternion.
    ///
    /// Frame-orientation results are expected to be unit quaternions, but this
    /// method normalizes defensively so provider or test data cannot scale the
    /// vector by accident.
    #[must_use]
    pub fn rotate_vector(self, vector: Vec3d) -> Vec3d {
        let q = self.normalized();
        let q_vector = Vec3d::new(q.x, q.y, q.z);
        let uv = cross(q_vector, vector);
        let uuv = cross(q_vector, uv);

        Vec3d::new(
            vector.x + 2.0 * (q.w * uv.x + uuv.x),
            vector.y + 2.0 * (q.w * uv.y + uuv.y),
            vector.z + 2.0 * (q.w * uv.z + uuv.z),
        )
    }
}

impl Default for QuatD {
    fn default() -> Self {
        Self::IDENTITY
    }
}

fn cross(left: Vec3d, right: Vec3d) -> Vec3d {
    Vec3d::new(
        left.y.mul_add(right.z, -left.z * right.y),
        left.z.mul_add(right.x, -left.x * right.z),
        left.x.mul_add(right.y, -left.y * right.x),
    )
}

#[cfg(test)]
mod tests {
    use super::{QuatD, Vec3d};

    fn assert_close(lhs: f64, rhs: f64) {
        assert!(
            (lhs - rhs).abs() <= 1.0e-9,
            "float mismatch: lhs={lhs}, rhs={rhs}"
        );
    }

    #[test]
    fn quaternion_rotation_maps_vectors_between_frames() {
        let half_angle = std::f64::consts::FRAC_PI_4;
        let rotation = QuatD::from_xyzw(0.0, half_angle.sin(), 0.0, half_angle.cos());
        let rotated = rotation.rotate_vector(Vec3d::new(0.0, 0.0, 1.0));

        assert_close(rotated.x, 1.0);
        assert_close(rotated.y, 0.0);
        assert!(rotated.z.abs() <= 1.0e-9);
    }

    #[test]
    fn quaternion_rotation_normalizes_before_rotating() {
        let scaled_identity = QuatD::from_xyzw(0.0, 0.0, 0.0, 2.0);
        let rotated = scaled_identity.rotate_vector(Vec3d::new(1.0, 2.0, 3.0));

        assert_close(rotated.x, 1.0);
        assert_close(rotated.y, 2.0);
        assert_close(rotated.z, 3.0);
    }
}
