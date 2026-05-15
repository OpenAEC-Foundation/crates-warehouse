//! Block renderer — converts schema ContentBlock variants into layout flowables.
//!
//! Bridge between the data layer (schema.rs) and the layout engine (openaec-layout).

use openaec_layout::{
    Color, Flowable, ImageFlowable, Mm, Paragraph, ParagraphStyle, Pt, Spacer, Table,
    TableStyleConfig,
};

use crate::brand::BrandConfig;
use crate::schema::{CheckResult, ContentBlock, Section};

/// Render a single content block into layout flowables.
///
/// Returns a `Vec` because some block types expand into multiple flowables
/// (e.g., a Check block produces a description paragraph + result table).
pub fn render_block(block: &ContentBlock, brand: &BrandConfig) -> Vec<Box<dyn Flowable>> {
    match block {
        ContentBlock::Paragraph(p) => render_paragraph(p, brand),
        ContentBlock::Calculation(c) => render_calculation(c, brand),
        ContentBlock::Check(c) => render_check(c, brand),
        ContentBlock::Table(t) => render_table(t, brand),
        ContentBlock::Image(img) => render_image(img, brand),
        ContentBlock::Map(_) => {
            // TODO: Map rendering not yet implemented (requires PDOK WMS)
            Vec::new()
        }
        ContentBlock::Spacer(s) => render_spacer(s),
        ContentBlock::PageBreak(_) => render_page_break(),
        ContentBlock::BulletList(bl) => render_bullet_list(bl, brand),
        ContentBlock::Heading2(h) => render_heading2(h, brand),
        // Placeholder rendering for tenant-specific blocks
        ContentBlock::RawFlowable(r) => render_placeholder(&format!(
            "[RawFlowable: {}]",
            r.class_name
        )),
        ContentBlock::BicTable(b) => render_placeholder(&format!(
            "[BicTable: {}]",
            b.location_name.as_deref().unwrap_or("unnamed")
        )),
        ContentBlock::CostSummary(_) => render_placeholder("[CostSummary]"),
        ContentBlock::LocationDetail(_) => render_placeholder("[LocationDetail]"),
        ContentBlock::ObjectDescription(o) => render_placeholder(&format!(
            "[ObjectDescription: {}]",
            o.object_name.as_deref().unwrap_or("unnamed")
        )),
        ContentBlock::Spreadsheet(s) => render_spreadsheet(s, brand),
    }
}

/// Render an entire section: section heading + all content blocks.
pub fn render_section(
    section: &Section,
    brand: &BrandConfig,
    section_num: usize,
) -> Vec<Box<dyn Flowable>> {
    let mut flowables: Vec<Box<dyn Flowable>> = Vec::new();

    // Section heading
    let heading_text = format!("{}  {}", section_num, section.title);
    let text_color = resolve_brand_text_color(brand);

    let heading_style = ParagraphStyle {
        font_size: Pt(14.0),
        leading: Pt(18.0),
        bold: true,
        text_color,
        space_before: Pt(12.0),
        space_after: Pt(6.0),
        ..Default::default()
    };
    flowables.push(Box::new(Paragraph::new(&heading_text, heading_style)));

    // Render all content blocks
    for block in &section.content {
        flowables.extend(render_block(block, brand));
    }

    // Add spacing after section
    flowables.push(Box::new(Spacer::from_mm(3.0)));

    flowables
}

// ── Individual block renderers ─────────────────────────────────────────

fn render_paragraph(
    p: &crate::schema::ParagraphBlock,
    brand: &BrandConfig,
) -> Vec<Box<dyn Flowable>> {
    let text_color = resolve_brand_text_color(brand);
    let style = ParagraphStyle {
        font_size: Pt(9.5),
        leading: Pt(12.0),
        text_color,
        space_after: Pt(4.0),
        ..Default::default()
    };
    vec![Box::new(Paragraph::new(&p.text, style))]
}

