use std::collections::HashMap;

/// A single outline command for a glyph.
#[derive(Debug, Clone)]
pub enum OutlineCommand {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    CubicTo(f32, f32, f32, f32, f32, f32),
    Close,
}

/// Extracted glyph outline with advance width.
#[derive(Debug, Clone)]
pub struct GlyphOutline {
    pub commands: Vec<OutlineCommand>,
    pub advance_width: f32,
}

/// Parsed font data with glyph outlines and character mapping.
pub struct ParsedFont {
    pub units_per_em: u16,
    pub glyphs: HashMap<u16, GlyphOutline>,
    pub cmap: HashMap<u32, u16>,
    /// Direct byte-code → GID map populated from non-Unicode cmap subtables
    /// (Macintosh 1,0 Roman and Microsoft 3,0 Symbol). Subset TrueType fonts
    /// in PDFs frequently have only these subtables — they map the 1-byte
    /// content-stream codes directly to the subset's renumbered GIDs.
    /// Empty for ordinary Unicode-cmap'd fonts.
    pub byte_cmap: HashMap<u8, u16>,
}

/// Builder that implements ttf_parser::OutlineBuilder to collect outline commands.
struct OutlineCollector {
    commands: Vec<OutlineCommand>,
}

/// Builder for Type1 (hayro-font) outlines that scales raw glyph-space
/// coordinates by a multiplier (typically 1000 * FontMatrix.sx) so the
/// resulting outlines live in the same 1000-unit em coordinate system that
/// the renderer's `units_per_em = 1000` Type1 path expects.
struct Type1Collector {
    commands: Vec<OutlineCommand>,
    scale: f32,
    last_x: f32,
    last_y: f32,
}

impl Type1Collector {
    fn new(scale: f32) -> Self {
        Self {
            commands: Vec::new(),
            scale,
            last_x: 0.0,
            last_y: 0.0,
        }
    }
}

impl hayro_font::OutlineBuilder for Type1Collector {
    fn move_to(&mut self, x: f32, y: f32) {
        let (sx, sy) = (x * self.scale, y * self.scale);
        self.commands.push(OutlineCommand::MoveTo(sx, sy));
        self.last_x = sx;
        self.last_y = sy;
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (sx, sy) = (x * self.scale, y * self.scale);
        self.commands.push(OutlineCommand::LineTo(sx, sy));
        self.last_x = sx;
        self.last_y = sy;
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (sx1, sy1) = (x1 * self.scale, y1 * self.scale);
        let (sx, sy) = (x * self.scale, y * self.scale);
        let (p0x, p0y) = (self.last_x, self.last_y);
        let cp1x = p0x + (2.0 / 3.0) * (sx1 - p0x);
        let cp1y = p0y + (2.0 / 3.0) * (sy1 - p0y);
        let cp2x = sx + (2.0 / 3.0) * (sx1 - sx);
        let cp2y = sy + (2.0 / 3.0) * (sy1 - sy);
        self.commands
            .push(OutlineCommand::CubicTo(cp1x, cp1y, cp2x, cp2y, sx, sy));
        self.last_x = sx;
        self.last_y = sy;
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (sx, sy) = (x * self.scale, y * self.scale);
        self.commands.push(OutlineCommand::CubicTo(
            x1 * self.scale,
            y1 * self.scale,
            x2 * self.scale,
            y2 * self.scale,
            sx,
            sy,
        ));
        self.last_x = sx;
        self.last_y = sy;
    }
    fn close(&mut self) {
        self.commands.push(OutlineCommand::Close);
    }
}

