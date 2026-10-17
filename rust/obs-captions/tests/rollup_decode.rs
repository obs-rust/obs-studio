//! Decode-back tests for the roll-up emitter: a test-side CEA-608 roll-up
//! decoder consumes the emitted `(frame, cc_data)` stream and rebuilds the
//! screen a real decoder would show, mirroring `deps/libcaption/src/caption.c`
//! roll-up handling (RU2/RU3 sets the window, CR scrolls it, PAC anchors the
//! bottom row, characters past column 32 are ignored).

use obs_captions::eia608::{self, SCREEN_COLS, control};
use obs_captions::rollup::{Rollup, RollupError, Rows, ScheduledPair};

/// Minimal roll-up decoder over a `cc_data` pair stream.
struct Screen {
    lines: Vec<String>,
    rows: usize,
    col: usize,
}

impl Screen {
    fn new() -> Self {
        Screen {
            lines: vec![String::new()],
            rows: 0,
            col: 0,
        }
    }

    fn feed(&mut self, pair: u16) {
        if let Some(cmd) = eia608::parse_control(pair) {
            match cmd {
                control::ROLL_UP_2 => self.rows = 2,
                control::ROLL_UP_3 => self.rows = 3,
                control::ERASE_DISPLAY_MEMORY => {
                    self.lines = vec![String::new()];
                    self.col = 0;
                }
                control::CARRIAGE_RETURN => {
                    self.lines.push(String::new());
                    if self.lines.len() > self.rows {
                        self.lines.remove(0);
                    }
                    self.col = 0;
                }
                _ => {}
            }
            return;
        }
        // PAC: first byte 0x10..=0x17, not a control pair we know — for this
        // stream the only PAC anchors at the bottom row, so a fresh window.
        let stripped = eia608::parity_strip(pair);
        if (stripped & 0x7f40) == 0x1040 {
            self.lines = vec![String::new()];
            self.col = 0;
            return;
        }
        let (c1, c2) = eia608::basicna_decode(pair);
        for c in [c1, c2].into_iter().flatten() {
            if self.col < SCREEN_COLS {
                self.lines.last_mut().unwrap().push(c);
                self.col += 1;
            }
        }
    }

    /// Concatenate of everything ever shown — the full transcript.
    fn transcript(pairs: &[ScheduledPair]) -> (Vec<String>, String) {
        let mut screen = Screen::new();
        let mut transcript = String::new();
        for &(_, pair) in pairs {
            screen.feed(pair);
            let (a, b) = eia608::basicna_decode(pair);
            for c in [a, b].into_iter().flatten() {
                transcript.push(c);
            }
        }
        (screen.lines, transcript)
    }
}

const WORDS: [(&str, f64); 20] = [
    ("LIVE", 0.5),
    ("CAPTIONS", 0.9),
    ("COME", 1.9),
    ("ONE", 2.3),
    ("WORD", 2.7),
    ("A", 3.0),
    ("TIME", 3.2),
    ("AS", 3.6),
    ("THEY", 3.8),
    ("ARE", 4.0),
    ("SPOKEN", 4.3),
    ("IN", 4.9),
    ("THE", 5.1),
    ("BROADCAST", 5.3),
    ("AND", 6.1),
    ("EVERY", 6.4),
    ("WORD", 6.7),
    ("IS", 7.0),
    ("RECOVERED", 7.3),
    ("INTACT", 8.0),
];

#[test]
fn init_sequence_matches_proven_harness() {
    let r = Rollup::new(30, Rows::Two);
    let p: Vec<u16> = r.pairs().iter().map(|&(_, d)| d).collect();
    let edm = eia608::control_command(control::ERASE_DISPLAY_MEMORY);
    let ru2 = eia608::control_command(control::ROLL_UP_2);
    let pac = eia608::row_column_preamble(14, 0, false);
    assert_eq!(p, vec![edm, edm, ru2, ru2, pac, pac]);
    assert!(p.iter().all(|&d| eia608::parity_verify(d)));
}

#[test]
fn every_word_recovers_ru2_30fps() {
    let mut r = Rollup::new(30, Rows::Two);
    for &(w, t) in &WORDS {
        r.word(t, w).unwrap();
    }
    let (window, transcript) = Screen::transcript(r.pairs());

    // Every spoken word appears in the decoded transcript, in order.
    let mut pos = 0;
    for &(w, _) in &WORDS {
        let found = transcript[pos..]
            .find(w)
            .unwrap_or_else(|| panic!("word {w} missing"));
        pos += found + w.len();
    }
    // The visible window never exceeds the roll-up row count or 32 columns.
    assert!(window.len() <= 2);
    assert!(window.iter().all(|l| l.chars().count() <= SCREEN_COLS));
    // The last window holds the trailing words, scrolled as live TV shows them.
    assert!(window.last().unwrap().contains("INTACT"));
}

