//! Sentence-aware speaker-turn assembly.
//!
//! Alignment is word-exact and splits rows at every speaker flip — including
//! sub-second backchannels — which leaves persisted text as fragments (77% of
//! rows on the reference meeting did not end in sentence punctuation). This
//! module re-assembles same-speaker neighbors into readable turns:
//!
//! - `detokenize` fixes whisper word-joined spacing ("How 's it going ?" →
//!   "How's it going?") mechanically — no repunctuation, no re-casing.
//! - `assemble_turns` merges adjacent same-speaker rows (gap ≤ 3 s) into one
//!   turn, drops punctuation-only rows, and always breaks on a speaker change
//!   (a mid-sentence interjection is real speech and stays its own row).

/// Maximum silence between two same-speaker rows for them to join one turn.
pub const MAX_MERGE_GAP_MS: i64 = 3_000;

/// A merged speaker turn.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeakerTurn {
    pub speaker: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

/// Borrowed view of one persisted transcript row.
#[derive(Debug, Clone, Copy)]
pub struct RowRef<'a> {
    pub speaker: &'a str,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: &'a str,
}

fn is_punct_only(token: &str) -> bool {
    !token.is_empty() && token.chars().all(|c| matches!(c, '.' | ',' | '!' | '?' | ';' | ':' | '%' | '…' | ')' | '(' | '"' | '\''))
}

/// Mechanical detokenization for whisper word-joined text. Tokens that are
/// pure punctuation glue to the previous word; tokens starting with an
/// apostrophe (contractions: 's 't 're 've 'll 'd 'm, possessives) glue to
/// the previous word; leading punctuation on the utterance is dropped.
pub fn detokenize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for token in text.split_whitespace() {
        if is_punct_only(token) {
            if out.is_empty() {
                continue; // drop leading punctuation-only tokens
            }
            out.push_str(token);
            continue;
        }
        if token.starts_with('\'') && !out.is_empty() {
            out.push_str(token);
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(token);
    }
    out
}

/// A merged speaker turn together with the indexes of the input rows it
/// absorbed. Input rows absent from every group were dropped as content-less.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnGroup {
    pub turn: SpeakerTurn,
    pub row_indexes: Vec<usize>,
}