fn render_calculation(
    c: &crate::schema::CalculationBlock,
    brand: &BrandConfig,
) -> Vec<Box<dyn Flowable>> {
    let text_color = resolve_brand_text_color(brand);

    // Title paragraph
    let title_style = ParagraphStyle {
        font_size: Pt(10.0),
        leading: Pt(13.0),
        bold: true,
        text_color,
        space_after: Pt(2.0),
        ..Default::default()
    };
    let mut flowables: Vec<Box<dyn Flowable>> = vec![
        Box::new(Paragraph::new(&c.title, title_style)),
    ];

    // Build rows for the calculation details table
    let mut rows: Vec<Vec<String>> = Vec::new();

    if let Some(ref formula) = c.formula {
        rows.push(vec!["Formule".to_string(), formula.clone()]);
    }
    if let Some(ref subst) = c.substitution {
        rows.push(vec!["Invulling".to_string(), subst.clone()]);
    }
    if let Some(ref result) = c.result {
        let result_text = match &c.unit {
            Some(unit) => format!("{} {}", result, unit),
            None => result.clone(),
        };
        rows.push(vec!["Resultaat".to_string(), result_text]);
    }
    if let Some(ref reference) = c.reference {
        rows.push(vec!["Referentie".to_string(), reference.clone()]);
    }

    if !rows.is_empty() {
        let table_style = TableStyleConfig {
            header_background: None,
            header_text_color: text_color,
            grid_color: Color::rgb(200, 200, 200),
            grid_width: Pt(0.5),
            row_backgrounds: vec![None, Some(Color::rgb(248, 248, 248))],
            cell_padding: openaec_layout::Padding::new(Pt(2.0), Pt(4.0), Pt(2.0), Pt(4.0)),
            font_name: brand.resolve_font_name("body"),
            header_font_name: brand.resolve_font_name("heading"),
            font_size: Pt(9.0),
            header_font_size: Pt(9.0),
        };

        let table = Table::new(Vec::new(), rows)
            .with_col_widths(vec![
                Pt::from(Mm(40.0)),
                Pt::from(Mm(130.0)),
            ])
            .with_style(table_style);
        flowables.push(Box::new(table));
    }

    flowables.push(Box::new(Spacer::from_mm(3.0)));
    flowables
}

fn render_check(
    c: &crate::schema::CheckBlock,
    brand: &BrandConfig,
) -> Vec<Box<dyn Flowable>> {
    let text_color = resolve_brand_text_color(brand);
    let mut flowables: Vec<Box<dyn Flowable>> = Vec::new();

    // Description paragraph
    let desc_style = ParagraphStyle {
        font_size: Pt(9.5),
        leading: Pt(12.0),
        text_color,
        space_after: Pt(2.0),
        ..Default::default()
    };
    flowables.push(Box::new(Paragraph::new(&c.description, desc_style)));

    // Result table row
    let mut headers = vec!["Omschrijving".to_string()];
    let mut row = vec![c.description.clone()];

    if let Some(ref req) = c.required_value {
        headers.push("Eis".to_string());
        row.push(req.clone());
    }
    if let Some(ref calc) = c.calculated_value {
        headers.push("Berekend".to_string());
        row.push(calc.clone());
    }
    if let Some(uc) = c.unity_check {
        headers.push("UC".to_string());
        row.push(format!("{:.2}", uc));
    }

    // Result column
    let result = c.effective_result();
    let result_text = match result {
        Some(CheckResult::Pass) => "VOLDOET",
        Some(CheckResult::Fail) => "VOLDOET NIET",
        None => "-",
    };
    headers.push("Resultaat".to_string());
    row.push(result_text.to_string());

    // Use green header for pass, red for fail
    let header_bg = match result {
        Some(CheckResult::Pass) => Color::rgb(0, 128, 0),
        Some(CheckResult::Fail) => Color::rgb(200, 0, 0),
        None => Color::rgb(128, 128, 128),
    };

    let table_style = TableStyleConfig {
        header_background: Some(header_bg),
        header_text_color: Color::WHITE,
        grid_color: Color::rgb(200, 200, 200),
        grid_width: Pt(0.5),
        row_backgrounds: vec![None],
        cell_padding: openaec_layout::Padding::new(Pt(2.0), Pt(4.0), Pt(2.0), Pt(4.0)),
        font_name: brand.resolve_font_name("body"),
        header_font_name: brand.resolve_font_name("heading"),
        font_size: Pt(9.0),
        header_font_size: Pt(9.0),
    };

    let table = Table::new(headers, vec![row]).with_style(table_style);
    flowables.push(Box::new(table));

    flowables.push(Box::new(Spacer::from_mm(4.0)));
    flowables
}

