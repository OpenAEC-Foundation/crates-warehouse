//! Shared test fixtures helper.
use std::path::PathBuf;

pub fn fixture(name: &str) -> PathBuf {
    PathBuf::from(r"C:\Users\rickd\Documents\GitHub\verification-files\GEF-BRO-XML")
        .join(name)
}

/// Read a fixture as a String. GEF files are typically ASCII or Windows-1252;
/// any non-UTF-8 bytes are replaced with U+FFFD so this works as a test helper
/// regardless of the file's exact legacy encoding.
pub fn read_fixture(name: &str) -> String {
    let bytes = std::fs::read(fixture(name))
        .unwrap_or_else(|e| panic!("missing fixture {}: {}", name, e));
    String::from_utf8_lossy(&bytes).into_owned()
}
