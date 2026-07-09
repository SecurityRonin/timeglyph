//! Composite (two-word) decode: timestamps split across two integer fields, as
//! stored in registry exports, IE cookies, and malware configs. No single-value
//! tool reconstructs these. Oracle: time-decode --filetimelohi.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::Command;

#[test]
fn filetime_hilo_reconstructs_the_64bit_filetime() {
    // Low 0x69050000 + High 0x01d5c036 = FILETIME 132223104000000000 = 2020-01-01.
    let inst = timeglyph::compose::filetime_hilo(0x6905_0000, 0x01d5_c036).unwrap();
    assert_eq!(
        inst.render(&timeglyph::RenderZone::Utc).unwrap(),
        "2020-01-01T00:00:00Z"
    );
}

#[test]
fn filetime_hilo_agrees_with_time_decode() {
    let ok = Command::new("time-decode")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !ok {
        eprintln!("skipping: time-decode not on PATH");
        return;
    }
    let out = Command::new("time-decode")
        .args(["--filetimelohi", "69050000:01d5c036"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("2020-01-01 00:00:00"), "oracle: {text}");
    let inst = timeglyph::compose::filetime_hilo(0x6905_0000, 0x01d5_c036).unwrap();
    assert!(inst
        .render(&timeglyph::RenderZone::Utc)
        .unwrap()
        .starts_with("2020-01-01T00:00:00"));
}