/// Parse an embedded PDF Type1 font (FontFile stream) using hayro-font.
///
/// The resulting `ParsedFont` uses `units_per_em = 1000` and indexes glyphs
/// by their PDF character code (so `cmap` can be empty — `char_to_glyph_id`'s
/// "direct char code as GID" priority lookup picks them up).
///
/// `widths_by_code` provides per-character advance widths in 1/1000 em (the
/// values from the PDF font dict's `/Widths` array). Pass an empty map if
/// none are available — outlines will still render but advance defaults to
/// 500 units (~0.5 em), which is good enough to keep text from collapsing.
pub fn parse_type1(
    font_data: &[u8],
    widths_by_code: &HashMap<u8, f32>,
    encoding_name: Option<&str>,
    differences: &HashMap<u8, String>,
) -> Result<ParsedFont, String> {
    let table = hayro_font::type1::Table::parse(font_data)
        .ok_or_else(|| "hayro-font failed to parse Type1 data".to_string())?;

    // Type1 FontMatrix is typically [0.001 0 0 0.001 0 0] — outlines are in
    // glyph-space units that, multiplied by FontMatrix, give 1-em units. We
    // scale to 1000-em-units here so the renderer's existing units_per_em=1000
    // pipeline works without per-font-type branching.
    let fm = table.matrix();
    let scale = 1000.0 * fm.sx;

    let mut glyphs = HashMap::new();

    for code in 0u32..=255 {
        let code_u8 = code as u8;
        // Resolve code → glyph name, preferring an explicit /Differences entry,
        // then the font's own Encoding (Standard or custom Encoding vector).
        let name_owned: Option<String> = if let Some(n) = differences.get(&code_u8) {
            Some(n.clone())
        } else if let Some(n) = table.code_to_string(code_u8) {
            Some(n.to_string())
        } else {
            None
        };
        let _ = encoding_name; // reserved for future encoding-specific resolution

        let Some(name) = name_owned else { continue };

        let mut collector = Type1Collector::new(scale);
        if table.outline(&name, &mut collector).is_some() && !collector.commands.is_empty() {
            let advance = widths_by_code.get(&code_u8).copied().unwrap_or(500.0);
            glyphs.insert(
                code_u8 as u16,
                GlyphOutline {
                    commands: collector.commands,
                    advance_width: advance,
                },
            );
        } else if let Some(&w) = widths_by_code.get(&code_u8) {
            // No outline (e.g. .notdef or missing glyph) but we still want a
            // sensible advance so x-positions stay consistent.
            glyphs.insert(
                code_u8 as u16,
                GlyphOutline {
                    commands: Vec::new(),
                    advance_width: w,
                },
            );
        }
    }

    Ok(ParsedFont {
        units_per_em: 1000,
        glyphs,
        cmap: HashMap::new(),
        byte_cmap: HashMap::new(),
    })
}


impl OutlineCollector {
    fn new() -> Self {
        OutlineCollector {
            commands: Vec::new(),
        }
    }
}

