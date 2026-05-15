//! Table of contents — TOC entry collection and styling.
//!
//! Port of `core/toc.py`. Provides data structures for collecting
//! heading entries during rendering and configuring TOC level styles.
//! The actual TOC page rendering is handled by `special_pages.rs`.

use openaec_layout::Pt;
use serde::{Deserialize, Serialize};

/// Heading style names that the TOC should detect.
pub const TOC_HEADING_STYLES: [&str; 3] = ["Heading1", "Heading2", "Heading3"];

/// Maximum supported TOC depth.
pub const MAX_TOC_DEPTH: usize = 3;

/// Style configuration for a single TOC level.
#[derive(Debug, Clone)]
pub struct TocLevelStyle {
    /// Font name for this level.
    pub font_name: String,
    /// Font size in points.
    pub font_size: Pt,
    /// Line leading in points.
    pub leading: Pt,
    /// Left indent in points.
    pub left_indent: Pt,
    /// Space before entry in points.
    pub space_before: Pt,
    /// Whether the text is bold.
    pub bold: bool,
}

/// Default TOC level styles (3 levels).
pub fn default_toc_styles() -> Vec<TocLevelStyle> {
    vec![
        // Level 1: bold, larger, no indent
        TocLevelStyle {
            font_name: "GothamBold".to_string(),
            font_size: Pt(10.5),
            leading: Pt(14.0),
            left_indent: Pt(0.0),
            space_before: Pt(4.0),
            bold: true,
        },
        // Level 2: regular, body size, 15pt indent
        TocLevelStyle {
            font_name: "GothamBook".to_string(),
            font_size: Pt(9.5),
            leading: Pt(12.0),
            left_indent: Pt(15.0),
            space_before: Pt(2.0),
            bold: false,
        },
        // Level 3: regular, slightly smaller, 30pt indent
        TocLevelStyle {
            font_name: "GothamBook".to_string(),
            font_size: Pt(9.0),
            leading: Pt(11.0),
            left_indent: Pt(30.0),
            space_before: Pt(1.0),
            bold: false,
        },
    ]
}

/// A single entry in the table of contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TocEntry {
    /// Section title.
    pub title: String,
    /// Heading level (0 = H1, 1 = H2, 2 = H3).
    pub level: usize,
    /// Page number (filled in during/after rendering).
    pub page_number: Option<u32>,
    /// Bookmark key for linking.
    pub key: String,
}

/// Collects TOC entries during report rendering.
///
/// In a two-pass rendering system:
/// 1. First pass: build report, collect heading positions via `add_entry()`
/// 2. Second pass: render TOC page with page numbers filled in
pub struct TocBuilder {
    entries: Vec<TocEntry>,
    entry_count: usize,
}

impl TocBuilder {
    /// Create a new empty TOC builder.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            entry_count: 0,
        }
    }

    /// Register a heading for the TOC.
    ///
    /// # Arguments
    /// * `title` — Section title text.
    /// * `level` — Heading level (0 = H1, 1 = H2, 2 = H3).
    pub fn add_entry(&mut self, title: &str, level: usize) -> String {
        let key = format!("toc-{}", self.entry_count);
        self.entry_count += 1;

        self.entries.push(TocEntry {
            title: title.to_string(),
            level,
            page_number: None,
            key: key.clone(),
        });

        key
    }

    /// Set the page number for an entry by key.
    pub fn set_page_number(&mut self, key: &str, page: u32) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.key == key) {
            entry.page_number = Some(page);
        }
    }

    /// Get all collected entries.
    pub fn entries(&self) -> &[TocEntry] {
        &self.entries
    }

    /// Get entries filtered by max depth.
    pub fn entries_filtered(&self, max_depth: usize) -> Vec<&TocEntry> {
        self.entries
            .iter()
            .filter(|e| e.level < max_depth)
            .collect()
    }

    /// Total number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the TOC is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.entry_count = 0;
    }
}

impl Default for TocBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_toc_styles() {
        let styles = default_toc_styles();
        assert_eq!(styles.len(), 3);
        assert!(styles[0].bold);
        assert!(!styles[1].bold);
        assert_eq!(styles[0].left_indent, Pt(0.0));
        assert_eq!(styles[1].left_indent, Pt(15.0));
        assert_eq!(styles[2].left_indent, Pt(30.0));
    }

    #[test]
    fn test_toc_builder_add_entries() {
        let mut toc = TocBuilder::new();
        let key1 = toc.add_entry("Uitgangspunten", 0);
        let key2 = toc.add_entry("Normen", 1);
        let key3 = toc.add_entry("Materialen", 1);

        assert_eq!(toc.len(), 3);
        assert_eq!(key1, "toc-0");
        assert_eq!(key2, "toc-1");
        assert_eq!(key3, "toc-2");
    }

    #[test]
    fn test_toc_builder_page_numbers() {
        let mut toc = TocBuilder::new();
        let key = toc.add_entry("Test", 0);
        assert!(toc.entries()[0].page_number.is_none());

        toc.set_page_number(&key, 3);
        assert_eq!(toc.entries()[0].page_number, Some(3));
    }

    #[test]
    fn test_toc_builder_filtered() {
        let mut toc = TocBuilder::new();
        toc.add_entry("H1", 0);
        toc.add_entry("H2", 1);
        toc.add_entry("H3", 2);

        let filtered = toc.entries_filtered(2);
        assert_eq!(filtered.len(), 2); // Only H1 and H2
    }

    #[test]
    fn test_toc_builder_clear() {
        let mut toc = TocBuilder::new();
        toc.add_entry("Test", 0);
        assert!(!toc.is_empty());

        toc.clear();
        assert!(toc.is_empty());
    }

    #[test]
    fn test_toc_heading_styles_constant() {
        assert_eq!(TOC_HEADING_STYLES, ["Heading1", "Heading2", "Heading3"]);
    }
}
