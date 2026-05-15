//! Font discovery and management.
//!
//! Loads TTF/OTF fonts from tenant directories with graceful fallback
//! to system-standard fonts (Helvetica equivalents).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tracing::{info, warn};

/// Gotham font candidates — multiple filename variants per logical font.
const FONT_CANDIDATES: &[(&str, &[&str])] = &[
    (
        "GothamBold",
        &[
            "Gotham-Bold.ttf",
            "Gotham-Bold.otf",
            "GothamBold.ttf",
            "GothamBold.otf",
        ],
    ),
    (
        "GothamBook",
        &[
            "Gotham-Book.ttf",
            "Gotham-Book.otf",
            "GothamBook.ttf",
            "GothamBook.otf",
        ],
    ),
    (
        "GothamMedium",
        &[
            "Gotham-Medium.ttf",
            "Gotham-Medium.otf",
            "GothamMedium.ttf",
            "GothamMedium.otf",
        ],
    ),
    (
        "GothamBookItalic",
        &[
            "Gotham-BookItalic.ttf",
            "Gotham-BookItalic.otf",
            "GothamBookItalic.ttf",
            "GothamBookItalic.otf",
        ],
    ),
];

/// Fallback font mapping (Gotham -> standard equivalents for Typst).
const FALLBACK_FONTS: &[(&str, &str)] = &[
    ("GothamBold", "Helvetica"),
    ("GothamBook", "Helvetica"),
    ("GothamMedium", "Helvetica"),
    ("GothamBookItalic", "Helvetica"),
];

/// Font registration status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontStatus {
    /// Font file found and loaded.
    Registered(PathBuf),
    /// Font not found, using fallback.
    Fallback(String),
}

/// Manages font discovery and loading for report generation.
///
/// Discovers fonts from tenant directories, tracks registration state,
/// and provides fallback mappings for missing fonts.
#[derive(Debug)]
pub struct FontManager {
    /// Registered font name -> status.
    fonts: HashMap<String, FontStatus>,

    /// Font database for font metrics and discovery.
    db: fontdb::Database,
}

/// Global singleton for checking if Gotham fonts are available.
static GOTHAM_AVAILABLE: OnceLock<bool> = OnceLock::new();

impl FontManager {
    /// Create a new FontManager and discover fonts from the given directory.
    ///
    /// If `fonts_dir` is `None`, only fallback fonts are available.
    pub fn new(fonts_dir: Option<&Path>) -> Self {
        let mut manager = Self {
            fonts: HashMap::new(),
            db: fontdb::Database::new(),
        };

        // Load system fonts for fallback.
        manager.db.load_system_fonts();

        // Discover Gotham fonts from tenant dir.
        if let Some(dir) = fonts_dir {
            manager.discover_gotham_fonts(dir);
        } else {
            manager.register_all_fallbacks();
        }

        manager
    }

    /// Discover and register Gotham fonts from a directory.
    fn discover_gotham_fonts(&mut self, dir: &Path) {
        let mut any_found = false;

        for &(font_name, candidates) in FONT_CANDIDATES {
            let found = candidates.iter().find_map(|filename| {
                let path = dir.join(filename);
                if path.is_file() {
                    Some(path)
                } else {
                    None
                }
            });

            if let Some(path) = found {
                info!(font = font_name, path = %path.display(), "Font discovered");
                self.db.load_font_file(&path).ok();
                self.fonts
                    .insert(font_name.to_string(), FontStatus::Registered(path));
                any_found = true;
            } else {
                let fallback = get_fallback(font_name);
                warn!(
                    font = font_name,
                    fallback = fallback,
                    "Font not found, using fallback"
                );
                self.fonts.insert(
                    font_name.to_string(),
                    FontStatus::Fallback(fallback.to_string()),
                );
            }
        }

        GOTHAM_AVAILABLE.get_or_init(|| any_found);
    }

    /// Register all fonts as fallbacks (no tenant fonts available).
    fn register_all_fallbacks(&mut self) {
        for &(font_name, _) in FONT_CANDIDATES {
            let fallback = get_fallback(font_name);
            self.fonts.insert(
                font_name.to_string(),
                FontStatus::Fallback(fallback.to_string()),
            );
        }

        GOTHAM_AVAILABLE.get_or_init(|| false);
    }

    /// Register tenant-specific custom fonts (arbitrary name -> filename mappings).
    pub fn register_tenant_fonts(&mut self, font_files: &HashMap<String, String>, dir: &Path) {
        for (name, filename) in font_files {
            let path = dir.join(filename);
            if path.is_file() {
                info!(font = name.as_str(), path = %path.display(), "Tenant font registered");
                self.db.load_font_file(&path).ok();
                self.fonts
                    .insert(name.clone(), FontStatus::Registered(path));
            } else {
                warn!(
                    font = name.as_str(),
                    filename = filename.as_str(),
                    "Tenant font file not found"
                );
            }
        }
    }