fn render_table(
    t: &crate::schema::TableBlock,
    brand: &BrandConfig,
) -> Vec<Box<dyn Flowable>> {
    let text_color = resolve_brand_text_color(brand);
    let mut flowables: Vec<Box<dyn Flowable>> = Vec::new();

    // Optional table title
    if let Some(ref title) = t.title {
        let title_style = ParagraphStyle {
            font_size: Pt(10.0),
            leading: Pt(13.0),
            bold: true,
            text_color,
            space_after: Pt(2.0),
            ..Default::default()
        };
        flowables.push(Box::new(Paragraph::new(title, title_style)));
    }

    // Convert serde_json::Value cells to strings
    let rows: Vec<Vec<String>> = t
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(json_value_to_string)
                .collect()
        })
        .collect();

    // Resolve header background from brand primary color
    let header_bg = resolve_brand_primary_color(brand);

    let table_style = TableStyleConfig {
        header_background: Some(header_bg),
        header_text_color: Color::WHITE,
        grid_color: Color::rgb(200, 200, 200),
        grid_width: Pt(0.5),
        row_backgrounds: vec![None, Some(Color::rgb(245, 245, 245))],
        cell_padding: openaec_layout::Padding::new(Pt(3.0), Pt(4.0), Pt(3.0), Pt(4.0)),
        font_name: brand.resolve_font_name("body"),
        header_font_name: brand.resolve_font_name("heading"),
        font_size: Pt(9.0),
        header_font_size: Pt(9.0),
    };

    let mut table = Table::new(t.headers.clone(), rows).with_style(table_style);

    // Apply column widths if specified
    if let Some(ref widths) = t.column_widths {
        let widths_mm: Vec<f64> = widths.clone();
        table = table.with_col_widths_mm(widths_mm);
    }

    flowables.push(Box::new(table));
    flowables.push(Box::new(Spacer::from_mm(3.0)));
    flowables
}

fn render_spacer(s: &crate::schema::SpacerBlock) -> Vec<Box<dyn Flowable>> {
    vec![Box::new(Spacer::from_mm(s.height_mm as f32))]
}

fn render_page_break() -> Vec<Box<dyn Flowable>> {
    vec![Box::new(openaec_layout::PageBreak)]
}

fn render_bullet_list(
    bl: &crate::schema::BulletListBlock,
    brand: &BrandConfig,
) -> Vec<Box<dyn Flowable>> {
    let text_color = resolve_brand_text_color(brand);
    let style = ParagraphStyle {
        font_size: Pt(9.5),
        leading: Pt(12.0),
        text_color,
        space_after: Pt(2.0),
        left_indent: Pt(12.0),
        first_line_indent: Pt(-12.0),
        ..Default::default()
    };

    bl.items
        .iter()
        .map(|item| {
            let text = format!("\u{2022}  {}", item);
            Box::new(Paragraph::new(text, style.clone())) as Box<dyn Flowable>
        })
        .collect()
}

fn render_heading2(
    h: &crate::schema::Heading2Block,
    brand: &BrandConfig,
) -> Vec<Box<dyn Flowable>> {
    let text_color = resolve_brand_text_color(brand);
    let heading_text = match &h.number {
        Some(num) => format!("{}  {}", num, h.title),
        None => h.title.clone(),
    };

    let style = ParagraphStyle {
        font_size: Pt(11.0),
        leading: Pt(15.0),
        bold: true,
        text_color,
        space_before: Pt(8.0),
        space_after: Pt(4.0),
        ..Default::default()
    };
    vec![Box::new(Paragraph::new(heading_text, style))]
}

