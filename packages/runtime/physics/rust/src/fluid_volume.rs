use crate::MotionError;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SubmergedVolume {
    volume: f32,
    moment: [f32; 3],
}
impl SubmergedVolume {
    pub fn add_triangle(
        &mut self,
        points: [[f32; 3]; 3],
        plane: [f32; 4],
        epsilon: f32,
    ) -> Result<(), MotionError> {
        if points
            .iter()
            .flatten()
            .chain(&plane)
            .chain([&epsilon])
            .any(|v| !v.is_finite())
            || epsilon < 0.0
        {
            return Err(MotionError::NonFinite);
        }
        let projection =
            std::array::from_fn(|axis| (f64::from(plane[axis]) * f64::from(-plane[3])) as f32);
        let mut distances = [0.0; 3];
        let mut negative = 0;
        let mut positive_index = 0;
        let mut negative_index = 0;
        for index in (0..3).rev() {
            let p = points[index];
            let distance = (p[0] * plane[0] + p[1] * plane[1]) + (p[2] * plane[2] + plane[3]);
            if distance < 0.0 {
                negative += 1;
                negative_index = index;
                distances[index] = distance - epsilon;
            } else {
                positive_index = index;
                distances[index] = distance + epsilon;
            }
        }
        if negative == 3 {
            return Ok(());
        }
        let full = f64::from(pyramid(points, projection));
        let mut partial = full;
        let centroid_sum: [f32; 3] = match negative {
            0 => std::array::from_fn(|axis| {
                ((points[0][axis] + points[1][axis]) + points[2][axis]) + projection[axis]
            }),
            1 | 2 => {
                let index = if negative == 1 {
                    negative_index
                } else {
                    positive_index
                };
                let next = (index + 1) % 3;
                let last = (index + 2) % 3;
                let a = f64::from(distances[index] / (distances[index] - distances[next]));
                let b = f64::from(distances[index] / (distances[index] - distances[last]));
                let product = a * b;
                let weight = (3.0 - a) - b;
                let cut: [f32; 3] = std::array::from_fn(|axis| {
                    let first = (f64::from(points[index][axis]) * weight) as f32;
                    let second = (f64::from(first) + f64::from(points[next][axis]) * a) as f32;
                    (f64::from(second) + f64::from(points[last][axis]) * b) as f32
                });
                if negative == 2 {
                    partial *= product;
                    std::array::from_fn(|axis| cut[axis] + projection[axis])
                } else {
                    partial -= (full * a) * b;
                    let remaining = 1.0 - product;
                    if remaining < f64::from(epsilon) {
                        self.volume = 0.0;
                        self.moment = [0.0; 3];
                        return Ok(());
                    }
                    let inverse = 1.0 / remaining;
                    std::array::from_fn(|axis| {
                        let all = (points[0][axis] + points[1][axis]) + points[2][axis];
                        let dry = (f64::from(cut[axis]) * product) as f32;
                        (f64::from(all - dry) * inverse) as f32 + projection[axis]
                    })
                }
            }
            _ => unreachable!(),
        };
        let volume = (f64::from(self.volume) + partial) as f32;
        let moment: [f32; 3] = std::array::from_fn(|axis| {
            self.moment[axis] + (f64::from(centroid_sum[axis]) * partial) as f32
        });
        if !volume.is_finite() || moment.iter().any(|v| !v.is_finite()) {
            return Err(MotionError::NonFinite);
        }
        self.volume = volume;
        self.moment = moment;
        Ok(())
    }
    pub fn finish(self, epsilon: f32) -> Result<(f32, [f32; 3]), MotionError> {
        if !epsilon.is_finite() || epsilon < 0.0 {
            return Err(MotionError::NonFinite);
        }
        let center = if self.volume > epsilon {
            let factor = 0.25_f32 / self.volume;
            self.moment
                .map(|v| (f64::from(v) * f64::from(factor)) as f32)
        } else {
            self.moment
        };
        if center.iter().any(|v| !v.is_finite()) {
            return Err(MotionError::NonFinite);
        }
        Ok((self.volume, center))
    }
}
fn pyramid(points: [[f32; 3]; 3], projection: [f32; 3]) -> f32 {
    let a = std::array::from_fn::<_, 3, _>(|i| f64::from(points[2][i]) - f64::from(points[0][i]));
    let b = std::array::from_fn::<_, 3, _>(|i| f64::from(points[1][i]) - f64::from(points[0][i]));
    let normal = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let delta = std::array::from_fn::<_, 3, _>(|i| f64::from(points[0][i] - projection[i]));
    (((normal[1] * delta[1] + normal[0] * delta[0]) + normal[2] * delta[2]) * (-1.0 / 6.0)) as f32
}
