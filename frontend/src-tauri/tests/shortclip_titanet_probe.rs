//! Can TitaNet separate voices on sub-second clips? (S7 residual: one source
//! row 32.51-40.24s holds "Yeah. Gotcha. Where is UserC? I don't know. Let
//! me ping him. Okay." and the render keeps the middle four under UserB's
//! badge; the user's ear verdict says Gotcha and "I don't know" are UserA.)
//! Embeds each atom at its true token wall — plus padded variants — and
//! scores against the enrolled fingerprints. Known-true controls calibrate
//! what a decided margin looks like at each duration.
//!
//! Run: MEETIFY_LIVE_DIAG=1 cargo test --release --test shortclip_titanet_probe -- --ignored --nocapture

#![cfg(test)]

use app_lib::audio::speaker::nemo_extractor::NemoEmbeddingExtractor;
use app_lib::audio::speaker::run_assembly::cosine;
use serde::Deserialize;
use sqlx::Row;

const AUDIO: &str =
    "Music/local-recordings/Meeting 2026-06-22_16-04-01_2026-06-22_14-04/audio.mp4";
const DB_PATH: &str = "AppData/Roaming/com.meetily.ai/meeting_minutes.sqlite";
const MODELS_DIR: &str = ".meetily-models";
const SAMPLE_RATE: usize = 16_000;

fn home() -> String {
    std::env::var("USERPROFILE").unwrap()
}

#[derive(Deserialize)]
struct Fixture {
    rows: Vec<FixtureRow>,
}

#[derive(Deserialize)]
struct FixtureRow {
    #[allow(dead_code)]
    start_ms: i64,
    text: String,
    token_timestamps: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Voice {
    UserA,
    UserB,
}

fn voice_tag(v: Voice) -> &'static str {
    match v {
        Voice::UserA => "UA",
        Voice::UserB => "UB",
    }
}

fn l2normalize(v: &mut [f32]) {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > f32::EPSILON {
        for x in v.iter_mut() {
            *x /= n;
        }
    }
}

fn embed_and_score(
    extractor: &NemoEmbeddingExtractor,
    samples: &[f32],
    means: &[(Voice, Vec<f32>)],
    s_ms: i64,
    e_ms: i64,
) -> Option<(Voice, f32, f32)> {
    let a = (s_ms as f64 / 1000.0 * SAMPLE_RATE as f64) as usize;
    let b = ((e_ms as f64 / 1000.0 * SAMPLE_RATE as f64) as usize).min(samples.len());
    let e = extractor.extract_embedding(&samples[a..b], SAMPLE_RATE as u32)?;
    let mut emb = e;
    l2normalize(&mut emb);
    let mut scored: Vec<(f32, Voice)> = means.iter().map(|(v, m)| (cosine(m, &emb), *v)).collect();
    scored.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
    Some((scored[0].1, scored[0].0, scored[1].0))
}