fn render_spreadsheet(
    block: &crate::schema::SpreadsheetBlock,
    brand: &BrandConfig,
) -> Vec<Box<dyn Flowable>> {
    let mut result: Vec<Box<dyn Flowable>> = Vec::new();

    // Title
    if let Some(ref title) = block.title {
        let style = ParagraphStyle {
            font_size: Pt(10.0),
            leading: Pt(14.0),
            bold: true,
            text_color: resolve_brand_text_color(brand),
            space_after: Pt(4.0),
            ..Default::default()
        };
        result.push(Box::new(Paragraph::new(title, style)));
    }

    // Render as table
    let rows: Vec<Vec<String>> = block
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut cells: Vec<String> = Vec::new();
            if block.show_row_numbers {
                cells.push(format!("{}", i + 1));
            }
            cells.extend(row.iter().map(json_value_to_string));
            cells
        })
        .collect();

    let headers = if block.show_row_numbers {
        let mut h = vec!["#".to_string()];
        h.extend(block.headers.clone());
        h
    } else {
        block.headers.clone()
    };

    let table = Table::new(headers, rows);
    result.push(Box::new(table));

    // Footnote
    if let Some(ref footnote) = block.footnote {
        let style = ParagraphStyle {
            font_size: Pt(8.0),
            leading: Pt(10.0),
            text_color: Color::GREY,
            italic: true,
            space_before: Pt(2.0),
            ..Default::default()
        };
        result.push(Box::new(Paragraph::new(footnote, style)));
    }

    result.push(Box::new(Spacer::from_mm(4.0)));
    result
}

fn render_placeholder(label: &str) -> Vec<Box<dyn Flowable>> {
    let style = ParagraphStyle {
        font_size: Pt(9.0),
        leading: Pt(12.0),
        text_color: Color::GREY,
        italic: true,
        space_after: Pt(4.0),
        ..Default::default()
    };
    vec![Box::new(Paragraph::new(label, style))]
}

fn render_image(
    img: &crate::schema::ImageBlock,
    _brand: &BrandConfig,
) -> Vec<Box<dyn Flowable>> {
    use crate::schema::ImageSource;

    // Resolve image bytes from source
    let result = match &img.src {
        ImageSource::Path(path_str) => {
            // Try as file path first
            let path = std::path::Path::new(path_str);
            if path.is_file() {
                std::fs::read(path).ok()
            } else {
                // Could be a URL — skip for now (would need HTTP client)
                tracing::warn!(path = %path_str, "Image file not found, skipping");
                None
            }
        }
        ImageSource::Base64 { data, .. } => {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD
                .decode(data)
                .ok()
        }
    };

    let Some(bytes) = result else {
        return Vec::new();
    };

    // Determine width
    let width = if let Some(w_mm) = img.width_mm {
        Pt::from(Mm(w_mm as f32))
    } else {
        // Default: full frame width (will be constrained by wrap())
        Pt::from(Mm(160.0))
    };

    let Ok(mut flowable) = ImageFlowable::from_bytes(bytes, width) else {
        return Vec::new();
    };

    // Alignment
    let alignment = match img.alignment {
        crate::schema::Alignment::Left => openaec_layout::Alignment::Left,
        crate::schema::Alignment::Center => openaec_layout::Alignment::Center,
        crate::schema::Alignment::Right => openaec_layout::Alignment::Right,
    };
    flowable = flowable.with_alignment(alignment);

    // Caption
    if let Some(ref caption) = img.caption {
        flowable = flowable.with_caption(caption);
    }

    let mut result: Vec<Box<dyn Flowable>> = vec![Box::new(flowable)];
    result.push(Box::new(Spacer::from_mm(3.0)));
    result
}

// ── Brand color helpers ────────────────────────────────────────────────

/// Resolve the text color from brand config, falling back to near-black.
fn resolve_brand_text_color(brand: &BrandConfig) -> Color {
    brand
        .resolve_color("$text")
        .and_then(Color::from_hex)
        .unwrap_or(Color::rgb(69, 36, 61)) // #45243D — 3BM default
}

