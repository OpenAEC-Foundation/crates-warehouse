//! Multi-tenant asset path management.
//!
//! Centralizes all asset paths (templates, brands, stationery, logos, fonts)
//! with a fallback chain: tenant directory -> package defaults.

use std::env;
use std::path::{Path, PathBuf};

/// Environment variable for the active tenant directory.
const ENV_TENANT_DIR: &str = "OPENAEC_TENANT_DIR";

/// Environment variable for the tenants root (parent of all tenant dirs).
const ENV_TENANTS_ROOT: &str = "OPENAEC_TENANTS_ROOT";

/// Multi-tenant configuration with fallback to package defaults.
///
/// Asset lookup order:
/// 1. Tenant directory (if set)
/// 2. Package defaults (built-in)
#[derive(Debug, Clone)]
pub struct TenantConfig {
    /// Primary tenant directory (e.g. `tenants/3bm_cooperatie/`).
    tenant_dir: Option<PathBuf>,

    /// Root directory containing all tenant directories.
    tenants_root: Option<PathBuf>,

    /// Package assets directory (built-in defaults).
    package_assets_dir: PathBuf,
}

impl TenantConfig {
    /// Create a new TenantConfig.
    ///
    /// If `tenant_dir` is `None`, reads from `OPENAEC_TENANT_DIR` env var.
    /// Package assets are resolved relative to the crate manifest dir (for dev)
    /// or a configurable path.
    pub fn new(tenant_dir: Option<&Path>, package_assets_dir: Option<&Path>) -> Self {
        let tenant_dir = tenant_dir
            .map(PathBuf::from)
            .or_else(|| env::var(ENV_TENANT_DIR).ok().map(PathBuf::from))
            .filter(|p| p.is_dir());

        let tenants_root = detect_tenants_root(&tenant_dir);

        let package_assets_dir = package_assets_dir
            .map(PathBuf::from)
            .unwrap_or_else(default_package_assets_dir);

        Self {
            tenant_dir,
            tenants_root,
            package_assets_dir,
        }
    }

    /// The active tenant directory, if any.
    pub fn tenant_dir(&self) -> Option<&Path> {
        self.tenant_dir.as_deref()
    }

    /// The tenants root directory (parent of all tenant dirs).
    pub fn tenants_root(&self) -> Option<&Path> {
        self.tenants_root.as_deref()
    }

    /// Template directories, tenant-first then package defaults.
    ///
    /// Returns a list of directories to search for YAML templates.
    /// Templates in earlier directories take precedence.
    pub fn templates_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        if let Some(ref td) = self.tenant_dir {
            let tenant_templates = td.join("templates");
            if tenant_templates.is_dir() {
                dirs.push(tenant_templates);
            }
        }

        let pkg_templates = self.package_assets_dir.join("templates");
        if pkg_templates.is_dir() {
            dirs.push(pkg_templates);
        }

        dirs
    }

    /// Path to the brand YAML file.
    ///
    /// Checks tenant `brand.yaml` first, falls back to package default.
    pub fn brand_path(&self) -> Option<PathBuf> {
        if let Some(ref td) = self.tenant_dir {
            let tenant_brand = td.join("brand.yaml");
            if tenant_brand.is_file() {
                return Some(tenant_brand);
            }
        }

        let pkg_brand = self.package_assets_dir.join("brands").join("default.yaml");
        if pkg_brand.is_file() {
            return Some(pkg_brand);
        }

        None
    }

    /// Stationery directory (PDFs/PNGs for page backgrounds).
    pub fn stationery_dir(&self) -> Option<PathBuf> {
        self.resolve_dir("stationery")
    }

    /// Logos directory.
    pub fn logos_dir(&self) -> Option<PathBuf> {
        self.resolve_dir("logos")
    }

    /// Fonts directory.
    pub fn fonts_dir(&self) -> Option<PathBuf> {
        self.resolve_dir("fonts")
    }

    /// Page types directory (tenant-only, YAML page definitions).
    pub fn page_types_dir(&self) -> Option<PathBuf> {
        self.tenant_only_dir("page_types")
    }

    /// Modules directory (tenant-only, YAML module definitions).
    pub fn modules_dir(&self) -> Option<PathBuf> {
        self.tenant_only_dir("modules")
    }

    /// Resolve a subdirectory with fallback: tenant -> package.
    fn resolve_dir(&self, name: &str) -> Option<PathBuf> {
        if let Some(ref td) = self.tenant_dir {
            let tenant_path = td.join(name);
            if tenant_path.is_dir() {
                return Some(tenant_path);
            }
        }

        let pkg_path = self.package_assets_dir.join(name);
        if pkg_path.is_dir() {
            return Some(pkg_path);
        }

        None
    }

    /// Resolve a tenant-only subdirectory (no package fallback).
    fn tenant_only_dir(&self, name: &str) -> Option<PathBuf> {
        self.tenant_dir.as_ref().and_then(|td| {
            let path = td.join(name);
            path.is_dir().then_some(path)
        })
    }
}

