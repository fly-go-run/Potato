//! Measures core/database readiness, not native window paint or model latency.
fn main() {
    let root = tempfile::tempdir().unwrap();
    let mut samples = Vec::new();
    for _ in 0..25 {
        let started = std::time::Instant::now();
        let runtime = potato_core::Runtime::open(root.path()).unwrap();
        samples.push(started.elapsed().as_secs_f64() * 1000.0);
        drop(runtime);
    }
    let first = samples[0];
    samples.sort_by(f64::total_cmp);
    println!("Core readiness only: first={first:.2} ms, p50={:.2} ms, p95={:.2} ms (25 opens, same process)", samples[12], samples[23]);
}
