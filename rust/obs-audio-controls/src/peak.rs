//! Sample peak and true peak from `obs-audio-controls.c.h`.
//!
//! The true-peak path is the SSE Whittaker-Shannon 5x oversample, written
//! as scalar `f32` in the same multiply and add order. Lane 0 is the low
//! element of `_mm_set_ps`, which is the last argument.

/// `_mm_set_ps(-0.155915, 0.935489, 0.233872, -0.103943)`.
const M3: [f32; 4] = [-0.103_943, 0.233_872, 0.935_489, -0.155_915];
/// `_mm_set_ps(-0.216236, 0.756827, 0.504551, -0.189207)`.
const M1: [f32; 4] = [-0.189_207, 0.504_551, 0.756_827, -0.216_236];
/// `_mm_set_ps(-0.189207, 0.504551, 0.756827, -0.216236)`.
const P1: [f32; 4] = [-0.216_236, 0.756_827, 0.504_551, -0.189_207];
/// `_mm_set_ps(-0.103943, 0.233872, 0.935489, -0.155915)`.
const P3: [f32; 4] = [-0.155_915, 0.935_489, 0.233_872, -0.103_943];

/// `get_sample_peak()`: the largest absolute sample of the previous four
/// and of each whole group of four in `samples[..frames]`.
pub fn sample_peak(prev: [f32; 4], samples: &[f32], frames: usize) -> f32 {
    let mut peak = prev.map(f32::abs);
    // Whole groups only, as the C's `i + 3 < nr_samples`. One slice up front
    // lets the compiler drop the per-sample bounds checks and load a group
    // at once (#3005).
    let (groups, _) = samples[..frames / 4 * 4].as_chunks::<4>();
    for group in groups {
        for lane in 0..4 {
            peak[lane] = max_ps(peak[lane], group[lane].abs());
        }
    }
    hmax(peak)
}

/// `get_true_peak()`: [`sample_peak`] plus the 5x oversampled points
/// between the samples.
pub fn true_peak(prev: [f32; 4], samples: &[f32], frames: usize) -> f32 {
    let mut work = prev;
    let mut peak = prev.map(f32::abs);
    let (groups, _) = samples[..frames / 4 * 4].as_chunks::<4>();
    for group in groups {
        let mut new_work = *group;
        for lane in 0..4 {
            peak[lane] = max_ps(peak[lane], new_work[lane].abs());
        }
        for _ in 0..4 {
            shift_right_2ps(&mut new_work, &mut work);
            let intrp = cross(work, M3, M1, P1, P3);
            for lane in 0..4 {
                peak[lane] = max_ps(peak[lane], intrp[lane].abs());
            }
        }
    }
    hmax(peak)
}

/// Keep the last four samples for the next packet. A short packet shifts
/// the new samples in over the old ones.
pub(crate) fn store_last(prev: &mut [f32; 4], samples: &[f32], frames: usize) {
    match frames {
        0 => {}
        1 => {
            prev[0] = prev[1];
            prev[1] = prev[2];
            prev[2] = prev[3];
            prev[3] = samples[frames - 1];
        }
        2 => {
            prev[0] = prev[2];
            prev[1] = prev[3];
            prev[2] = samples[frames - 2];
            prev[3] = samples[frames - 1];
        }
        3 => {
            prev[0] = prev[3];
            prev[1] = samples[frames - 3];
            prev[2] = samples[frames - 2];
            prev[3] = samples[frames - 1];
        }
        _ => {
            prev[0] = samples[frames - 4];
            prev[1] = samples[frames - 3];
            prev[2] = samples[frames - 2];
            prev[3] = samples[frames - 1];
        }
    }
}

/// One lane of `_mm_max_ps(a, b)`: `a > b ? a : b`, so `b` when either is
/// NaN. `f32::max` returns the other operand instead, which keeps a lane's
/// peak where the C replaces it with the NaN (#3005). The comparison form
/// is also the one compilers turn into `maxps`.
#[inline]
fn max_ps(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

/// `hmax_ps`: `fmaxf` over the lanes, which skips a NaN like `f32::max`.
fn hmax(v: [f32; 4]) -> f32 {
    v[0].max(v[1]).max(v[2]).max(v[3])
}

/// `VECTOR_MATRIX_CROSS_PS`: `out[i] = dot(v, m_i)` after the transpose sum.
fn cross(v: [f32; 4], m0: [f32; 4], m1: [f32; 4], m2: [f32; 4], m3: [f32; 4]) -> [f32; 4] {
    [dot(v, m0), dot(v, m1), dot(v, m2), dot(v, m3)]
}

fn dot(v: [f32; 4], m: [f32; 4]) -> f32 {
    let mut acc = v[0] * m[0];
    acc += v[1] * m[1];
    acc += v[2] * m[2];
    acc += v[3] * m[3];
    acc
}

/// `SHIFT_RIGHT_2PS`. `msb` is the new group, `lsb` is the running window.
/// `[a,b,c,d | e,f,g,h]` becomes `[b,c,d,e | f,g,h,h]`.
fn shift_right_2ps(msb: &mut [f32; 4], lsb: &mut [f32; 4]) {
    let tmp = shuffle(*lsb, *msb, 3, 3, 0, 0);
    *lsb = shuffle(*lsb, tmp, 1, 2, 1, 2);
    *msb = shuffle(*msb, *msb, 1, 2, 3, 3);
}

/// `_mm_shuffle_ps`: lanes 0 and 1 from `a`, lanes 2 and 3 from `b`.
fn shuffle(a: [f32; 4], b: [f32; 4], i0: usize, i1: usize, i2: usize, i3: usize) -> [f32; 4] {
    [a[i0], a[i1], b[i2], b[i3]]
}
