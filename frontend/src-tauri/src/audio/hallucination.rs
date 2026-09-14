//! Text-level hallucination audit for transcript rows (no audio needed).
//!
//! Every decoded segment passes [`audit`] before persistence (change
//! `whisper-hallucination-cleanup`). Thresholds are calibrated on the pinned
//! cde5c264 fixtures — 22/240 rows flagged pre-live, 15/173 in the current
//! snapshot, zero clean rows flagged; expected flag sets are pinned in
//! `tests/expected-flags.json` (keyed by each fixture's `row_sha256`).
//!
//! Triggers:
//! - U+FFFD replacement chars (lossy token decode corruption) — never in legitimate rows
//! - `MIN_NON_LATIN_CHARS` non-Latin script chars (Latin/Common/Inherited/Unknown are
//!   exempt, so "é/ñ" in names, em-dashes and emoji never trip it)
//! - token loop: ratio ≥ [`MIN_LOOP_RATIO`] AND top token ≥ [`MIN_LOOP_COUNT`]
//!   occurrences ("Okay." ×37 flags; 6× unpunctuated "yeah" backchannel does not)
//! - absurd rate ≥ [`ABSURD_WPS`] words/s on rows ≥ 0.8 s (insurance; never fired
//!   alone on the fixtures)

use std::collections::HashMap;

use unicode_script::Script;

/// Absolute count of non-Latin script chars that flags a row.
pub const MIN_NON_LATIN_CHARS: usize = 3;
/// Top-token / total-tokens ratio needed for a loop flag (with MIN_LOOP_COUNT).
pub const MIN_LOOP_RATIO: f32 = 0.5;
/// How many times the top token must occur for a loop flag (a 6× "yeah yeah
/// yeah yeah yeah yeah" burst is real speech; "Okay." ×37 is degeneration).
pub const MIN_LOOP_COUNT: usize = 10;
/// Minimum whitespace-token count before the loop trigger is evaluated.
pub const MIN_LOOP_TOKENS: usize = 6;
/// Words-per-second rate that flags a row (rows shorter than 0.8 s exempt).
pub const ABSURD_WPS: f32 = 25.0;
/// Minimum row duration (seconds) for the wps trigger.
pub const MIN_WPS_DURATION_S: f64 = 0.8;

#[derive(Debug, Clone, PartialEq)]
pub struct HallucinationReport {
    pub is_garbage: bool,
    pub non_latin: usize,
    pub fffd: usize,
    pub loop_ratio: f32,
    pub loop_count: usize,
    pub wps: f32,
}

fn is_suspect_script(ch: char) -> bool {
    // Anything that is not Latin or script-neutral counts: an English-meeting
    // transcript carrying Han/Hangul/Cyrillic/Thai/Arabic/... glyphs is salad.
    !matches!(
        Script::from(ch),
        Script::Latin | Script::Common | Script::Inherited | Script::Unknown
    )
}

