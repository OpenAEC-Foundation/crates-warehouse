//! Stationery resolver — resolves PDF/PNG background paths.
//!
//! Port of `core/stationery.py`. Handles path resolution for stationery
//! (background PDF/PNG files). Actual PDF merging is delegated to the
//! layout crate (separation of data and rendering concerns).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Stationery file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StationeryFormat {
    Pdf,
    Png,
    Jpg,
    Jpeg,
}

impl StationeryFormat {
    /// Detect format from file extension.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "pdf" => Some(Self::Pdf),
            "png" => Some(Self::Png),
            "jpg" => Some(Self::Jpg),
            "jpeg" => Some(Self::Jpeg),
            _ => None,
        }
    }
}

/// Resolved stationery specification — ready for the layout engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationerySpec {
    /// Absolute path to the stationery file.
    pub path: PathBuf,
    /// File format.
    pub format: StationeryFormat,
    /// Page dimensions in mm (if known from brand config).
    pub width_mm: Option<f64>,
    pub height_mm: Option<f64>,
}

/// Resolves stationery file paths relative to a brand directory.
///
/// Caches resolved paths for repeated lookups (e.g., every page
/// in a multi-page document using the same stationery).
pub struct StationeryResolver {
    brand_dir: Option<PathBuf>,
    cache: HashMap<String, Option<PathBuf>>,
}

impl StationeryResolver {
    /// Create a new resolver with an optional brand directory.
    pub fn new(brand_dir: Option<PathBuf>) -> Self {
        Self {
            brand_dir,
            cache: HashMap::new(),
        }
    }

    /// Resolve a stationery source path to an absolute path.
    ///
    /// Resolution order:
    /// 1. If absolute and exists → use directly
    /// 2. If relative → resolve against brand_dir
    /// 3. If relative → resolve against brand_dir/stationery/
    ///
    /// Returns `None` if the file cannot be found.
    pub fn resolve(&mut self, source: &str) -> Option<PathBuf> {
        // Check cache
        if let Some(cached) = self.cache.get(source) {
            return cached.clone();
        }

        let result = self.resolve_uncached(source);
        self.cache.insert(source.to_string(), result.clone());
        result
    }

    /// Resolve without caching.
    fn resolve_uncached(&self, source: &str) -> Option<PathBuf> {
        if source.is_empty() {
            return None;
        }

        let path = Path::new(source);

        // Absolute path
        if path.is_absolute() && path.exists() {
            return Some(path.to_path_buf());
        }

        // Relative to brand_dir
        if let Some(ref brand_dir) = self.brand_dir {
            let resolved = brand_dir.join(path);
            if resolved.exists() {
                return Some(resolved);
            }

            // Also check stationery/ subdirectory
            let stationery_dir = brand_dir.join("stationery").join(path);
            if stationery_dir.exists() {
                return Some(stationery_dir);
            }
        }

        None
    }

    /// Resolve to a full `StationerySpec`.
    pub fn resolve_spec(&mut self, source: &str) -> Option<StationerySpec> {
        let path = self.resolve(source)?;
        let ext = path.extension()?.to_str()?;
        let format = StationeryFormat::from_extension(ext)?;

        Some(StationerySpec {
            path,
            format,
            width_mm: None,
            height_mm: None,
        })
    }

    /// Clear the resolution cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_from_extension() {
        assert_eq!(
            StationeryFormat::from_extension("pdf"),
            Some(StationeryFormat::Pdf)
        );
        assert_eq!(
            StationeryFormat::from_extension("PNG"),
            Some(StationeryFormat::Png)
        );
        assert_eq!(
            StationeryFormat::from_extension("jpg"),
            Some(StationeryFormat::Jpg)
        );
        assert_eq!(StationeryFormat::from_extension("svg"), None);
    }

    #[test]
    fn test_resolve_empty() {
        let mut resolver = StationeryResolver::new(None);
        assert!(resolver.resolve("").is_none());
    }

    #[test]
    fn test_resolve_nonexistent() {
        let mut resolver = StationeryResolver::new(None);
        assert!(resolver.resolve("nonexistent.pdf").is_none());
    }

    #[test]
    fn test_resolve_relative_to_brand_dir() {
        let dir = tempfile::tempdir().unwrap();
        let pdf_path = dir.path().join("bg.pdf");
        std::fs::write(&pdf_path, b"fake pdf").unwrap();

        let mut resolver = StationeryResolver::new(Some(dir.path().to_path_buf()));
        let resolved = resolver.resolve("bg.pdf");
        assert_eq!(resolved, Some(pdf_path));
    }

    #[test]
    fn test_resolve_stationery_subdir() {
        let dir = tempfile::tempdir().unwrap();
        let stat_dir = dir.path().join("stationery");
        std::fs::create_dir(&stat_dir).unwrap();
        let pdf_path = stat_dir.join("cover.pdf");
        std::fs::write(&pdf_path, b"fake pdf").unwrap();

        let mut resolver = StationeryResolver::new(Some(dir.path().to_path_buf()));
        let resolved = resolver.resolve("cover.pdf");
        assert_eq!(resolved, Some(pdf_path));
    }

    #[test]
    fn test_resolve_caching() {
        let dir = tempfile::tempdir().unwrap();
        let pdf_path = dir.path().join("cached.pdf");
        std::fs::write(&pdf_path, b"fake pdf").unwrap();

        let mut resolver = StationeryResolver::new(Some(dir.path().to_path_buf()));

        // First call: resolves
        let first = resolver.resolve("cached.pdf");
        assert!(first.is_some());

        // Delete the file
        std::fs::remove_file(&pdf_path).unwrap();

        // Second call: still returns cached result
        let second = resolver.resolve("cached.pdf");
        assert_eq!(first, second);
    }

    #[test]
    fn test_resolve_spec() {
        let dir = tempfile::tempdir().unwrap();
        let pdf_path = dir.path().join("page.pdf");
        std::fs::write(&pdf_path, b"fake pdf").unwrap();

        let mut resolver = StationeryResolver::new(Some(dir.path().to_path_buf()));
        let spec = resolver.resolve_spec("page.pdf").unwrap();
        assert_eq!(spec.format, StationeryFormat::Pdf);
        assert_eq!(spec.path, pdf_path);
    }
}
