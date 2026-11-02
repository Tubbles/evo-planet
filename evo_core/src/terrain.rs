//! Spherical terrain: uniform patch centres on a Fibonacci sphere (no poles,
//! no wrap seams), a noise elevation field with ridged mountains, a sea level
//! chosen from the water-coverage dial, local temperature from latitude and
//! altitude, and a displaced icosphere mesh for rendering.

use std::collections::HashMap;

// --------------------------------------------------------------------------
// Hash-based 3D value noise
// --------------------------------------------------------------------------

fn hash3(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6b343) ^ (y as u32).wrapping_mul(0xd8163841) ^ (z as u32).wrapping_mul(0xcb1ab31f) ^ seed.wrapping_mul(0x9e3779b9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a2d39);
    h ^= h >> 15;
    (h as f32 / u32::MAX as f32) * 2.0 - 1.0
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Value noise in [-1, 1].
pub fn noise3(p: [f32; 3], seed: u32) -> f32 {
    let (x, y, z) = (p[0], p[1], p[2]);
    let (xi, yi, zi) = (x.floor(), y.floor(), z.floor());
    let (fx, fy, fz) = (smooth(x - xi), smooth(y - yi), smooth(z - zi));
    let (xi, yi, zi) = (xi as i32, yi as i32, zi as i32);
    let c = |dx: i32, dy: i32, dz: i32| hash3(xi + dx, yi + dy, zi + dz, seed);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = lerp(c(0, 0, 0), c(1, 0, 0), fx);
    let x10 = lerp(c(0, 1, 0), c(1, 1, 0), fx);
    let x01 = lerp(c(0, 0, 1), c(1, 0, 1), fx);
    let x11 = lerp(c(0, 1, 1), c(1, 1, 1), fx);
    let y0 = lerp(x00, x10, fy);
    let y1 = lerp(x01, x11, fy);
    lerp(y0, y1, fz)
}

fn fbm(p: [f32; 3], octaves: u32, lacunarity: f32, gain: f32, seed: u32) -> f32 {
    let mut amp = 1.0;
    let mut freq = 1.0;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for o in 0..octaves {
        sum += amp * noise3([p[0] * freq, p[1] * freq, p[2] * freq], seed.wrapping_add(o * 131));
        norm += amp;
        amp *= gain;
        freq *= lacunarity;
    }
    sum / norm
}

fn ridged(p: [f32; 3], octaves: u32, seed: u32) -> f32 {
    let mut amp = 1.0;
    let mut freq = 1.0;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for o in 0..octaves {
        let n = 1.0 - noise3([p[0] * freq, p[1] * freq, p[2] * freq], seed.wrapping_add(7 + o * 977)).abs();
        sum += amp * n * n;
        norm += amp;
        amp *= 0.5;
        freq *= 2.1;
    }
    sum / norm
}

/// Raw elevation in metres before sea level is chosen. Continents from low
/// frequency noise, ridged mountain belts where a mask is high, detail on top.
pub fn elevation_at(dir: [f32; 3], seed: u32) -> f32 {
    let p = |s: f32| [dir[0] * s, dir[1] * s, dir[2] * s];
    let continents = fbm(p(1.6), 4, 2.0, 0.5, seed); // -1..1
    let belt_mask = (fbm(p(2.4), 3, 2.0, 0.5, seed.wrapping_add(31)) * 1.6).clamp(0.0, 1.0);
    let mountains = ridged(p(4.5), 4, seed) * belt_mask; // 0..1
    let detail = fbm(p(9.0), 3, 2.0, 0.5, seed.wrapping_add(59)) * 0.15;
    // Land-biased mountains: stronger where continents are high.
    let land_bias = ((continents + 0.2) * 1.5).clamp(0.0, 1.0);
    continents * 2600.0 + mountains * 5200.0 * land_bias + detail * 800.0
}

// --------------------------------------------------------------------------
// Fibonacci sphere
// --------------------------------------------------------------------------

pub fn fibonacci_sphere(n: usize) -> Vec<[f32; 3]> {
    let golden = std::f32::consts::PI * (3.0 - 5f32.sqrt());
    (0..n)
        .map(|i| {
            let y = 1.0 - (i as f32 + 0.5) / n as f32 * 2.0;
            let r = (1.0 - y * y).max(0.0).sqrt();
            let theta = golden * i as f32;
            [r * theta.cos(), y, r * theta.sin()]
        })
        .collect()
}

pub fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// K nearest neighbours by angular distance (brute force; N is small).
pub fn nearest_neighbours(points: &[[f32; 3]], k: usize) -> Vec<Vec<usize>> {
    let n = points.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut best: Vec<(f32, usize)> = Vec::with_capacity(k + 1);
        for j in 0..n {
            if i == j {
                continue;
            }
            let d = dot(points[i], points[j]);
            if best.len() < k {
                best.push((d, j));
                best.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
            } else if d > best[k - 1].0 {
                best[k - 1] = (d, j);
                best.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
            }
        }
        out.push(best.into_iter().map(|(_, j)| j).collect());
    }
    out
}

