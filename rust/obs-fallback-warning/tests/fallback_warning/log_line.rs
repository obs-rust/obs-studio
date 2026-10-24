//! The log line.

use obs_fallback_warning::{FUTURE_ERROR, log_line};

#[test]
fn the_line_has_the_id_the_message_and_the_standard_sentence() {
    assert_eq!(
        log_line(
            "browser-sandbox",
            "Browser sources run without the sandbox."
        ),
        "[fallback] browser-sandbox: Browser sources run without the sandbox. This may become an error in a \
         future version of OBS."
    );
}

#[test]
fn the_standard_sentence_is_the_decided_one() {
    assert_eq!(
        FUTURE_ERROR,
        "This may become an error in a future version of OBS."
    );
}

#[test]
fn an_empty_message_still_gives_the_standard_sentence() {
    assert_eq!(
        log_line("browser-sandbox", ""),
        "[fallback] browser-sandbox: This may become an error in a future version of OBS."
    );
}