/// Audit one transcript row's text against its time span.
pub fn audit(text: &str, start_ms: f64, end_ms: f64) -> HallucinationReport {
    let non_latin = text.chars().filter(|c| is_suspect_script(*c)).count();
    let fffd = text.matches('\u{FFFD}').count();

    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut loop_ratio = 0.0f32;
    let mut loop_count = 0usize;
    if tokens.len() >= MIN_LOOP_TOKENS {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for t in &tokens {
            *counts.entry(t).or_insert(0) += 1;
        }
        loop_count = counts.values().copied().max().unwrap_or(0);
        loop_ratio = loop_count as f32 / tokens.len() as f32;
    }

    let duration_s = (end_ms - start_ms) / 1000.0;
    let wps = if duration_s >= MIN_WPS_DURATION_S {
        tokens.len() as f32 / duration_s as f32
    } else {
        0.0
    };

    let is_garbage = fffd >= 1
        || non_latin >= MIN_NON_LATIN_CHARS
        || (loop_ratio >= MIN_LOOP_RATIO && loop_count >= MIN_LOOP_COUNT)
        || wps >= ABSURD_WPS;

    HallucinationReport {
        is_garbage,
        non_latin,
        fffd,
        loop_ratio,
        loop_count,
        wps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span_s(start_s: f64, dur_s: f64) -> (f64, f64) {
        (start_s * 1000.0, (start_s + dur_s) * 1000.0)
    }

    #[test]
    fn clean_english_passes_with_accents_and_punctuation() {
        let (s, e) = span_s(10.0, 4.0);
        let text = "It's a café résumé — naïve, but “fine” — Mr. Oñate said: costs, 42%, and more.";
        let r = audit(text, s, e);
        assert!(!r.is_garbage, "report: {r:?}");
        assert_eq!(r.non_latin, 0);
        assert_eq!(r.fffd, 0);
    }

    #[test]
    fn emoji_only_row_passes() {
        let (s, e) = span_s(10.0, 2.0);
        let r = audit("great job team 😀 🎉 done", s, e);
        assert!(!r.is_garbage, "report: {r:?}");
    }

    #[test]
    fn cjk_salad_flags() {
        let (s, e) = span_s(100.0, 3.0);
        let r = audit("Stud Derekまた goloper exception您 para capacities", s, e);
        assert!(r.is_garbage, "report: {r:?}");
        assert!(r.non_latin >= MIN_NON_LATIN_CHARS);
    }

    #[test]
    fn korean_and_cyrillic_salad_flags() {
        let (s, e) = span_s(100.0, 3.0);
        let r = audit("oppa 그렇ets ob 사람이 сноваなんだ advantages", s, e);
        assert!(r.is_garbage, "report: {r:?}");
    }

    #[test]
    fn two_quoted_cjk_chars_do_not_flag() {
        // Below the absolute-count threshold: a quote of a product name stays.
        let (s, e) = span_s(100.0, 3.0);
        let r = audit("we ship the 抖音 app now", s, e);
        assert!(!r.is_garbage, "report: {r:?}");
    }

    #[test]
    fn replacement_chars_flag() {
        let (s, e) = span_s(100.0, 2.0);
        let r = audit("ao aantly заг except13 Ple\u{FFFD}\u{FFFD}\u{FFFD} Select patrim", s, e);
        assert!(r.is_garbage, "report: {r:?}");
        assert_eq!(r.fffd, 3);
    }

    #[test]
    fn okay_loop_flags() {
        let (s, e) = span_s(971.0, 9.6);
        let text = "Okay. ".repeat(40);
        let r = audit(text.trim(), s, e);
        assert!(r.is_garbage, "report: {r:?}");
        assert_eq!(r.loop_count, 40);
    }

    #[test]
    fn short_yeah_backchannel_passes() {
        // 6 identical unpunctuated tokens: real speech, under MIN_LOOP_COUNT.
        let (s, e) = span_s(50.0, 2.0);
        let r = audit("yeah yeah yeah yeah yeah yeah", s, e);
        assert!(!r.is_garbage, "report: {r:?}");
    }

    #[test]
    fn yes_loop_flags() {
        let (s, e) = span_s(206.0, 22.3);
        let text = "Yes. ".repeat(50);
        let r = audit(text.trim(), s, e);
        assert!(r.is_garbage, "report: {r:?}");
    }

    #[test]
    fn absurd_rate_flags() {
        // 40 words in one second — no human speech does this.
        let (s, e) = span_s(10.0, 1.0);
        let text = (0..40).map(|i| format!("word{i}")).collect::<Vec<_>>().join(" ");
        let r = audit(&text, s, e);
        assert!(r.is_garbage, "report: {r:?}");
        assert!(r.wps >= ABSURD_WPS);
    }

    #[test]
    fn short_rows_skip_wps_check() {
        // 4 words in 0.2 s is a timestamp rounding artifact, not a flag.
        let (s, e) = span_s(10.0, 0.2);
        let r = audit("one two three four", s, e);
        assert!(!r.is_garbage, "report: {r:?}");
        assert_eq!(r.wps, 0.0);
    }

    #[test]
    fn empty_text_is_clean() {
        let (s, e) = span_s(10.0, 2.0);
        let r = audit("", s, e);
        assert!(!r.is_garbage);
    }
}