/// Resolve the primary brand color, falling back to 3BM purple.
fn resolve_brand_primary_color(brand: &BrandConfig) -> Color {
    brand
        .resolve_color("$primary")
        .and_then(Color::from_hex)
        .unwrap_or(Color::rgb(64, 18, 74)) // #40124A — 3BM default
}

/// Convert a serde_json::Value to a display string.
fn json_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => if *b { "Ja" } else { "Nee" }.to_string(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brand::BrandIdentity;
    use crate::schema::*;

    fn empty_brand() -> BrandConfig {
        BrandConfig {
            brand: BrandIdentity::default(),
            colors: std::collections::HashMap::new(),
            fonts: std::collections::HashMap::new(),
            logos: std::collections::HashMap::new(),
            contact: std::collections::HashMap::new(),
            header: crate::brand::ZoneConfig::default(),
            footer: crate::brand::ZoneConfig::default(),
            styles: std::collections::HashMap::new(),
            pages: std::collections::HashMap::new(),
            stationery: std::collections::HashMap::new(),
            modules: std::collections::HashMap::new(),
            module_config: std::collections::HashMap::new(),
            tenant_modules: serde_yaml::Value::Null,
            font_files: std::collections::HashMap::new(),
            brand_dir: None,
        }
    }

    #[test]
    fn test_render_paragraph() {
        let block = ContentBlock::Paragraph(ParagraphBlock {
            text: "Hello world".to_string(),
            style: "Normal".to_string(),
        });
        let result = render_block(&block, &empty_brand());
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_render_spacer() {
        let block = ContentBlock::Spacer(SpacerBlock { height_mm: 10.0 });
        let result = render_block(&block, &empty_brand());
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_render_page_break() {
        let block = ContentBlock::PageBreak(PageBreakBlock {});
        let result = render_block(&block, &empty_brand());
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_render_bullet_list() {
        let block = ContentBlock::BulletList(BulletListBlock {
            items: vec!["Item 1".to_string(), "Item 2".to_string()],
        });
        let result = render_block(&block, &empty_brand());
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_render_heading2() {
        let block = ContentBlock::Heading2(Heading2Block {
            number: Some("1.1".to_string()),
            title: "Subheading".to_string(),
        });
        let result = render_block(&block, &empty_brand());
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_render_check_pass() {
        let block = ContentBlock::Check(CheckBlock {
            description: "Test check".to_string(),
            required_value: Some("100 mm".to_string()),
            calculated_value: Some("80 mm".to_string()),
            unity_check: Some(0.8),
            limit: 1.0,
            result: None,
            reference: None,
        });
        let result = render_block(&block, &empty_brand());
        // Description paragraph + table + spacer
        assert!(result.len() >= 2);
    }

    #[test]
    fn test_render_section() {
        let section = Section {
            title: "Test Section".to_string(),
            level: 1,
            content: vec![
                ContentBlock::Paragraph(ParagraphBlock {
                    text: "Some text".to_string(),
                    style: "Normal".to_string(),
                }),
                ContentBlock::Spacer(SpacerBlock { height_mm: 5.0 }),
            ],
            orientation: None,
            page_break_before: false,
        };
        let result = render_section(&section, &empty_brand(), 1);
        // Heading + paragraph + spacer + trailing spacer
        assert!(result.len() >= 3);
    }

    #[test]
    fn test_render_image_stub() {
        let block = ContentBlock::Image(ImageBlock {
            src: crate::schema::ImageSource::Path("test.png".to_string()),
            caption: None,
            width_mm: None,
            alignment: crate::schema::Alignment::Center,
        });
        let result = render_block(&block, &empty_brand());
        assert!(result.is_empty()); // Stubbed out
    }

    #[test]
    fn test_json_value_to_string() {
        assert_eq!(
            json_value_to_string(&serde_json::Value::String("hello".into())),
            "hello"
        );
        assert_eq!(
            json_value_to_string(&serde_json::json!(42)),
            "42"
        );
        assert_eq!(
            json_value_to_string(&serde_json::Value::Bool(true)),
            "Ja"
        );
        assert_eq!(
            json_value_to_string(&serde_json::Value::Null),
            ""
        );
    }
}
