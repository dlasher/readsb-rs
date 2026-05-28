//! Histogram-based noise floor estimation for adaptive preamble threshold.
//!
//! Instead of sorting the entire buffer to find the noise percentile (O(n log n)),
//! this uses a 256-bin histogram to estimate the noise floor in O(n) time.
//! For u8 magnitude format (CU8 input), the histogram is exact.
//! For other formats, a scaling factor normalizes the percentile.

/// Maximum value for u8 magnitude buckets
const HISTOGRAM_BUCKETS: usize = 256;

/// Noise floor estimation result
#[derive(Clone, Copy, Debug)]
pub struct NoiseFloor {
    /// Estimated noise floor (magnitude value)
    pub floor: u32,
    /// Median (50th percentile)
    pub median: u32,
    /// 75th percentile (used as noise floor proxy)
    pub p75: u32,
}

/// Compute histogram bins for magnitude buffer.
/// For u8 format: bin[mag[i]]++.
/// Returns the highest non-empty bin for bounds checking.
fn compute_histogram(mag: &[u16]) -> ([u32; HISTOGRAM_BUCKETS], u16) {
    let mut histogram = [0u32; HISTOGRAM_BUCKETS];
    let mut max_val: u16 = 0;

    for &v in mag {
        let v_clipped = v.min((HISTOGRAM_BUCKETS - 1) as u16);
        histogram[v_clipped as usize] += 1;
        if v_clipped > max_val {
            max_val = v_clipped;
        }
    }

    (histogram, max_val)
}

/// Find the k-th percentile from histogram bins.
fn histogram_percentile(histogram: &[u32; HISTOGRAM_BUCKETS], k: u32) -> u32 {
    let total: usize = histogram.iter().map(|&c| c as usize).sum();
    if total == 0 {
        return 0;
    }
    let target = (total as f64 * k as f64 / 100.0).ceil() as usize;
    let mut cumulative = 0usize;

    for (i, &count) in histogram.iter().enumerate() {
        cumulative += count as usize;
        if cumulative >= target {
            return i as u32;
        }
    }

    // Fallback: return last non-zero bin
    (HISTOGRAM_BUCKETS - 1) as u32
}

/// Estimate noise floor from magnitude buffer.
/// Returns NoiseFloor with median and 75th percentile estimates.
/// Uses histogram for O(n) performance instead of full sort.
pub fn estimate_noise_floor(mag: &[u16]) -> NoiseFloor {
    let (histogram, _max_val) = compute_histogram(mag);
    let total: usize = histogram.iter().map(|&c| c as usize).sum();
    if total == 0 {
        return NoiseFloor {
            floor: 20,
            median: 15,
            p75: 20,
        };
    }

    let median = histogram_percentile(&histogram, 50);
    let p75 = histogram_percentile(&histogram, 75);

    NoiseFloor {
        floor: p75,
        median,
        p75,
    }
}

/// Compute adaptive threshold for a given pass.
/// threshold = base_threshold * (margin ^ pass)
/// where margin is a factor < 1.0 (default 0.8).
/// Minimum threshold = noise_floor * 1.5 to avoid noise spikes.
pub fn adaptive_threshold(pass: u32, base_threshold: f32, margin: f32, noise_floor: u32) -> u32 {
    let scale = margin.powi(pass as i32);
    let calculated = (base_threshold * scale) as u32;
    let minimum = (noise_floor as f32 * 1.5) as u32;
    calculated.max(minimum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_histogram_exact_percentile() {
        // 100 samples: 30 zeros, 30 ones, 40 twos
        // 75th percentile should be 2
        let mag: Vec<u16> = vec![
            vec![0u16; 30],
            vec![1u16; 30],
            vec![2u16; 40],
        ].into_iter().flatten().collect();

        let (_, _) = compute_histogram(&mag);
        let floor = estimate_noise_floor(&mag);
        
        assert_eq!(floor.median, 1); // 50th percentile = 1
        assert_eq!(floor.p75, 2); // 75th percentile = 2
    }

    #[test]
    fn test_adaptive_threshold_scales() {
        let base: f32 = 58.0;
        let margin: f32 = 0.8;
        let noise: u32 = 20;

        // Pass 0: 58.0 * 0.8^0 = 58.0
        assert_eq!(adaptive_threshold(0, base, margin, noise), 58);
        // Pass 1: 58.0 * 0.8^1 = 46.4 -> 46
        assert_eq!(adaptive_threshold(1, base, margin, noise), 46);
        // Pass 2: 58.0 * 0.8^2 = 37.12 -> 37
        assert_eq!(adaptive_threshold(2, base, margin, noise), 37);
    }

    #[test]
    fn test_adaptive_threshold_has_minimum() {
        // If noise floor is high, threshold should not go below 1.5x noise
        let noise: u32 = 40; // high noise floor
        let base: f32 = 10.0;
        let margin: f32 = 0.8;

        // Base = 10, margin^5 = 0.32768, calculated = 3.2
        // Minimum = 40 * 1.5 = 60
        let result = adaptive_threshold(5, base, margin, noise);
        assert!(result >= 60);
    }

    #[test]
    fn test_estimate_noise_floor_empty() {
        let mag: Vec<u16> = vec![];
        let floor = estimate_noise_floor(&mag);
        assert_eq!(floor.floor, 20);
        assert_eq!(floor.median, 15);
    }

    #[test]
    fn test_estimate_noise_floor_high_values() {
        // Values above 255 should be capped to 255, not skipped
        let mut mag = vec![5u16; 100];
        mag.extend(vec![300u16; 50]);
        let floor = estimate_noise_floor(&mag);
        
        // With skip: total=100, median=5, p75=5
        // With cap: total=150, 75th percentile ≈ 255 (first 100 are 5, next 50 are 255)
        assert!(floor.p75 >= 100, "High signal values should not be silently skipped");
    }
}
