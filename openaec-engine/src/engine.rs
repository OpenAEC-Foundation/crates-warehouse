//! Template Engine — orchestrates PDF generation from YAML templates.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use openaec_core::brand::{BrandConfig, BrandLoader};
use openaec_core::template_config::{
    PageDefType, PageType, RepeatMode, TemplateConfigV2,
};
use openaec_core::tenant::TenantConfig;
use serde_json::Value;

use crate::data_bind::resolve_bind_value;
use crate::error::{EngineError, Result};
use crate::flow_layout::{self, apply_flow_layout, paginate_flow_zones};
use crate::font_engine::FontEngine;
use crate::pdf_backend::{MM_TO_PT, PageHandle, PdfBackend};
use crate::special_pages::{self, TocEntry};
use crate::zone_renderer;

/// Engine configuration.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub tenant_dir: Option<PathBuf>,
    pub tenants_root: Option<PathBuf>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            tenant_dir: None,
            tenants_root: None,
        }
    }
}

/// Validation result.
#[derive(Debug)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub template: String,
}

/// The V3 TemplateEngine — generates PDF from YAML page_types + JSON data.
pub struct Engine {
    config: EngineConfig,
}

impl Engine {
    pub fn new(config: EngineConfig) -> Self {
        Self { config }
    }

    /// Generate PDF bytes from template name, tenant, and JSON data.
    pub fn generate(
        &self,
        template_name: &str,
        tenant: &str,
        data: &Value,
    ) -> Result<Vec<u8>> {
        // Resolve tenant config
        let tenant_config = self.resolve_tenant(tenant)?;

        // Load brand
        let brand = self.load_brand(&tenant_config, tenant)?;

        // Load template YAML
        let template = self.load_template(&tenant_config, template_name)?;

        // Load all referenced page_types
        let page_types = self.load_page_types(&tenant_config, &template)?;

        // Stationery directory
        let stationery_dir = tenant_config
            .stationery_dir()
            .unwrap_or_else(|| PathBuf::from("stationery"));

        // Base dir for relative image paths
        let base_dir = tenant_config.tenant_dir()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        // Build PDF
        self.build(&template, &page_types, &brand, data, &stationery_dir, &base_dir)
    }