impl Default for TenantConfig {
    fn default() -> Self {
        Self::new(None, None)
    }
}

/// Detect the tenants root directory.
///
/// Search order:
/// 1. `OPENAEC_TENANTS_ROOT` environment variable
/// 2. Parent of the tenant directory (if set)
fn detect_tenants_root(tenant_dir: &Option<PathBuf>) -> Option<PathBuf> {
    // Check env var first.
    if let Ok(root) = env::var(ENV_TENANTS_ROOT) {
        let path = PathBuf::from(root);
        if path.is_dir() {
            return Some(path);
        }
    }

    // Derive from tenant dir (parent).
    tenant_dir
        .as_ref()
        .and_then(|td| td.parent().map(PathBuf::from))
        .filter(|p| p.is_dir())
}

/// Default package assets directory.
///
/// In development, this is `<repo>/src/openaec_reports/assets/`.
/// In a packaged context, should be configured explicitly.
fn default_package_assets_dir() -> PathBuf {
    // For now, use a relative path. In production this will be configurable.
    PathBuf::from("assets")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        // Without any env vars or tenant dir, should not panic.
        let config = TenantConfig::new(None, None);
        assert!(config.tenant_dir().is_none());
    }

    #[test]
    fn test_with_tenant_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let tenant = tmp.path().join("my_tenant");
        std::fs::create_dir_all(tenant.join("templates")).unwrap();
        std::fs::create_dir_all(tenant.join("fonts")).unwrap();
        std::fs::create_dir_all(tenant.join("stationery")).unwrap();

        let config = TenantConfig::new(Some(&tenant), None);
        assert_eq!(config.tenant_dir(), Some(tenant.as_path()));
        assert_eq!(config.templates_dirs().len(), 1);
        assert!(config.fonts_dir().is_some());
        assert!(config.stationery_dir().is_some());
    }

    #[test]
    fn test_brand_path_from_tenant() {
        let tmp = tempfile::tempdir().unwrap();
        let tenant = tmp.path().join("my_tenant");
        std::fs::create_dir_all(&tenant).unwrap();
        std::fs::write(tenant.join("brand.yaml"), "name: test").unwrap();

        let config = TenantConfig::new(Some(&tenant), None);
        let brand = config.brand_path().unwrap();
        assert!(brand.ends_with("brand.yaml"));
    }

    #[test]
    fn test_tenants_root_from_parent() {
        let tmp = tempfile::tempdir().unwrap();
        let tenants_root = tmp.path().join("tenants");
        let tenant = tenants_root.join("my_tenant");
        std::fs::create_dir_all(&tenant).unwrap();

        let config = TenantConfig::new(Some(&tenant), None);
        assert_eq!(config.tenants_root(), Some(tenants_root.as_path()));
    }

    #[test]
    fn test_tenant_only_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let tenant = tmp.path().join("tenant");
        std::fs::create_dir_all(tenant.join("modules")).unwrap();

        let config = TenantConfig::new(Some(&tenant), None);
        assert!(config.modules_dir().is_some());
        // page_types dir does not exist
        assert!(config.page_types_dir().is_none());
    }
}