#[tokio::test]
#[ignore = "offline probe: MEETIFY_LIVE_DIAG=1 cargo test --release --test shortclip_titanet_probe -- --ignored --nocapture"]
async fn s7_atoms_scored_at_true_token_walls() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let samples: Vec<f32> = {
        let audio_path = format!("{}/{AUDIO}", home());
        let dir = std::path::Path::new(&audio_path).parent().unwrap().to_path_buf();
        let bytes = std::fs::read(dir.join("samples_16k.f32")).expect("samples cache");
        bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    };
    let fixture: Fixture = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/cde5c264_transcripts.json"
        ))
        .expect("read fixture"),
    )
    .expect("parse fixture");
    let row = fixture
        .rows
        .iter()
        .filter(|r| r.text.contains("Where is UserC?"))
        .min_by_key(|r| r.start_ms)
        .expect("S7 source row in fixture");
    #[derive(Deserialize)]
    struct TokenWord {
        word: String,
        start_ms: i64,
        end_ms: i64,
    }
    let tokens: Vec<TokenWord> = serde_json::from_str(
        row.token_timestamps
            .as_deref()
            .expect("S7 row has token walls"),
    )
    .expect("parse S7 tokens");

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .read_only(true)
                .filename(format!("{}/{}", home(), DB_PATH)),
        )
        .await
        .expect("db connect");
    let mut means: Vec<(Voice, Vec<f32>)> = Vec::new();
    for (id, vec) in
        app_lib::database::repositories::speaker::SpeakerRepository::list_enrollment_refs(&pool)
            .await
            .expect("enrollment refs")
    {
        let v = match id.as_str() {
            "speaker-447b345b-e4d3-41d3-a971-32863dc0c5d4" => Some(Voice::UserA),
            "speaker-0a2ba31a-6f2c-4c13-9a64-05e0a24fed0a" => Some(Voice::UserB),
            _ => None,
        };
        if let Some(v) = v {
            let mut vec = vec;
            l2normalize(&mut vec);
            means.push((v, vec));
        }
    }
    drop(pool);
    assert_eq!(means.len(), 2, "need both enrolled fingerprints");

    let extractor = NemoEmbeddingExtractor::new(&format!(
        "{}/{MODELS_DIR}/{}",
        home(),
        app_lib::audio::speaker::model_download::embedding_filename()
    ))
    .expect("embedding model");

    // Atom walls straight from the token list (absolute ms): Yeah[32709..33499]
    // Gotcha[33709..34499] Where..?[34719..35509] I don't know[35509..36069].
    let phrase = |from: &str, to: &str| -> (i64, i64) {
        let f = tokens
            .iter()
            .position(|t| t.word.trim() == from)
            .unwrap_or_else(|| panic!("token {from:?}"));
        let t = tokens
            .iter()
            .position(|x| x.word.trim() == to)
            .unwrap_or_else(|| panic!("token {to:?}"));
        (tokens[f].start_ms, tokens[t].end_ms)
    };
    let atoms: Vec<(&str, i64, i64, Option<Voice>)> = vec![
        ("Yeah", 32_709, 33_499, None),
        ("Gotcha", 33_709, 34_499, Some(Voice::UserA)),
        (
            "Where is UserC?",
            phrase("Where", "?").0,
            phrase("Where", "?").1,
            Some(Voice::UserB),
        ),
        ("I don't know", phrase("I", "know").0, phrase("I", "know").1, Some(Voice::UserA)),
    ];

    eprintln!("\n===== S7 atoms at true token walls (+pad variants) =====");
    eprintln!("ear truth: Gotcha + \"I don't know\" = UserA; \"Where is UserC?\" = UserB; production vote margin bar = 0.05");
    for (name, s, e, want) in &atoms {
        for pad_ms in [0i64, 100, 200] {
            match embed_and_score(&extractor, &samples, &means, s - pad_ms, e + pad_ms) {
                Some((winner, top, second)) => eprintln!(
                    "  {:<18} {:>5}+{pad_ms:>3}ms  UA={:+.3}  UB={:+.3}  -> {} (want {:?})  margin={:+.3}{}",
                    name,
                    e - s,
                    top.min(second),
                    top.max(second),
                    voice_tag(winner),
                    want.map(voice_tag).unwrap_or("?"),
                    top - second,
                    if top - second >= 0.05 { "" } else { "  < ABSTAIN" },
                ),
                None => eprintln!(
                    "  {name:<18} {:>5}+{pad_ms:>3}ms  extractor: silent/refused",
                    e - s
                ),
            }
        }
        eprintln!();
    }

    eprintln!("===== CONTROLS (known-true voices) =====");
    for (text, s, e, want) in [
        ("Let me ping him.", 36_719, 38_259, Voice::UserA),
        ("How's it going?", 5_870, 7_210, Voice::UserB),
    ] {
        match embed_and_score(&extractor, &samples, &means, s, e) {
            Some((winner, top, second)) => eprintln!(
                "  {:<18} {:>5}ms  UA={:+.3}  UB={:+.3}  -> {} (want {})  margin={:+.3}",
                text,
                e - s,
                top.min(second),
                top.max(second),
                voice_tag(winner),
                voice_tag(want),
                top - second
            ),
            None => eprintln!("  {text:<18} extractor: silent/refused"),
        }
    }

    // ── Ear ruling 2026-09-24: "I don't know" (35.509-36.069) is USERA, but
    // the whole-atom embedding reads UserB (0.32 margin, UA cosine 0.04).
    // Overlap is the suspect: UserA spoke over UserB's tail. Question for
    // the algorithm: does ANY sub-window of the atom carry UserA?
    eprintln!("\n===== SUB-WINDOW SCAN of 'I don't know' (35.2-36.3) =====");
    for win_ms in [200i64, 250, 300] {
        let mut t = 35_200i64;
        eprintln!("  -- window {win_ms}ms, 50ms stride --");
        while t + win_ms <= 36_300 {
            match embed_and_score(&extractor, &samples, &means, t, t + win_ms) {
                Some((winner, top, second)) => eprintln!(
                    "  [{:.2}-{:.2}] UA={:+.3} UB={:+.3} -> {} margin={:+.3}{}",
                    t as f64 / 1000.0,
                    (t + win_ms) as f64 / 1000.0,
                    top.min(second),
                    top.max(second),
                    voice_tag(winner),
                    top - second,
                    if top - second >= 0.05 { "" } else { "  ABSTAIN" },
                ),
                None => eprintln!("  [{:.2}-{:.2}] silent", t as f64 / 1000.0, (t + win_ms) as f64 / 1000.0),
            }
            t += 50;
        }
    }

    // Enriched UserA reference: mean of two ear+model-agreed UserA spans
    // (the "Gotcha." wall and "Let me ping him."). Does a stronger UA ref
    // recover the answer atom, or is the clip UserB-dominated throughout?
    eprintln!("\n===== ENRICHED UA REF rescore =====");
    let mut parts: Vec<Vec<f32>> = Vec::new();
    for (s, e) in [(33_709i64, 34_499i64), (36_719, 38_259)] {
        let a = (s as f64 / 1000.0 * SAMPLE_RATE as f64) as usize;
        let b = (e as f64 / 1000.0 * SAMPLE_RATE as f64) as usize;
        if let Some(e) = extractor.extract_embedding(&samples[a..b], SAMPLE_RATE as u32) {
            parts.push(e);
        }
    }
    let dim = parts[0].len();
    let mut cr2 = vec![0.0f32; dim];
    for p in &parts {
        for (m, x) in cr2.iter_mut().zip(p.iter()) {
            *m += x;
        }
    }
    l2normalize(&mut cr2);
    let cr_old = &means.iter().find(|(v, _)| *v == Voice::UserA).expect("UA ref").1;
    let means2: Vec<(Voice, Vec<f32>)> = vec![
        (Voice::UserA, cr2.clone()),
        (Voice::UserB, means.iter().find(|(v, _)| *v == Voice::UserB).expect("UB ref").1.clone()),
    ];
    for (name, s, e) in [
        ("I don't know (atom)", 35_509i64, 36_069i64),
        ("Gotcha (sanity UA)", 33_709, 34_499),
        ("Where is UserC?", 34_719, 35_509),
        ("Let me ping him (sanity)", 36_719, 38_259),
    ] {
        for (tag, m) in [("old", cr_old), ("new", &cr2)] {
            let a = (s as f64 / 1000.0 * SAMPLE_RATE as f64) as usize;
            let b = (e as f64 / 1000.0 * SAMPLE_RATE as f64) as usize;
            if let Some(e) = extractor.extract_embedding(&samples[a..b], SAMPLE_RATE as u32) {
                let mut emb = e;
                l2normalize(&mut emb);
                let cy = means
                    .iter()
                    .find(|(v, _)| *v == Voice::UserB)
                    .map(|(_, m)| cosine(m, &emb))
                    .expect("UB");
                let cr = cosine(m, &emb);
                eprintln!(
                    "  {name:<24} UA({tag})={cr:+.3}  UB={cy:+.3}  -> {} margin={:+.3}",
                    if cr > cy { "UA" } else { "UB" },
                    cr - cy
                );
            }
        }
        eprintln!();
    }
}
