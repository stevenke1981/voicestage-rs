use std::{hint::black_box, path::Path, time::Instant};
use voicestage::model::{Project, atomic_write, new_id};
pub fn run(path: &Path) {
    let mut p = Project::template();
    let base = p.segments[0].clone();
    for _ in 0..498 {
        let mut s = base.clone();
        s.id = new_id();
        p.segments.push(s);
    }
    let bytes = serde_json::to_vec(&p).unwrap();
    let mut trials = Vec::new();
    for _ in 0..5 {
        let now = Instant::now();
        for _ in 0..200 {
            let p: Project = serde_json::from_slice(black_box(&bytes)).unwrap();
            p.validate().unwrap();
            black_box(serde_json::to_vec(&p).unwrap());
            black_box(p.utf8_bytes());
        }
        trials.push(now.elapsed().as_micros());
    }
    let result = serde_json::json!({"workload":"5 trials; each 200 parses/validations/serializations/UTF8 counts of 500 CJK dialogue lines","trials_us":trials});
    let _ = atomic_write(path, &serde_json::to_vec_pretty(&result).unwrap());
}