impl ttf_parser::OutlineBuilder for OutlineCollector {
    fn move_to(&mut self, x: f32, y: f32) {
        self.commands.push(OutlineCommand::MoveTo(x, y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.commands.push(OutlineCommand::LineTo(x, y));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        // Convert quadratic bezier to cubic bezier.
        // Given quadratic control point Q and endpoints P0 (last point), P2 (x,y):
        //   CP1 = P0 + 2/3 * (Q - P0)
        //   CP2 = P2 + 2/3 * (Q - P2)
        // We don't have P0 here, so we get it from the last command.
        let (p0x, p0y) = self.last_point();
        let cp1x = p0x + (2.0 / 3.0) * (x1 - p0x);
        let cp1y = p0y + (2.0 / 3.0) * (y1 - p0y);
        let cp2x = x + (2.0 / 3.0) * (x1 - x);
        let cp2y = y + (2.0 / 3.0) * (y1 - y);
        self.commands
            .push(OutlineCommand::CubicTo(cp1x, cp1y, cp2x, cp2y, x, y));
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.commands
            .push(OutlineCommand::CubicTo(x1, y1, x2, y2, x, y));
    }

    fn close(&mut self) {
        self.commands.push(OutlineCommand::Close);
    }
}

impl OutlineCollector {
    fn last_point(&self) -> (f32, f32) {
        for cmd in self.commands.iter().rev() {
            match cmd {
                OutlineCommand::MoveTo(x, y)
                | OutlineCommand::LineTo(x, y)
                | OutlineCommand::CubicTo(_, _, _, _, x, y) => return (*x, *y),
                OutlineCommand::Close => continue,
            }
        }
        (0.0, 0.0)
    }
}

/// Parse a TrueType/OpenType font from raw bytes.
/// Extracts glyph outlines and builds a Unicode-to-glyph-ID mapping.
pub fn parse_truetype(font_data: &[u8]) -> Result<ParsedFont, String> {
    let face =
        ttf_parser::Face::parse(font_data, 0).map_err(|e| format!("Failed to parse font: {}", e))?;

    let units_per_em = face.units_per_em();
    let num_glyphs = face.number_of_glyphs();

    let mut glyphs = HashMap::new();

    // Extract outlines for all glyphs
    for gid in 0..num_glyphs {
        let glyph_id = ttf_parser::GlyphId(gid);
        let advance_width = face
            .glyph_hor_advance(glyph_id)
            .unwrap_or(0) as f32;

        let mut collector = OutlineCollector::new();
        let has_outline = face.outline_glyph(glyph_id, &mut collector).is_some();

        if has_outline || advance_width > 0.0 {
            glyphs.insert(
                gid,
                GlyphOutline {
                    commands: collector.commands,
                    advance_width,
                },
            );
        }
    }

    // Build cmap: unicode codepoint -> glyph ID
    let mut cmap = HashMap::new();
    for codepoint in 0x0020u32..=0xFFFEu32 {
        if let Some(ch) = char::from_u32(codepoint) {
            if let Some(glyph_id) = face.glyph_index(ch) {
                if glyph_id.0 != 0 {
                    cmap.insert(codepoint, glyph_id.0);
                }
            }
        }
    }

    // Build byte_cmap from non-Unicode subtables. Subset TrueType fonts
    // in PDFs (Encoding=None, FirstChar=1..N) typically expose a
    // (1,0) Macintosh-Roman or (3,0) Microsoft-Symbol cmap whose entries
    // map the raw 1-byte content-stream codes to the subset's renumbered
    // GIDs. Without this, we cannot resolve those codes to glyphs.
    let mut byte_cmap = HashMap::new();
    if let Some(cmap_table) = face.tables().cmap {
        for sub in cmap_table.subtables {
            // Skip Unicode-capable subtables — they're already covered above.
            if sub.is_unicode() { continue; }
            for code in 0u32..=255 {
                if let Some(gid) = sub.glyph_index(code) {
                    if gid.0 != 0 {
                        byte_cmap.entry(code as u8).or_insert(gid.0);
                    }
                }
            }
        }
    }

    Ok(ParsedFont {
        units_per_em,
        glyphs,
        cmap,
        byte_cmap,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ttf_parser::OutlineBuilder;

    #[test]
    fn test_outline_collector_quad_to_cubic() {
        let mut collector = OutlineCollector::new();
        collector.move_to(0.0, 0.0);
        collector.quad_to(50.0, 100.0, 100.0, 0.0);
        collector.close();

        assert_eq!(collector.commands.len(), 3);
        match &collector.commands[1] {
            OutlineCommand::CubicTo(x1, y1, x2, y2, x, y) => {
                // CP1 = (0,0) + 2/3*(50-0, 100-0) = (33.33, 66.67)
                assert!((x1 - 33.333).abs() < 0.01);
                assert!((y1 - 66.667).abs() < 0.01);
                // CP2 = (100,0) + 2/3*(50-100, 100-0) = (66.67, 66.67)
                assert!((x2 - 66.667).abs() < 0.01);
                assert!((y2 - 66.667).abs() < 0.01);
                assert_eq!(*x, 100.0);
                assert_eq!(*y, 0.0);
            }
            _ => panic!("Expected CubicTo"),
        }
    }
}