    /// Get the effective font name for rendering.
    ///
    /// If the font is registered (custom/Gotham), returns the registered name.
    /// If it's a fallback, returns the fallback name.
    /// Otherwise returns the literal name (for built-in fonts).
    pub fn get_font_name<'a>(&'a self, font_name: &'a str) -> &'a str {
        match self.fonts.get(font_name) {
            Some(FontStatus::Registered(_)) => font_name,
            Some(FontStatus::Fallback(fallback)) => fallback.as_str(),
            None => font_name,
        }
    }

    /// Check if Gotham fonts are available.
    pub fn gotham_available(&self) -> bool {
        self.fonts
            .iter()
            .any(|(_, status)| matches!(status, FontStatus::Registered(_)))
    }

    /// Get the font database reference (for Typst integration).
    pub fn database(&self) -> &fontdb::Database {
        &self.db
    }

    /// Get status of all tracked fonts.
    pub fn status(&self) -> &HashMap<String, FontStatus> {
        &self.fonts
    }

    /// Get the file path for a registered font, if available.
    pub fn font_path(&self, font_name: &str) -> Option<&Path> {
        match self.fonts.get(font_name) {
            Some(FontStatus::Registered(path)) => Some(path.as_path()),
            _ => None,
        }
    }
}

/// Get the fallback font for a Gotham font name.
fn get_fallback(font_name: &str) -> &'static str {
    FALLBACK_FONTS
        .iter()
        .find(|&&(name, _)| name == font_name)
        .map(|&(_, fallback)| fallback)
        .unwrap_or("Helvetica")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_manager_no_fonts() {
        let manager = FontManager::new(None);
        assert!(!manager.gotham_available());
        assert_eq!(manager.get_font_name("GothamBold"), "Helvetica");
        assert_eq!(manager.get_font_name("GothamBook"), "Helvetica");
    }

    #[test]
    fn test_font_manager_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let manager = FontManager::new(Some(tmp.path()));
        assert!(!manager.gotham_available());
        assert_eq!(manager.get_font_name("GothamBold"), "Helvetica");
    }

    #[test]
    fn test_font_manager_with_fonts() {
        let tmp = tempfile::tempdir().unwrap();
        // Create a minimal valid TTF-like file (won't actually render,
        // but tests the discovery logic).
        std::fs::write(tmp.path().join("Gotham-Bold.ttf"), b"fake-ttf").unwrap();

        let manager = FontManager::new(Some(tmp.path()));

        // GothamBold should be "registered" (file found).
        match manager.fonts.get("GothamBold") {
            Some(FontStatus::Registered(path)) => {
                assert!(path.ends_with("Gotham-Bold.ttf"));
            }
            other => panic!("Expected Registered, got {other:?}"),
        }

        // GothamBook should be fallback (not found).
        assert_eq!(manager.get_font_name("GothamBook"), "Helvetica");
    }

    #[test]
    fn test_font_path() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("Gotham-Bold.ttf"), b"fake-ttf").unwrap();

        let manager = FontManager::new(Some(tmp.path()));
        assert!(manager.font_path("GothamBold").is_some());
        assert!(manager.font_path("GothamBook").is_none());
        assert!(manager.font_path("NonExistent").is_none());
    }

    #[test]
    fn test_literal_font_passthrough() {
        let manager = FontManager::new(None);
        // Unknown font names should be returned as-is.
        assert_eq!(manager.get_font_name("Helvetica-Bold"), "Helvetica-Bold");
        assert_eq!(manager.get_font_name("Arial"), "Arial");
    }

    #[test]
    fn test_tenant_fonts() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("CustomFont.ttf"), b"fake-ttf").unwrap();

        let mut manager = FontManager::new(None);
        let font_files: HashMap<String, String> =
            [("MyCustom".to_string(), "CustomFont.ttf".to_string())]
                .into_iter()
                .collect();

        manager.register_tenant_fonts(&font_files, tmp.path());

        match manager.fonts.get("MyCustom") {
            Some(FontStatus::Registered(path)) => {
                assert!(path.ends_with("CustomFont.ttf"));
            }
            other => panic!("Expected Registered, got {other:?}"),
        }
    }

    #[test]
    fn test_status_report() {
        let manager = FontManager::new(None);
        let status = manager.status();
        assert!(status.contains_key("GothamBold"));
        assert!(status.contains_key("GothamBook"));
        assert!(status.contains_key("GothamMedium"));
        assert!(status.contains_key("GothamBookItalic"));
    }
}