    /// Core build: template + page_types + brand + data → PDF bytes.
    pub fn build(
        &self,
        template: &TemplateConfigV2,
        page_types: &HashMap<String, PageType>,
        brand: &BrandConfig,
        data: &Value,
        stationery_dir: &Path,
        base_dir: &Path,
    ) -> Result<Vec<u8>> {
        let mut backend = PdfBackend::new();
        let mut font_engine = FontEngine::new();

        // Load fonts from tenant
        let tenant_config = TenantConfig::new(None, None);
        if let Some(fonts_dir) = tenant_config.fonts_dir() {
            let _ = font_engine.load_fonts_from_dir(&fonts_dir, &mut backend);
        }

        // Register Liberation Sans fallback from package assets
        self.load_fallback_fonts(&mut font_engine, &mut backend);

        // Set up brand font mappings
        for (logical, real) in &brand.fonts {
            font_engine.add_fallback(logical, real);
        }

        // Pre-calculate total page count for page numbering
        let _total_pages = self.calculate_page_count(template, page_types, brand, data, &font_engine);

        // Render each page
        let mut abs_page_num = 1;
        let toc_entries: Vec<TocEntry> = Vec::new();

        for page_def in &template.pages {
            let page_type = page_types.get(&page_def.page_type)
                .ok_or_else(|| EngineError::Template(
                    format!("Page type '{}' not found", page_def.page_type)
                ))?;

            let (width_mm, height_mm) = match page_def.orientation.as_str() {
                "landscape" => (297.0, 210.0),
                _ => (210.0, 297.0),
            };
            let page_height_pt = height_mm * MM_TO_PT;

            match page_def.page_type_kind {
                PageDefType::Special => {
                    let page = backend.add_page(width_mm, height_mm);

                    // Embed stationery
                    self.embed_stationery(&mut backend, page, page_type, stationery_dir);

                    // Render zones
                    self.render_page_zones(
                        &mut backend, &font_engine, brand, page, page_type,
                        data, page_height_pt, abs_page_num, base_dir,
                    );

                    // Special page rendering (cover, colofon, etc.)
                    match page_def.page_type.as_str() {
                        name if name.contains("cover") || name.contains("voorblad") => {
                            special_pages::render_cover(
                                &mut backend, &font_engine, brand, page, data, page_height_pt,
                            );
                        }
                        name if name.contains("colofon") => {
                            special_pages::render_colofon(
                                &mut backend, &font_engine, brand, page, data, page_height_pt,
                            );
                        }
                        name if name.contains("achterblad") || name.contains("backcover") => {
                            special_pages::render_backcover(
                                &mut backend, &font_engine, brand, page, page_height_pt,
                            );
                        }
                        _ => {}
                    }

                    abs_page_num += 1;
                }

                PageDefType::Toc => {
                    let page = backend.add_page(width_mm, height_mm);
                    self.embed_stationery(&mut backend, page, page_type, stationery_dir);
                    special_pages::render_toc(
                        &mut backend, &font_engine, brand, page,
                        &toc_entries, page_height_pt, width_mm,
                    );
                    abs_page_num += 1;
                }

                PageDefType::Fixed | PageDefType::Flow => {
                    if page_type.flow_layout {
                        // Apply flow layout + pagination
                        let shifted_zones = apply_flow_layout(
                            &page_type.text_zones, data, &font_engine, brand,
                            page_type.flow_footer_y_mm,
                        );

                        let pages = paginate_flow_zones(
                            &shifted_zones,
                            &page_type.image_zones,
                            &page_type.line_zones,
                            page_type.flow_footer_y_mm,
                            page_type.flow_content_start_y_mm,
                        );

                        for page_zones in &pages {
                            let page = backend.add_page(width_mm, height_mm);
                            self.embed_stationery(&mut backend, page, page_type, stationery_dir);

                            zone_renderer::draw_line_zones(
                                &mut backend, brand, page, &page_zones.line_zones, page_height_pt,
                            );
                            zone_renderer::draw_text_zones(
                                &mut backend, &font_engine, brand, page,
                                &page_zones.text_zones, data, page_height_pt, abs_page_num,
                            );
                            zone_renderer::draw_image_zones(
                                &mut backend, page, &page_zones.image_zones,
                                data, page_height_pt, base_dir,
                            );

                            if let Some(ref table) = page_type.table {
                                zone_renderer::draw_table(
                                    &mut backend, &font_engine, brand, page,
                                    table, data, page_height_pt, 0, None,
                                );
                            }

                            abs_page_num += 1;
                        }
                    } else if page_def.repeat == RepeatMode::Auto {
                        // Table pagination
                        if let Some(ref table) = page_type.table {
                            let total_rows = resolve_bind_value(data, &table.data_bind)
                                .and_then(|v| v.as_array())
                                .map(|a| a.len())
                                .unwrap_or(0);

                            let rpp = flow_layout::rows_per_page(
                                table.origin_y_mm, table.max_y_mm,
                                table.row_height_mm, table.show_header,
                            );
                            let chunks = flow_layout::paginate_table_data(total_rows, rpp);

                            for (offset, count) in chunks {
                                let page = backend.add_page(width_mm, height_mm);
                                self.embed_stationery(&mut backend, page, page_type, stationery_dir);

                                self.render_page_zones(
                                    &mut backend, &font_engine, brand, page, page_type,
                                    data, page_height_pt, abs_page_num, base_dir,
                                );

                                zone_renderer::draw_table(
                                    &mut backend, &font_engine, brand, page,
                                    table, data, page_height_pt, offset, Some(count),
                                );

                                abs_page_num += 1;
                            }
                        } else {
                            // No table, single page
                            let page = backend.add_page(width_mm, height_mm);
                            self.embed_stationery(&mut backend, page, page_type, stationery_dir);
                            self.render_page_zones(
                                &mut backend, &font_engine, brand, page, page_type,
                                data, page_height_pt, abs_page_num, base_dir,
                            );
                            abs_page_num += 1;
                        }
                    } else {
                        // Single fixed page
                        let page = backend.add_page(width_mm, height_mm);
                        self.embed_stationery(&mut backend, page, page_type, stationery_dir);

                        self.render_page_zones(
                            &mut backend, &font_engine, brand, page, page_type,
                            data, page_height_pt, abs_page_num, base_dir,
                        );

                        if let Some(ref table) = page_type.table {
                            zone_renderer::draw_table(
                                &mut backend, &font_engine, brand, page,
                                table, data, page_height_pt, 0, None,
                            );
                        }

                        abs_page_num += 1;
                    }
                }
            }
        }

        backend.finish()
    }

    /// Render all zones (text, lines, images) for a page type.
    fn render_page_zones(
        &self,
        backend: &mut PdfBackend,
        font_engine: &FontEngine,
        brand: &BrandConfig,
        page: PageHandle,
        page_type: &PageType,
        data: &Value,
        page_height_pt: f64,
        abs_page_num: usize,
        base_dir: &Path,
    ) {
        zone_renderer::draw_line_zones(backend, brand, page, &page_type.line_zones, page_height_pt);
        zone_renderer::draw_text_zones(
            backend, font_engine, brand, page,
            &page_type.text_zones, data, page_height_pt, abs_page_num,
        );
        zone_renderer::draw_image_zones(
            backend, page, &page_type.image_zones, data, page_height_pt, base_dir,
        );
    }

