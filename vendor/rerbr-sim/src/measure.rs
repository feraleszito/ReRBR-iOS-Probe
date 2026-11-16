#[derive(Debug, Clone, Copy, Default)]
pub struct Summary {
    pub steps: usize,
    pub mean_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
    pub wall_seconds: f64,
    pub simulated_seconds: f64,
    pub realtime_factor: f64,
}
pub fn summarize(samples: &[f64], dt: f32, wall: f64) -> Result<Summary, String> {
    if samples.is_empty()
        || !dt.is_finite()
        || dt <= 0.0
        || !wall.is_finite()
        || wall <= 0.0
        || samples.iter().any(|x| !x.is_finite() || *x < 0.0)
    {
        return Err("invalid physics timing samples".into());
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let percentile = |p: f64| sorted[((p * sorted.len() as f64).ceil() as usize).saturating_sub(1)] * 1000.0;
    let simulated = samples.len() as f64 * f64::from(dt);
    Ok(Summary {
        steps: samples.len(),
        mean_ms: samples.iter().sum::<f64>() / samples.len() as f64 * 1000.0,
        p95_ms: percentile(0.95),
        p99_ms: percentile(0.99),
        max_ms: sorted[sorted.len() - 1] * 1000.0,
        wall_seconds: wall,
        simulated_seconds: simulated,
        realtime_factor: simulated / wall,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_percentiles_and_actual_wall_factor() {
        let s = summarize(&[0.001, 0.002, 0.004, 0.003], 0.01, 0.02).unwrap();
        assert_eq!(s.mean_ms, 2.5);
        assert_eq!(s.p95_ms, 4.0);
        assert_eq!(s.p99_ms, 4.0);
        assert!((s.realtime_factor - 2.0).abs() < 1e-6);
    }
    #[test]
    fn invalid_timings_fail() {
        assert!(summarize(&[], 0.01, 1.0).is_err());
        assert!(summarize(&[f64::NAN], 0.01, 1.0).is_err());
        assert!(summarize(&[0.001], 0.0, 1.0).is_err());
    }
}
