//! Geometry of the "cake" board: a rounded-square ring cut into wedges,
//! Start at the bottom left and play running clockwise, like the original.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};
use std::sync::LazyLock;

/// Board centre in world space; the side panel takes the right part of the window.
pub const CENTER: Vec2 = Vec2::new(-262.0, 0.0);
/// Outer edge of the board's felt.
pub const FRAME: f32 = 436.0;
/// The squares span these radii.
pub const OUTER: f32 = 418.0;
pub const INNER: f32 = 252.0;
/// The track inside the squares – the pawns walk on it, it shows owners and race tokens.
pub const TRACK: f32 = 176.0;

/// Squareness of the board outline (2 = circle, larger = squarer).
const EXPONENT: f32 = 4.0;
/// Gap between neighbouring wedges, as a fraction of the outline.
const GAP: f32 = 0.0007;

/// Cumulative length of the outline at evenly spaced parameter angles, normalised to 1,
/// so squares can get equal slices of the outline like on the real board.
static ARC: LazyLock<Vec<f32>> = LazyLock::new(|| {
    const SAMPLES: usize = 4096;
    let mut acc = vec![0.0];
    for k in 1..=SAMPLES {
        let a = outline(TAU * (k - 1) as f32 / SAMPLES as f32, 1.0);
        let b = outline(TAU * k as f32 / SAMPLES as f32, 1.0);
        acc.push(acc[k - 1] + a.distance(b));
    }
    let total = acc[SAMPLES];
    acc.into_iter().map(|l| l / total).collect()
});

/// Parameter angle at fraction `f` of the outline (counter-clockwise from angle 0).
fn param_at(f: f32) -> f32 {
    let f = f.rem_euclid(1.0);
    let k = ARC.partition_point(|&l| l < f).clamp(1, ARC.len() - 1);
    let t = (f - ARC[k - 1]) / (ARC[k] - ARC[k - 1]);
    TAU * (k as f32 - 1.0 + t) / (ARC.len() - 1) as f32
}

/// Fraction of the outline at parameter angle `theta`.
fn fraction_at(theta: f32) -> f32 {
    let x = theta.rem_euclid(TAU) / TAU * (ARC.len() - 1) as f32;
    let k = (x as usize).min(ARC.len() - 2);
    ARC[k] + (ARC[k + 1] - ARC[k]) * (x - k as f32)
}

/// Where along the outline square `i` is centred; Start sits in the bottom-left corner
/// and the squares follow clockwise.
fn center_fraction(i: usize, count: usize) -> f32 {
    fraction_at(1.25 * PI) - i as f32 / count as f32
}

/// Point on the rounded square of "radius" `r` at parameter angle `theta`.
pub fn outline(theta: f32, r: f32) -> Vec2 {
    let (s, c) = theta.sin_cos();
    let e = 2.0 / EXPONENT;
    CENTER + Vec2::new(c.signum() * c.abs().powf(e), s.signum() * s.abs().powf(e)) * r
}

/// Middle of square `i` at radius `r`.
pub fn spot(i: usize, count: usize, r: f32) -> Vec2 {
    outline(param_at(center_fraction(i, count)), r)
}

/// Rotation that points a sprite's x axis outward at square `i`, flipped on
/// the left half so text never reads upside down.
pub fn radial_rotation(i: usize, count: usize) -> Quat {
    let dir = spot(i, count, OUTER) - CENTER;
    let a = dir.y.atan2(dir.x);
    Quat::from_rotation_z(if dir.x < -1.0 { a + PI } else { a })
}

/// The wedge of square `i` between radii `r_in` and `r_out`.
pub fn wedge(i: usize, count: usize, r_in: f32, r_out: f32) -> Mesh {
    let (c, half) = (center_fraction(i, count), 0.5 / count as f32 - GAP);
    band(c - half, c + half, r_in, r_out)
}

/// A full ring between two radii (`r_in` = 0 gives a filled shape).
pub fn ring(r_in: f32, r_out: f32) -> Mesh {
    band(0.0, 1.0, r_in, r_out)
}

/// The part of the ring between outline fractions `from` and `to`.
fn band(from: f32, to: f32, r_in: f32, r_out: f32) -> Mesh {
    let steps = ((to - from) * 240.0).ceil().max(4.0) as u32;
    let mut positions = Vec::new();
    for k in 0..=steps {
        let t = param_at(from + (to - from) * k as f32 / steps as f32);
        for r in [r_in, r_out] {
            let p = outline(t, r) - CENTER;
            positions.push([p.x, p.y, 0.0]);
        }
    }
    let indices = (0..steps).flat_map(|k| {
        let i = k * 2;
        [i, i + 1, i + 2, i + 1, i + 3, i + 2]
    });
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_indices(Indices::U32(indices.collect()))
}

/// Which square (if any) lies under a world position.
pub fn square_at(pos: Vec2, count: usize) -> Option<usize> {
    let p = pos - CENTER;
    let r = (p.x.abs().powf(EXPONENT) + p.y.abs().powf(EXPONENT)).powf(1.0 / EXPONENT);
    if !(INNER..=OUTER).contains(&r) {
        return None;
    }
    let h = EXPONENT / 2.0;
    let theta = ((p.y / r).signum() * (p.y / r).abs().powf(h)).atan2((p.x / r).signum() * (p.x / r).abs().powf(h));
    let steps = ((center_fraction(0, count) - fraction_at(theta)) * count as f32).round();
    Some((steps as i64).rem_euclid(count as i64) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_testing_inverts_layout() {
        for i in 0..40 {
            let mid = spot(i, 40, (INNER + OUTER) / 2.0);
            assert_eq!(square_at(mid, 40), Some(i));
        }
        assert_eq!(square_at(CENTER, 40), None);
    }
}
