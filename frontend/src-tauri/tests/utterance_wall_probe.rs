//! Utterance-wall probe (4.3 follow-up, clip-02 ear ruling): the gate v4
//! render came back byte-identical to the pre-fix run — no window split —
//! so the energy splitter never saw the inter-utterance gaps. Prime
//! suspect: the port RMS-normalizes its streams, so a ~24s gap carries the
//! OTHER voice's bleed above the p90-anchored floor. This probe separates
//! the clip-02 span directly and prints each stream's frame-RMS profile
//! and speech-run structure at several floor ratios, so the dial moves on
//! evidence.
//!
//! MEETIFY_LIVE_DIAG=1 cargo test --test utterance_wall_probe -- --ignored --nocapture

use app_lib::audio::speaker::ports::VoiceSeparationPort;
use app_lib::audio::speaker::separation::ConvTasNetSeparator;

const SPAN: (f64, f64) = (367.39, 396.99);

#[tokio::test]
#[ignore = "live GPU probe: MEETIFY_LIVE_DIAG=1 cargo test --test utterance_wall_probe -- --ignored --nocapture"]
async fn probe_utterance_envelope_clip02() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let cache = std::path::PathBuf::from(std::env::var(
        "MEETIFY_SAMPLES_PATH",
    )
    .expect("MEETIFY_SAMPLES_PATH (path to the samples_16k.f32 cache)"));
    let bytes = std::fs::read(&cache).expect("samples cache");
    let samples: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    eprintln!(
        "PROBE: {} samples ({:.1}s)",
        samples.len(),
        samples.len() as f64 / 16_000.0
    );
    let separator = ConvTasNetSeparator::from_models_dir().expect("separation model");
    let streams = separator
        .separate(&samples, SPAN)
        .expect("separate clip-02 span");
    for (i, st) in streams.iter().enumerate() {
        let sr = 16_000.0f64;
        let frame = 1_600usize; // 100ms, same as the splitter
        let rms: Vec<f32> = st
            .samples
            .chunks_exact(frame)
            .map(|c| (c.iter().map(|v| v * v).sum::<f32>() / frame as f32).sqrt())
            .collect();
        let mut sorted = rms.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let pct = |p: f32| sorted[((sorted.len() - 1) as f32 * p) as usize];
        eprintln!(
            "PROBE stream{}: {} frames; p10={:.4} p50={:.4} p90={:.4} p99={:.4} max={:.4}",
            i,
            rms.len(),
            pct(0.10),
            pct(0.50),
            pct(0.90),
            pct(0.99),
            sorted[sorted.len() - 1]
        );
        // Speech-run structure at several floor ratios of p90.
        for ratio in [0.05f32, 0.10, 0.15, 0.25, 0.35, 0.50] {
            let floor = pct(0.90) * ratio;
            let mut runs: Vec<(usize, usize)> = Vec::new();
            let mut k = 0;
            while k < rms.len() {
                if rms[k] <= floor {
                    k += 1;
                    continue;
                }
                let s = k;
                while k < rms.len() && rms[k] > floor {
                    k += 1;
                }
                runs.push((s, k));
            }
            // Merge runs separated by <= 3s (the splitter's rule).
            let max_gap = 30usize; // 3s / 100ms
            let mut merged: Vec<(usize, usize)> = Vec::new();
            for r in runs {
                match merged.last_mut() {
                    Some(last) if r.0 - last.1 <= max_gap => last.1 = r.1,
                    _ => merged.push(r),
                }
            }
            let walls: Vec<String> = merged
                .iter()
                .map(|(a, b)| format!("[{:.1}-{:.1}]", *a as f64 / 10.0, *b as f64 / 10.0))
                .collect();
            eprintln!(
                "PROBE stream{} floor={:.2}*p90 ({:.4}): {} chunk(s) {}",
                i,
                ratio,
                floor,
                merged.len(),
                walls.join(" ")
            );
        }
        // A coarse energy trace every second, to see the gap's actual level.
        let per_sec: Vec<String> = rms
            .chunks(10)
            .map(|c| {
                let m = c.iter().cloned().fold(f32::MIN, f32::max);
                format!("{:.2}", m)
            })
            .collect();
        eprintln!("PROBE stream{} per-second max RMS: {}", i, per_sec.join(" "));
        // The PRODUCTION splitter's verdict on this exact stream.
        let spans =
            app_lib::audio::speaker::run_assembly::stream_utterance_spans(&st.samples);
        let walls: Vec<String> = spans
            .iter()
            .map(|(a, b)| format!("[{:.1}-{:.1}]", a, b))
            .collect();
        eprintln!(
            "PROBE stream{} stream_utterance_spans -> {} chunk(s) {}",
            i,
            spans.len(),
            walls.join(" ")
        );
        // Frame-level tail trace (the attested order: Participant B ~rel 27.0,
        // Participant A ~rel 28.0-28.4) — 100ms frames from rel 26.0s, with the
        // candidate floors marked.
        if i == 0 {
            let start = (26.0 * 10.0) as usize;
            let trace: Vec<String> = rms[start..rms.len().min(start + 36)]
                .iter()
                .map(|v| {
                    let m = *v;
                    format!("{:.3}", m)
                })
                .collect();
            eprintln!("PROBE stream{} tail frames (rel 26.0s+, 100ms each): {}", i, trace.join(" "));
        }
    }
}