/// Nearest patch index for an arbitrary direction.
pub fn nearest_patch(points: &[[f32; 3]], dir: [f32; 3]) -> usize {
    let mut best = 0;
    let mut bd = -2.0;
    for (i, p) in points.iter().enumerate() {
        let d = dot(*p, dir);
        if d > bd {
            bd = d;
            best = i;
        }
    }
    best
}

// --------------------------------------------------------------------------
// Icosphere mesh
// --------------------------------------------------------------------------

pub struct Icosphere {
    pub positions: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-9);
    [v[0] / l, v[1] / l, v[2] / l]
}

pub fn icosphere(detail: u32) -> Icosphere {
    let t = (1.0 + 5f32.sqrt()) / 2.0;
    let mut positions: Vec<[f32; 3]> = vec![
        [-1.0, t, 0.0],
        [1.0, t, 0.0],
        [-1.0, -t, 0.0],
        [1.0, -t, 0.0],
        [0.0, -1.0, t],
        [0.0, 1.0, t],
        [0.0, -1.0, -t],
        [0.0, 1.0, -t],
        [t, 0.0, -1.0],
        [t, 0.0, 1.0],
        [-t, 0.0, -1.0],
        [-t, 0.0, 1.0],
    ]
    .into_iter()
    .map(normalize)
    .collect();
    let mut faces: Vec<[u32; 3]> = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    for _ in 0..detail {
        let mut cache: HashMap<(u32, u32), u32> = HashMap::new();
        let mut next = Vec::with_capacity(faces.len() * 4);
        let mut mid = |a: u32, b: u32, positions: &mut Vec<[f32; 3]>| -> u32 {
            let key = if a < b { (a, b) } else { (b, a) };
            if let Some(&i) = cache.get(&key) {
                return i;
            }
            let pa = positions[a as usize];
            let pb = positions[b as usize];
            let m = normalize([(pa[0] + pb[0]) * 0.5, (pa[1] + pb[1]) * 0.5, (pa[2] + pb[2]) * 0.5]);
            positions.push(m);
            let i = (positions.len() - 1) as u32;
            cache.insert(key, i);
            i
        };
        for f in &faces {
            let a = mid(f[0], f[1], &mut positions);
            let b = mid(f[1], f[2], &mut positions);
            let c = mid(f[2], f[0], &mut positions);
            next.push([f[0], a, c]);
            next.push([f[1], b, a]);
            next.push([f[2], c, b]);
            next.push([a, b, c]);
        }
        faces = next;
    }
    let indices = faces.iter().flat_map(|f| f.iter().copied()).collect();
    Icosphere { positions, indices }
}

/// Local mean temperature: planet mean, warmer at the equator and colder at
/// the poles (about 45 C span on Earth), minus a 6.5 C/km lapse rate above
/// sea level.
pub fn local_temperature(mean_c: f32, dir: [f32; 3], elevation_m: f32) -> f32 {
    let lat = dir[1].clamp(-1.0, 1.0).asin().abs() / (std::f32::consts::PI / 2.0); // 0 equator .. 1 pole
    let latitudinal = 12.0 - 45.0 * lat * lat;
    mean_c + latitudinal - 6.5 * (elevation_m.max(0.0) / 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fibonacci_points_are_unit_and_spread() {
        let pts = fibonacci_sphere(500);
        for p in &pts {
            assert!((dot(*p, *p) - 1.0).abs() < 1e-4);
        }
        let nb = nearest_neighbours(&pts, 6);
        assert_eq!(nb.len(), 500);
        assert!(nb.iter().all(|v| v.len() == 6));
    }

    #[test]
    fn icosphere_counts() {
        let ico = icosphere(3);
        assert_eq!(ico.positions.len(), 642);
        assert_eq!(ico.indices.len(), 1280 * 3);
    }

    #[test]
    fn elevation_varies_and_peaks_are_cold() {
        let pts = fibonacci_sphere(300);
        let e: Vec<f32> = pts.iter().map(|p| elevation_at(*p, 7)).collect();
        let max = e.iter().cloned().fold(f32::MIN, f32::max);
        let min = e.iter().cloned().fold(f32::MAX, f32::min);
        assert!(max - min > 2000.0, "range {min}..{max}");
        assert!(local_temperature(15.0, [1.0, 0.0, 0.0], 4000.0) < local_temperature(15.0, [1.0, 0.0, 0.0], 0.0));
        assert!(local_temperature(15.0, [0.0, 1.0, 0.0], 0.0) < local_temperature(15.0, [1.0, 0.0, 0.0], 0.0));
    }
}