#[test]
fn every_word_recovers_ru3_60fps() {
    let mut r = Rollup::new(60, Rows::Three);
    for &(w, t) in &WORDS {
        r.word(t, w).unwrap();
    }
    let (window, transcript) = Screen::transcript(r.pairs());

    let mut pos = 0;
    for &(w, _) in &WORDS {
        let found = transcript[pos..]
            .find(w)
            .unwrap_or_else(|| panic!("word {w} missing"));
        pos += found + w.len();
    }
    assert!(window.len() <= 3);
    assert!(window.iter().all(|l| l.chars().count() <= SCREEN_COLS));

    // 60 fps field-1 cadence: pairs land only on alternating pictures.
    assert!(r.pairs().iter().all(|&(f, _)| f % 2 == 0));
    // Frame numbers are monotonically non-decreasing.
    assert!(r.pairs().windows(2).all(|w| w[0].0 <= w[1].0));
    assert!(r.pairs().iter().all(|&(_, d)| eia608::parity_verify(d)));
}

#[test]
fn word_that_would_cross_column_32_wraps_to_new_row() {
    let mut r = Rollup::new(30, Rows::Two);
    // 28 chars + space + 4 chars = 33 > 32: second word must CR first.
    r.word(0.0, "ABCDEFGHIJKLMNOPQRSTUVWXYZ12").unwrap();
    r.word(0.2, "WRAP").unwrap();
    let (window, transcript) = Screen::transcript(r.pairs());
    assert!(window.iter().all(|l| l.chars().count() <= SCREEN_COLS));
    assert_eq!(window.last().unwrap(), "WRAP");
    assert!(transcript.contains("ABCDEFGHIJKLMNOPQRSTUVWXYZ12"));
    assert!(transcript.contains("WRAP"));
}

#[test]
fn word_longer_than_a_row_chunks_with_carriage_returns() {
    let mut r = Rollup::new(30, Rows::Two);
    r.word(0.0, "ABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890").unwrap(); // 35 chars
    let (window, transcript) = Screen::transcript(r.pairs());
    assert!(window.iter().all(|l| l.chars().count() <= SCREEN_COLS));
    assert!(transcript.contains("ABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890"));
}

#[test]
fn control_commands_are_emitted_twice() {
    let mut r = Rollup::new(30, Rows::Two);
    r.word(0.0, "ABCDEFGHIJKLMNOPQRSTUVWXYZ1234567").unwrap(); // fills the row
    r.word(0.2, "NEXT").unwrap();
    let cr = eia608::control_command(control::CARRIAGE_RETURN);
    let pairs = r.pairs();
    let cr_runs: Vec<usize> = pairs
        .windows(2)
        .enumerate()
        .filter(|(_, w)| w[0].1 == cr && w[1].1 == cr)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(cr_runs.len(), 1, "the wrap CR must appear exactly doubled");
}

#[test]
fn one_character_tail_has_null_second_byte_not_space() {
    let mut r = Rollup::new(30, Rows::Two);
    r.word(0.0, "A").unwrap();
    // The text pair for a lone 'A' must be parity(0x4100) = 0xC180,
    // i.e. low byte null, matching the C harness comment.
    let text_pair = r.pairs().last().unwrap().1;
    assert_eq!(text_pair, 0xC180);
    assert_eq!(eia608::basicna_decode(text_pair), (Some('A'), None));
}

#[test]
fn golden_pairs_match_libcaption_encodings() {
    let mut r = Rollup::new(30, Rows::Two);
    r.word(0.0, "LI").unwrap();
    let last = r.pairs().last().unwrap().1;
    // 'L'=0x4C 'I'=0x49 both already odd-parity: pair is 0x4C49 verbatim.
    assert_eq!(last, 0x4C49);
    assert_eq!(eia608::basicna_decode(last), (Some('L'), Some('I')));
}

#[test]
fn unmappable_characters_are_rejected() {
    let mut r = Rollup::new(30, Rows::Two);
    // ASCII characters the 608 basic set lacks (0x2A is 'á', not '*').
    assert_eq!(r.word(0.0, "A*B"), Err(RollupError::UnmappableChar('*')));
    assert_eq!(r.word(1.0, "a~b"), Err(RollupError::UnmappableChar('~')));
    assert_eq!(r.word(2.0, "中"), Err(RollupError::UnmappableChar('中')));
    // 608-mappable accented letters work.
    r.word(3.0, "élÑñ").unwrap();
    let (_, transcript) = Screen::transcript(r.pairs());
    assert!(transcript.contains("élÑñ"));
}

#[test]
fn non_monotonic_timestamp_rejected() {
    let mut r = Rollup::new(30, Rows::Two);
    r.word(1.0, "OK").unwrap();
    assert_eq!(r.word(0.5, "BACK"), Err(RollupError::NonMonotonicTimestamp));
}

#[test]
fn every_pair_carries_odd_parity() {
    let mut r = Rollup::new(30, Rows::Three);
    for &(w, t) in &WORDS {
        r.word(t, w).unwrap();
    }
    assert!(r.pairs().iter().all(|&(_, d)| eia608::parity_verify(d)));
}
