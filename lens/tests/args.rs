//! CLI argument handling for the `timeglyph-lens` binary. The one-shot text mode
//! was dropped (it duplicated `timeglyph scan`); a positional text argument must
//! now fail loudly with a hint rather than silently launch the GUI ignoring it.
#![allow(clippy::unwrap_used)]

use std::process::Command;

fn run(args: &[&str]) -> (String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_timeglyph-lens"))
        .args(args)
        .output()
        .unwrap();
    let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    (combined, out.status.code().unwrap_or(-1))
}

#[test]
fn positional_text_argument_fails_loudly_with_a_hint() {
    // The one-shot text mode is gone: a positional value must not be silently
    // decoded nor launch the GUI — it exits non-zero pointing at `timeglyph scan`.
    let (out, code) = run(&["1577836800"]);
    assert_ne!(code, 0, "positional text arg should exit non-zero: {out}");
    assert!(
        out.contains("timeglyph scan"),
        "hint should point at `timeglyph scan`: {out}"
    );
    // It must NOT have actually decoded the value (that was the removed behavior).
    assert!(
        !out.contains("2020-01-01"),
        "must not decode the value itself: {out}"
    );
}