    /// Embed stationery PDF as page background.
    fn embed_stationery(
        &self,
        backend: &mut PdfBackend,
        page: PageHandle,
        page_type: &PageType,
        stationery_dir: &Path,
    ) {
        if let Some(ref stationery_name) = page_type.stationery {
            let path = stationery_dir.join(stationery_name);
            if path.exists() {
                if let Ok(data) = std::fs::read(&path) {
                    if stationery_name.ends_with(".pdf") {
                        let idx = backend.register_stationery(data, 0);
                        backend.embed_stationery(page, idx);
                    }
                    // PNG stationery: register as image covering full page
                    // (simplified — would need image dimensions)
                }
            }
        }
    }

    /// Pre-calculate total page count for page numbering.
    fn calculate_page_count(
        &self,
        template: &TemplateConfigV2,
        page_types: &HashMap<String, PageType>,
        brand: &BrandConfig,
        data: &Value,
        font_engine: &FontEngine,
    ) -> usize {
        let mut count = 0;
        for page_def in &template.pages {
            let page_type = match page_types.get(&page_def.page_type) {
                Some(pt) => pt,
                None => { count += 1; continue; }
            };

            if page_type.flow_layout {
                let shifted = apply_flow_layout(
                    &page_type.text_zones, data, font_engine, brand,
                    page_type.flow_footer_y_mm,
                );
                let pages = paginate_flow_zones(
                    &shifted, &page_type.image_zones, &page_type.line_zones,
                    page_type.flow_footer_y_mm, page_type.flow_content_start_y_mm,
                );
                count += pages.len();
            } else if page_def.repeat == RepeatMode::Auto {
                if let Some(ref table) = page_type.table {
                    let total_rows = resolve_bind_value(data, &table.data_bind)
                        .and_then(|v| v.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    let rpp = flow_layout::rows_per_page(
                        table.origin_y_mm, table.max_y_mm,
                        table.row_height_mm, table.show_header,
                    );
                    let chunks = flow_layout::paginate_table_data(total_rows, rpp);
                    count += chunks.len();
                } else {
                    count += 1;
                }
            } else {
                count += 1;
            }
        }
        count
    }

    // ── Loaders ──

    fn resolve_tenant(&self, _tenant: &str) -> Result<TenantConfig> {
        let tenant_dir = self.config.tenant_dir.as_deref();
        let tc = TenantConfig::new(tenant_dir, None);
        Ok(tc)
    }

    fn load_brand(&self, tenant_config: &TenantConfig, tenant: &str) -> Result<BrandConfig> {
        let loader = BrandLoader::new(tenant_config.clone());
        loader.load(Some(tenant))
            .map_err(|e| EngineError::Template(format!("Brand load: {}", e)))
    }

    fn load_template(
        &self,
        tenant_config: &TenantConfig,
        name: &str,
    ) -> Result<TemplateConfigV2> {
        let dirs = tenant_config.templates_dirs();
        for dir in &dirs {
            let path = dir.join(format!("{}.yaml", name));
            if path.exists() {
                let content = std::fs::read_to_string(&path)?;
                let template: TemplateConfigV2 = serde_yaml::from_str(&content)?;
                return Ok(template);
            }
        }
        Err(EngineError::Template(format!("Template '{}' not found", name)))
    }

    fn load_page_types(
        &self,
        tenant_config: &TenantConfig,
        template: &TemplateConfigV2,
    ) -> Result<HashMap<String, PageType>> {
        let mut map = HashMap::new();
        let page_types_dir = tenant_config.page_types_dir();

        for page_def in &template.pages {
            if map.contains_key(&page_def.page_type) {
                continue;
            }

            let mut loaded = false;
            if let Some(ref dir) = page_types_dir {
                let path = dir.join(format!("{}.yaml", page_def.page_type));
                if path.exists() {
                    let content = std::fs::read_to_string(&path)?;
                    let pt: PageType = serde_yaml::from_str(&content)?;
                    map.insert(page_def.page_type.clone(), pt);
                    loaded = true;
                }
            }

            if !loaded {
                // Create a minimal empty page type
                map.insert(page_def.page_type.clone(), PageType {
                    name: page_def.page_type.clone(),
                    stationery: None,
                    text_zones: Vec::new(),
                    image_zones: Vec::new(),
                    line_zones: Vec::new(),
                    table: None,
                    content_frame: None,
                    flow_layout: false,
                    flow_footer_y_mm: 260.0,
                    flow_content_start_y_mm: 32.0,
                });
            }
        }

        Ok(map)
    }

    fn load_fallback_fonts(&self, font_engine: &mut FontEngine, backend: &mut PdfBackend) {
        // Try to find Liberation Sans in common locations
        let candidates = [
            PathBuf::from("src/openaec_reports/assets/fonts"),
            PathBuf::from("assets/fonts"),
        ];

        for dir in &candidates {
            if dir.exists() {
                let _ = font_engine.load_fonts_from_dir(dir, backend);
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_config_default() {
        let config = EngineConfig::default();
        assert!(config.tenant_dir.is_none());
        assert!(config.tenants_root.is_none());
    }

    #[test]
    fn test_engine_new() {
        let engine = Engine::new(EngineConfig::default());
        // Engine creation should not fail
    }
}