/// Group time-ordered rows into speaker turns (see [`assemble_turns`] for the
/// rules). Exposes which input rows each turn absorbed so a caller persisting
/// to a database can update the first absorbed row and delete the rest.
pub fn assemble_groups(rows: &[RowRef<'_>]) -> Vec<TurnGroup> {
    let mut ordered: Vec<usize> = (0..rows.len()).collect();
    ordered.sort_by_key(|&i| (rows[i].start_ms, rows[i].end_ms));

    let mut groups: Vec<TurnGroup> = Vec::new();
    for &i in &ordered {
        let r = &rows[i];
        let text = detokenize(r.text);
        if !text.chars().any(|c| c.is_alphanumeric()) {
            continue; // punctuation-only (or empty) row: no content to keep
        }
        let merged = match groups.last_mut() {
            Some(g) if g.turn.speaker == r.speaker && r.start_ms - g.turn.end_ms <= MAX_MERGE_GAP_MS => {
                g.turn.text.push(' ');
                g.turn.text.push_str(&text);
                g.turn.end_ms = g.turn.end_ms.max(r.end_ms);
                g.row_indexes.push(i);
                true
            }
            _ => false,
        };
        if !merged {
            groups.push(TurnGroup {
                turn: SpeakerTurn {
                    speaker: r.speaker.to_string(),
                    start_ms: r.start_ms,
                    end_ms: r.end_ms,
                    text,
                },
                row_indexes: vec![i],
            });
        }
    }
    groups
}

/// Merge time-ordered rows into speaker turns. Rows of the same speaker join
/// while the gap stays within [`MAX_MERGE_GAP_MS`]; a speaker change or a
/// longer silence starts a new turn. Returns turns in time order.
pub fn assemble_turns(rows: &[RowRef<'_>]) -> Vec<SpeakerTurn> {
    assemble_groups(rows)
        .into_iter()
        .map(|g| g.turn)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SP0: &str = "Speaker 0";
    const SP1: &str = "Speaker 1";

    fn row<'a>(speaker: &'a str, start: i64, end: i64, text: &'a str) -> RowRef<'a> {
        RowRef { speaker, start_ms: start, end_ms: end, text }
    }

    // --- 1.1 detokenize ---

    #[test]
    fn detokenize_fixes_spacing_and_contractions() {
        assert_eq!(detokenize("How 's it going ?"), "How's it going?");
        assert_eq!(detokenize("I don 't like you 've aged"), "I don't like you've aged");
        assert_eq!(detokenize("Yeah , sure , sure"), "Yeah, sure, sure");
        assert_eq!(detokenize("wait ..."), "wait...");
        assert_eq!(detokenize("plain text"), "plain text");
    }

    #[test]
    fn detokenize_drops_leading_punctuation() {
        assert_eq!(detokenize(". Okay . I have updates"), "Okay. I have updates");
        assert_eq!(detokenize(",,"), "");
    }

    // --- 1.2 assemble_turns ---

    #[test]
    fn assembles_same_speaker_fragments_into_one_turn() {
        let rows = [
            row(SP0, 16_200, 20_180, ". Okay . I have some updates . Cool . On the"),
            row(SP0, 20_180, 26_110, "to , let 's , wait"),
        ];
        let turns = assemble_turns(&rows);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].speaker, SP0);
        assert_eq!(turns[0].start_ms, 16_200);
        assert_eq!(turns[0].end_ms, 26_110);
        assert_eq!(turns[0].text, "Okay. I have some updates. Cool. On the to, let's, wait");
    }

    #[test]
    fn speaker_flip_never_merges_and_backchannel_joins_own_speaker() {
        let rows = [
            row(SP1, 100_000, 102_000, "We 're going to do that"),
            row(SP1, 102_300, 102_800, "Yeah ,"),
            row(SP0, 102_900, 103_500, "your image search"),
            row(SP1, 103_600, 105_000, "Fine"),
        ];
        let turns = assemble_turns(&rows);
        assert_eq!(turns.len(), 3, "interjection splits, backchannel joins: {:?}", turns);
        assert_eq!(turns[0].speaker, SP1);
        assert_eq!(turns[0].text, "We're going to do that Yeah,");
        assert_eq!(turns[1].speaker, SP0);
        assert_eq!(turns[1].text, "your image search");
        assert_eq!(turns[2].text, "Fine");
    }

    #[test]
    fn gap_over_three_seconds_starts_new_turn() {
        let rows = [
            row(SP0, 0, 2_000, "First thought"),
            row(SP0, 6_000, 8_000, "Second thought"),
        ];
        let turns = assemble_turns(&rows);
        assert_eq!(turns.len(), 2);
    }

    #[test]
    fn punctuation_only_rows_are_dropped() {
        let rows = [
            row(SP0, 0, 1_420, ","),
            row(SP0, 5_870, 12_070, "How 's it going ?"),
        ];
        let turns = assemble_turns(&rows);
        assert_eq!(turns.len(), 1, "comma-only row gone, silence keeps turns apart");
        assert_eq!(turns[0].text, "How's it going?");
    }

    // --- 1.3 content preservation ---

    #[test]
    fn assembly_preserves_alphanumeric_content() {
        let rows = [
            row(SP0, 0, 1_000, ","),
            row(SP0, 1_000, 2_000, "How 's it going ?"),
            row(SP1, 2_100, 3_000, "Yeah , fine"),
            row(SP0, 9_000, 10_000, ". Okay"),
        ];
        let norm = |s: &str| -> String {
            let mut c: Vec<char> = s.chars().filter(|c| c.is_alphanumeric()).collect();
            c.sort_unstable();
            c.into_iter().collect()
        };
        let input: String = rows.iter().map(|r| norm(r.text)).collect();
        let output: String = assemble_turns(&rows).iter().map(|t| norm(&t.text)).collect();
        assert_eq!(input, output);
    }
}
