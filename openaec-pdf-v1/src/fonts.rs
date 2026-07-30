use std::collections::HashMap;
use std::sync::Arc;
use lopdf::{Dictionary, Document, Object, ObjectId};
use crate::encoding;
use crate::font_parser::{self, ParsedFont};

/// Cached font entry with parsed outlines and encoding info.
pub struct FontEntry {
    pub parsed: Option<ParsedFont>,
    pub encoding_name: Option<String>,
    pub differences: HashMap<u8, String>,
    pub base_font: String,
    /// True for Type0/CID fonts — character codes are 2-byte, not 1-byte
    pub is_cid: bool,
    /// True when CIDToGIDMap is Identity (CID values = glyph IDs directly)
    pub cid_to_gid_identity: bool,
    /// ToUnicode mapping: char_code → Unicode codepoint (from PDF ToUnicode CMap)
    pub to_unicode: HashMap<u8, char>,
    /// ToUnicode mapping for CID fonts: 2-byte code → Unicode codepoint
    pub cid_to_unicode: HashMap<u16, char>,
    /// Per-character advance widths in 1/1000-em units, indexed by char code
    /// (u8 for simple fonts) or CID (u16 for Type0). Always read from the PDF
    /// font dictionary's `/Widths` (simple) or `/W` (Type0) array per the
    /// PDF spec — these are authoritative and may differ from the embedded
    /// font's internal advance widths (esp. for subsetted fonts).
    pub widths: HashMap<u32, f32>,
    /// Default advance width in 1/1000-em units, applied when a code is not
    /// in `widths`. Read from `/MissingWidth` (simple fonts, default 0) or
    /// `/DW` (Type0 fonts, default 1000).
    pub default_width: f32,
    /// Whether the FontDescriptor `/Flags` integer has bit 19 (ForceBold,
    /// value 0x40000 = 262144) set. Per ISO 32000-1 §9.8.1 Table 122 this
    /// indicates that the glyphs should be rendered as if synthetically
    /// emboldened — the renderer typically fills the path AND strokes it
    /// with a small line width on top so the outline appears bolder than
    /// the embedded glyph would normally render.
    ///
    /// Note: in many real-world PDFs bold variants are handled by embedding
    /// a separate bold font (e.g. "Arial-BoldMT") rather than setting this
    /// flag, so this case is uncommon in practice but mandated by the spec.
    pub force_bold: bool,
}

/// Registry that caches parsed fonts by their global PDF ObjectId.
///
/// IMPORTANT: this is keyed by `ObjectId`, NOT by the page-local font name
/// (e.g. "F1"), because the same name can refer to different fonts on
/// different pages. ObjectId is the only stable global identifier for an
/// indirect font dictionary inside a single PDF document.
///
/// Inline (non-referenced) font dictionaries cannot be cached because they
/// have no stable identity — they get parsed on every lookup. This is rare
/// in practice; almost all PDFs share fonts via indirect references.
///
/// FontRegistry lives on `DocumentHandle` and survives across page renders,
/// so the expensive glyph-outline extraction (which dominates per-page cost
/// for text-heavy pages) only runs the first time a font is encountered.
pub struct FontRegistry {
    fonts: HashMap<ObjectId, Arc<FontEntry>>,
}

impl FontRegistry {
    pub fn new() -> Self {
        FontRegistry {
            fonts: HashMap::new(),
        }
    }

    /// Look up a font by its page-local name. Resolves the name through the
    /// page Resources/Font dictionary to a global `ObjectId`, then returns
    /// the cached `FontEntry` (or parses + caches it on miss).
    ///
    /// Returns `Arc<FontEntry>` so the caller can hold the entry across the
    /// borrow that filled the cache without lifetime contortions.
    pub fn get_font(
        &mut self,
        name: &str,
        doc: &Document,
        resources: &Dictionary,
    ) -> Option<Arc<FontEntry>> {
        self.get_font_with_id(name, doc, resources).map(|(_id, e)| e)
    }

    /// Same as `get_font` but also returns the font's global `ObjectId`
    /// when available. Used by the per-render glyph path cache so it can
    /// safely key on a stable identifier across pages.
    pub fn get_font_with_id(
        &mut self,
        name: &str,
        doc: &Document,
        resources: &Dictionary,
    ) -> Option<(Option<ObjectId>, Arc<FontEntry>)> {
        let (font_id_opt, font_dict) = Self::resolve_font_dict_with_id(name, doc, resources)?;

        // Cache hit (only possible for indirectly-referenced fonts)
        if let Some(font_id) = font_id_opt {
            if let Some(entry) = self.fonts.get(&font_id) {
                return Some((Some(font_id), entry.clone()));
            }
        }

        // Cache miss — do the expensive parse
        let entry = Arc::new(Self::build_font_entry(&font_dict, doc));

        // Cache only when we have a stable global ObjectId
        if let Some(font_id) = font_id_opt {
            self.fonts.insert(font_id, entry.clone());
        }

        Some((font_id_opt, entry))
    }

    /// Build a FontEntry from a font dictionary. This is the expensive
    /// per-font work — all of the calls inside (extract_and_parse_font,
    /// try_system_font, parse_truetype) walk every glyph in the embedded
    /// font and build a 64K-entry Unicode→GID cmap. Caching the result via
    /// `get_font()` saves all of this work on subsequent lookups within the
    /// same document.
    fn build_font_entry(font_dict: &Dictionary, doc: &Document) -> FontEntry {
        // Extract base font name
        let base_font = font_dict
            .get(b"BaseFont")
            .ok()
            .and_then(|o| Self::resolve_obj(o, doc))
            .and_then(|o| match o {
                Object::Name(n) => Some(String::from_utf8_lossy(&n).to_string()),
                _ => None,
            })
            .unwrap_or_default();

        // Detect Type0/CID fonts
        let subtype = font_dict
            .get(b"Subtype")
            .ok()
            .and_then(|o| Self::resolve_obj(o, doc))
            .and_then(|o| match o {
                Object::Name(n) => Some(n),
                _ => None,
            });
        let is_cid = subtype.as_deref() == Some(b"Type0");

        // Check CIDToGIDMap for Type0 fonts
        let cid_to_gid_identity = if is_cid {
            Self::check_cid_to_gid_identity(font_dict, doc)
        } else {
            false
        };

        // Extract encoding info
        let (encoding_name, differences) = Self::extract_encoding(font_dict, doc);

        // Extract ToUnicode CMap (maps char codes to Unicode codepoints)
        let to_unicode = Self::extract_to_unicode(font_dict, doc);
        let cid_to_unicode = if is_cid {
            Self::extract_cid_to_unicode(font_dict, doc)
        } else {
            HashMap::new()
        };

        // Try to extract and parse embedded font data.
        // For Type0/CID fonts the embedded font lives on the DescendantFont's
        // FontDescriptor, not on the parent Type0 dict — try descendant first.
        let mut parsed = if is_cid {
            Self::extract_descendant_font(font_dict, doc)
                .or_else(|| Self::extract_and_parse_font(font_dict, doc))
        } else {
            Self::extract_and_parse_font(font_dict, doc)
        };

        // For simple (non-CID) fonts, optionally fall back to a system font when
        // the embedded subset has no usable outlines for the common range.
        //
        // CRITICAL: never apply this fallback to Type0/CID fonts. CID fonts use
        // CID→GID mappings that are specific to the embedded TrueType (e.g. CID 46
        // = GID 46 = 'K' in this PDF). Substituting a system font would invalidate
        // that mapping and produce garbled text (issue #215).
        if !is_cid {
            // For TrueType embedded subsets, GIDs 1..=10 normally hold real
            // outlines if the subset is usable. For Type1 fonts parsed via
            // hayro-font we key glyphs by character code (GIDs ~32..=255), so
            // additionally check the printable-ASCII range.
            //
            // For TrueType subsets that renumber glyphs (where byte codes are
            // 1..N and the actual glyphs live at high GIDs), neither low nor
            // ASCII GID ranges contain outlines. Check `byte_cmap` (populated
            // from non-Unicode cmap subtables) to detect this case.
            let embedded_usable = parsed.as_ref().map(|p| {
                let count_outlines = |range: std::ops::RangeInclusive<u16>| -> usize {
                    range.filter(|gid| {
                        p.glyphs.get(gid).map(|g| !g.commands.is_empty()).unwrap_or(false)
                    }).count()
                };
                let low = count_outlines(1..=10);
                let ascii = count_outlines(0x41..=0x5A); // 'A'..'Z'

                // Subset font: count how many byte_cmap targets have outlines.
                let byte_cmap_outlines = p.byte_cmap.values().filter(|&&gid| {
                    p.glyphs.get(&gid).map(|g| !g.commands.is_empty()).unwrap_or(false)
                }).count();

                low > 5 || ascii > 5 || byte_cmap_outlines > 5
            }).unwrap_or(false);

            if parsed.is_none() || !embedded_usable {
                if let Some(sys_font) = Self::try_system_font(&base_font) {
                    parsed = Some(sys_font);
                }
            }
        } else if parsed.is_none() {
            // CID font with no embedded data — last resort system fallback.
            if let Some(sys_font) = Self::try_system_font(&base_font) {
                parsed = Some(sys_font);
            }
        }

        // Extract authoritative advance widths from the PDF font dictionary.
        // Per PDF spec §9.7.3 (simple) / §9.7.4.3 (CID), these override any
        // widths embedded in the font program — they are the values the
        // content stream's text-matrix advance must use.
        let (widths, default_width) = if is_cid {
            let (cid_w, dw) = Self::extract_cid_widths(font_dict, doc);
            let mut w_u32: HashMap<u32, f32> = HashMap::with_capacity(cid_w.len());
            for (cid, w) in cid_w {
                w_u32.insert(cid as u32, w);
            }
            (w_u32, dw)
        } else {
            let simple_w = Self::extract_widths(font_dict, doc);
            let mut w_u32: HashMap<u32, f32> = HashMap::with_capacity(simple_w.len());
            for (code, w) in simple_w {
                w_u32.insert(code as u32, w);
            }
            // Simple fonts: /MissingWidth from FontDescriptor (default 0).
            let dw = Self::extract_missing_width(font_dict, doc).unwrap_or(0.0);
            (w_u32, dw)
        };

        // Read FontDescriptor /Flags bit 19 (ForceBold).
        // For CID fonts the descriptor lives on the descendant; check both.
        let force_bold = Self::extract_force_bold(font_dict, doc);

        FontEntry {
            parsed,
            encoding_name,
            differences,
            base_font,
            is_cid,
            cid_to_gid_identity,
            to_unicode,
            cid_to_unicode,
            widths,
            default_width,
            force_bold,
        }
    }

    /// Read FontDescriptor `/Flags` and test bit 19 (ForceBold, 0x40000).
    /// Per PDF spec §9.8.1 Table 122. Returns `true` only when the flag is
    /// present and set; otherwise `false` (including all parse failures).
    /// For Type0 fonts the descriptor is on the descendant CIDFont, so we
    /// also probe `DescendantFonts[0]/FontDescriptor`.
    fn extract_force_bold(font_dict: &Dictionary, doc: &Document) -> bool {
        const FORCE_BOLD: i64 = 1 << 18; // bit 19, value 262144

        let read_flags = |desc: &Dictionary| -> i64 {
            desc.get(b"Flags")
                .ok()
                .and_then(|o| Self::resolve_obj(o, doc))
                .and_then(|o| match o {
                    Object::Integer(n) => Some(n),
                    Object::Real(n) => Some(n as i64),
                    _ => None,
                })
                .unwrap_or(0)
        };

        // Try parent dictionary's FontDescriptor first (simple fonts).
        if let Some(desc) = font_dict
            .get(b"FontDescriptor")
            .ok()
            .and_then(|o| Self::resolve_obj(o, doc))
            .and_then(|o| match o {
                Object::Dictionary(d) => Some(d),
                _ => None,
            })
        {
            if read_flags(&desc) & FORCE_BOLD != 0 {
                return true;
            }
        }

        // For Type0 fonts also probe the DescendantFonts entry.
        if let Some(arr) = font_dict
            .get(b"DescendantFonts")
            .ok()
            .and_then(|o| Self::resolve_obj(o, doc))
            .and_then(|o| match o {
                Object::Array(a) => Some(a),
                _ => None,
            })
        {
            if let Some(cid_dict) = arr.first().and_then(|o| Self::resolve_obj(o, doc))
                .and_then(|o| match o {
                    Object::Dictionary(d) => Some(d),
                    _ => None,
                })
            {
                if let Some(desc) = cid_dict
                    .get(b"FontDescriptor")
                    .ok()
                    .and_then(|o| Self::resolve_obj(o, doc))
                    .and_then(|o| match o {
                        Object::Dictionary(d) => Some(d),
                        _ => None,
                    })
                {
                    if read_flags(&desc) & FORCE_BOLD != 0 {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Resolve a character code to a glyph ID using the font entry.
    pub fn char_to_glyph_id(entry: &FontEntry, char_code: u8) -> Option<u16> {
        let parsed = entry.parsed.as_ref()?;

        // Priority 0: byte_cmap from non-Unicode subtables (Mac 1,0 / Win Symbol 3,0).
        // For TrueType subset fonts in PDFs this is the authoritative mapping
        // between content-stream byte codes and the subset's renumbered GIDs.
        //
        // We normally require the target GID to have an outline so this path
        // doesn't mask a working ToUnicode→cmap lookup against a system-font
        // fallback whose byte_cmap happens to be empty anyway.
        //
        // Exception: whitespace characters (space, NBSP, tab) legitimately have
        // empty outlines (advance-only). When ToUnicode tells us this char code
        // is whitespace, an empty-outline byte_cmap mapping is correct — we
        // must NOT fall through to the direct-index path which would resolve
        // the byte code to whatever unrelated glyph happens to live at that GID
        // in the subset (typically a visible character, producing a stray glyph
        // wherever a space should appear).
        if let Some(&gid) = parsed.byte_cmap.get(&char_code) {
            let has_outline = parsed
                .glyphs
                .get(&gid)
                .map(|g| !g.commands.is_empty())
                .unwrap_or(false);
            let is_whitespace = entry
                .to_unicode
                .get(&char_code)
                .map(|&u| matches!(u as u32, 0x0020 | 0x00A0 | 0x0009 | 0x000A | 0x000D))
                .unwrap_or(false);
            if has_outline || is_whitespace {
                return Some(gid);
            }
        }

        // Priority 1: ToUnicode → font cmap (works when cmap is populated)
        if let Some(&unicode_char) = entry.to_unicode.get(&char_code) {
            if let Some(&gid) = parsed.cmap.get(&(unicode_char as u32)) {
                return Some(gid);
            }
        }

        // Priority 2: Encoding + Differences → Unicode → cmap
        if entry.encoding_name.is_some() || !entry.differences.is_empty() {
            let ch = encoding::resolve_char_code(
                entry.encoding_name.as_deref(),
                &entry.differences,
                char_code,
            );
            if let Some(&gid) = parsed.cmap.get(&(ch as u32)) {
                return Some(gid);
            }
        }

        // Priority 3: Direct glyph index
        // For embedded subset TrueType fonts, character codes often map directly
        // to glyph indices (especially when cmap table is empty or Symbol-encoded)
        let gid = char_code as u16;
        if parsed.glyphs.contains_key(&gid) {
            return Some(gid);
        }

        None
    }

    /// Resolve the Font dictionary for a given font name from resources,
    /// returning the global `ObjectId` if the font is referenced indirectly.
    /// Inline (non-referenced) font dicts get `None` as the id and are
    /// re-parsed on every lookup.
    fn resolve_font_dict_with_id(
        name: &str,
        doc: &Document,
        resources: &Dictionary,
    ) -> Option<(Option<ObjectId>, Dictionary)> {
        let font_res = resources.get(b"Font").ok()?;
        let font_res_resolved = Self::resolve_obj(font_res, doc)?;
        let font_dict_parent = match font_res_resolved {
            Object::Dictionary(d) => d.clone(),
            _ => return None,
        };

        let font_obj = font_dict_parent.get(name.as_bytes()).ok()?;

        // The entry may be an indirect reference (which gives us a stable
        // ObjectId for caching) or an inline dictionary (no stable id).
        match font_obj {
            Object::Reference(id) => {
                let resolved = doc.get_object(*id).ok()?.clone();
                if let Object::Dictionary(d) = resolved {
                    Some((Some(*id), d))
                } else {
                    None
                }
            }
            Object::Dictionary(d) => Some((None, d.clone())),
            _ => None,
        }
    }

    /// Extract encoding name and Differences array from font dictionary.
    fn extract_encoding(
        font_dict: &Dictionary,
        doc: &Document,
    ) -> (Option<String>, HashMap<u8, String>) {
        let enc_obj = match font_dict.get(b"Encoding") {
            Ok(o) => Self::resolve_obj(o, doc),
            Err(_) => None,
        };

        match enc_obj {
            Some(Object::Name(n)) => {
                let name = String::from_utf8_lossy(&n).to_string();
                (Some(name), HashMap::new())
            }
            Some(Object::Dictionary(d)) => {
                let enc_name = d
                    .get(b"BaseEncoding")
                    .ok()
                    .and_then(|o| Self::resolve_obj(o, doc))
                    .and_then(|o| match o {
                        Object::Name(n) => Some(String::from_utf8_lossy(&n).to_string()),
                        _ => None,
                    });

                let diffs = d
                    .get(b"Differences")
                    .ok()
                    .and_then(|o| Self::resolve_obj(o, doc))
                    .and_then(|o| match o {
                        Object::Array(arr) => Some(encoding::parse_differences(&arr)),
                        _ => None,
                    })
                    .unwrap_or_default();

                (enc_name, diffs)
            }
            _ => (None, HashMap::new()),
        }
    }

    /// Try to extract embedded font data (FontFile2 or FontFile3) and parse it.
    fn extract_and_parse_font(font_dict: &Dictionary, doc: &Document) -> Option<ParsedFont> {
        // Get FontDescriptor
        let desc_obj = font_dict.get(b"FontDescriptor").ok()?;
        let desc_obj = Self::resolve_obj(desc_obj, doc)?;
        let desc = match desc_obj {
            Object::Dictionary(d) => d,
            _ => return None,
        };

        // Try FontFile2 (TrueType) first, then FontFile3 (CFF/OpenType)
        if let Ok(font_stream_obj) = desc.get(b"FontFile2").or_else(|_| desc.get(b"FontFile3")) {
            if let Some(font_data) = Self::get_stream_data(font_stream_obj, doc) {
                if let Ok(parsed) = font_parser::parse_truetype(&font_data) {
                    return Some(parsed);
                }
            }
        }

        // Fall back to FontFile (Type1 binary): use hayro-font to parse the
        // PostScript Type1 font and decode embedded charstrings into outlines,
        // so we render the *real* embedded letterforms (e.g. UniviaPro) rather
        // than substituting a system font.
        if let Ok(font_stream_obj) = desc.get(b"FontFile") {
            if let Some(font_data) = Self::get_stream_data(font_stream_obj, doc) {
                let widths_by_code = Self::extract_widths(font_dict, doc);
                let (enc_name, diffs) = Self::extract_encoding(font_dict, doc);
                if let Ok(parsed) = font_parser::parse_type1(
                    &font_data,
                    &widths_by_code,
                    enc_name.as_deref(),
                    &diffs,
                ) {
                    return Some(parsed);
                }
            }
        }

        None
    }

    /// Extract the per-character advance widths from a simple PDF font dict.
    /// PDF stores widths in a 1/1000 em unit, indexed FirstChar..=LastChar.
    fn extract_widths(font_dict: &Dictionary, doc: &Document) -> HashMap<u8, f32> {
        let mut out = HashMap::new();

        let first_char = font_dict
            .get(b"FirstChar")
            .ok()
            .and_then(|o| Self::resolve_obj(o, doc))
            .and_then(|o| match o {
                Object::Integer(i) => Some(i as i64),
                _ => None,
            })
            .unwrap_or(0);

        let widths_arr = font_dict
            .get(b"Widths")
            .ok()
            .and_then(|o| Self::resolve_obj(o, doc))
            .and_then(|o| match o {
                Object::Array(a) => Some(a),
                _ => None,
            });

        if let Some(arr) = widths_arr {
            for (i, w_obj) in arr.iter().enumerate() {
                let w = match Self::resolve_obj(w_obj, doc) {
                    Some(Object::Integer(n)) => n as f32,
                    Some(Object::Real(n)) => n as f32,
                    _ => continue,
                };
                let code = first_char + i as i64;
                if (0..=255).contains(&code) {
                    out.insert(code as u8, w);
                }
            }
        }

        out
    }

    /// Read `/MissingWidth` from the font's FontDescriptor.
    /// Per PDF spec §9.7.3, default is 0 for simple fonts.
    fn extract_missing_width(font_dict: &Dictionary, doc: &Document) -> Option<f32> {
        let desc_obj = font_dict.get(b"FontDescriptor").ok()?;
        let desc_obj = Self::resolve_obj(desc_obj, doc)?;
        let desc = match desc_obj {
            Object::Dictionary(d) => d,
            _ => return None,
        };
        match Self::resolve_obj(desc.get(b"MissingWidth").ok()?, doc)? {
            Object::Integer(n) => Some(n as f32),
            Object::Real(n) => Some(n as f32),
            _ => None,
        }
    }

    /// Extract per-CID widths from a Type0 font dict's descendant `/W` array.
    /// PDF spec §9.7.4.3 supports two forms:
    ///   `c [w1 w2 ... wn]` — widths for cids c..c+n-1
    ///   `c1 c2 w` — width w for all cids c1..=c2
    /// Returns (cid_widths, dw_default_in_glyph_units).
    fn extract_cid_widths(
        font_dict: &Dictionary,
        doc: &Document,
    ) -> (HashMap<u16, f32>, f32) {
        let mut out = HashMap::new();
        let mut dw = 1000.0_f32; // PDF default

        let desc_fonts_obj = match font_dict.get(b"DescendantFonts").ok() {
            Some(o) => o,
            None => return (out, dw),
        };
        let desc_fonts = match Self::resolve_obj(desc_fonts_obj, doc) {
            Some(Object::Array(a)) => a,
            _ => return (out, dw),
        };
        let cid_dict = match desc_fonts.first().and_then(|o| Self::resolve_obj(o, doc)) {
            Some(Object::Dictionary(d)) => d,
            _ => return (out, dw),
        };

        // /DW default width
        if let Some(dw_obj) = cid_dict.get(b"DW").ok().and_then(|o| Self::resolve_obj(o, doc)) {
            match dw_obj {
                Object::Integer(n) => dw = n as f32,
                Object::Real(n) => dw = n as f32,
                _ => {}
            }
        }

        // /W array
        let w_arr = match cid_dict.get(b"W").ok().and_then(|o| Self::resolve_obj(o, doc)) {
            Some(Object::Array(a)) => a,
            _ => return (out, dw),
        };

        let to_num = |o: &Object| -> Option<f32> {
            match o {
                Object::Integer(n) => Some(*n as f32),
                Object::Real(n) => Some(*n as f32),
                _ => None,
            }
        };

        let mut i = 0;
        while i < w_arr.len() {
            // First element must be the start CID (integer)
            let first_cid = match Self::resolve_obj(&w_arr[i], doc) {
                Some(Object::Integer(n)) if n >= 0 => n as u32,
                _ => {
                    i += 1;
                    continue;
                }
            };
            i += 1;
            if i >= w_arr.len() {
                break;
            }
            // Next is either an Array (form 1) or a number (form 2)
            match Self::resolve_obj(&w_arr[i], doc) {
                Some(Object::Array(widths_arr)) => {
                    // Form: c [w1 w2 ... wn]
                    for (j, w_obj) in widths_arr.iter().enumerate() {
                        if let Some(w) = to_num(w_obj) {
                            let cid = first_cid + j as u32;
                            if cid <= 0xFFFF {
                                out.insert(cid as u16, w);
                            }
                        }
                    }
                    i += 1;
                }
                Some(num_obj) => {
                    // Form: c1 c2 w  (already consumed c1, just read num as c2)
                    let last_cid = match num_obj {
                        Object::Integer(n) if n >= 0 => n as u32,
                        _ => {
                            i += 1;
                            continue;
                        }
                    };
                    i += 1;
                    if i >= w_arr.len() {
                        break;
                    }
                    let w = match Self::resolve_obj(&w_arr[i], doc).as_ref().and_then(to_num) {
                        Some(v) => v,
                        None => {
                            i += 1;
                            continue;
                        }
                    };
                    i += 1;
                    let lo = first_cid.min(last_cid);
                    let hi = first_cid.max(last_cid);
                    for cid in lo..=hi {
                        if cid <= 0xFFFF {
                            out.insert(cid as u16, w);
                        }
                    }
                }
                None => {
                    i += 1;
                }
            }
        }

        (out, dw)
    }

    /// Check if CIDToGIDMap is /Identity in the DescendantFont
    fn check_cid_to_gid_identity(font_dict: &Dictionary, doc: &Document) -> bool {
        let desc_fonts = match font_dict.get(b"DescendantFonts") {
            Ok(o) => Self::resolve_obj(o, doc),
            _ => return false,
        };
        let arr = match desc_fonts {
            Some(Object::Array(a)) => a,
            _ => return false,
        };
        let first = match arr.first() {
            Some(o) => Self::resolve_obj(o, doc),
            None => return false,
        };
        let cid_dict = match first {
            Some(Object::Dictionary(d)) => d,
            _ => return false,
        };
        // Check CIDToGIDMap.
        // Per ISO 32000-1 §9.7.4.2, the DEFAULT value of CIDToGIDMap for a
        // CIDFontType2 font is /Identity when the entry is absent — meaning
        // CID values map directly to GIDs in the embedded TrueType. We must
        // therefore treat a missing entry as Identity (true), not as false.
        match cid_dict.get(b"CIDToGIDMap") {
            Ok(obj) => {
                match Self::resolve_obj(obj, doc) {
                    Some(Object::Name(n)) => n == b"Identity",
                    // A stream-based CIDToGIDMap is a non-identity custom map
                    Some(Object::Stream(_)) => false,
                    // Anything else: treat as Identity (spec default)
                    _ => true,
                }
            }
            // Entry absent → spec default is Identity
            Err(_) => true,
        }
    }

    /// Extract font from DescendantFonts array (for Type0 fonts)
    fn extract_descendant_font(font_dict: &Dictionary, doc: &Document) -> Option<ParsedFont> {
        let desc_fonts = Self::resolve_obj(font_dict.get(b"DescendantFonts").ok()?, doc)?;
        let arr = match desc_fonts {
            Object::Array(a) => a,
            _ => return None,
        };
        let first = Self::resolve_obj(arr.first()?, doc)?;
        let cid_dict = match first {
            Object::Dictionary(d) => d,
            _ => return None,
        };
        // Try to get embedded font from the descendant's FontDescriptor
        Self::extract_and_parse_font(&cid_dict, doc)
    }

    /// Try to load a system font matching the PDF BaseFont name.
    /// Strips subset prefix (e.g., "ESYDQT+SegoeUI-Bold" → "SegoeUI-Bold")
    /// then maps to Windows font files.
    ///
    /// As a last resort (e.g. for Type1 fonts whose embedded /FontFile we
    /// cannot parse, like UniviaPro), falls back to Arial so text is at
    /// least visible — character mapping still works through the font's
    /// ToUnicode CMap → Arial's Unicode cmap → glyph IDs.
    fn try_system_font(base_font: &str) -> Option<ParsedFont> {
        // Strip subset prefix: 6 uppercase letters + '+'
        let clean_name = if base_font.len() > 7 && base_font.as_bytes()[6] == b'+' {
            &base_font[7..]
        } else {
            base_font
        };

        // Try to find system font file
        if let Some(font_path) = Self::find_system_font(clean_name) {
            if let Ok(font_data) = std::fs::read(&font_path) {
                match font_parser::parse_truetype(&font_data) {
                    Ok(parsed) => {
                        eprintln!("[fonts] Loaded system font: {} → {}", clean_name, font_path);
                        return Some(parsed);
                    }
                    Err(e) => {
                        eprintln!("[fonts] Failed to parse system font {}: {}", font_path, e);
                    }
                }
            }
        }

        // Last-resort generic fallback — pick a sane default based on the
        // font name's hints (bold/italic) so substitutions look reasonable.
        let lower = clean_name.to_lowercase();
        let is_bold = lower.contains("bold") || lower.contains("black") || lower.contains("heavy");
        let is_italic = lower.contains("italic") || lower.contains("oblique");
        let fallback_file = match (is_bold, is_italic) {
            (true, true) => "arialbi.ttf",
            (true, false) => "arialbd.ttf",
            (false, true) => "ariali.ttf",
            (false, false) => "arial.ttf",
        };
        let path = format!(r"C:\Windows\Fonts\{}", fallback_file);
        if let Ok(font_data) = std::fs::read(&path) {
            if let Ok(parsed) = font_parser::parse_truetype(&font_data) {
                eprintln!("[fonts] Generic fallback: {} → {}", clean_name, fallback_file);
                return Some(parsed);
            }
        }
        None
    }

    /// Find a system font file matching a PDF font name.
    /// Maps names like "SegoeUI-Bold" to "C:\Windows\Fonts\segoeuib.ttf"
    fn find_system_font(name: &str) -> Option<String> {
        let fonts_dir = r"C:\Windows\Fonts";

        // Normalize: lowercase, remove hyphens/spaces
        let normalized = name.to_lowercase().replace('-', "").replace(' ', "");

        // Common font name → file mappings
        let candidates = Self::font_name_to_files(&normalized);

        for candidate in &candidates {
            let path = format!("{}\\{}", fonts_dir, candidate);
            if std::path::Path::new(&path).exists() {
                return Some(path);
            }
        }

        // Brute-force: scan fonts directory for matching name
        if let Ok(entries) = std::fs::read_dir(fonts_dir) {
            let search = normalized.replace("bold", "").replace("italic", "")
                .replace("regular", "").replace("light", "");
            for entry in entries.flatten() {
                let filename = entry.file_name().to_string_lossy().to_lowercase();
                if filename.ends_with(".ttf") || filename.ends_with(".ttc") {
                    let stem = filename.trim_end_matches(".ttf").trim_end_matches(".ttc");
                    if stem.contains(&search) || search.contains(stem) {
                        return Some(entry.path().to_string_lossy().to_string());
                    }
                }
            }
        }

        None
    }

    /// Map normalized PDF font names to possible Windows font file names
    fn font_name_to_files(normalized: &str) -> Vec<String> {
        let mut files = Vec::new();

        // Segoe UI family
        if normalized.contains("segoeui") {
            if normalized.contains("bold") && normalized.contains("italic") {
                files.push("segoeuiz.ttf".to_string());
            } else if normalized.contains("bold") {
                files.push("segoeuib.ttf".to_string());
            } else if normalized.contains("italic") {
                files.push("segoeuii.ttf".to_string());
            } else {
                files.push("segoeui.ttf".to_string());
            }
        }
        // Arial family
        if normalized.contains("arial") {
            if normalized.contains("bold") && normalized.contains("italic") {
                files.push("arialbi.ttf".to_string());
            } else if normalized.contains("bold") {
                files.push("arialbd.ttf".to_string());
            } else if normalized.contains("italic") {
                files.push("ariali.ttf".to_string());
            } else {
                files.push("arial.ttf".to_string());
            }
        }
        // Times New Roman
        if normalized.contains("timesnewroman") || normalized.contains("times") {
            if normalized.contains("bold") && normalized.contains("italic") {
                files.push("timesbi.ttf".to_string());
            } else if normalized.contains("bold") {
                files.push("timesbd.ttf".to_string());
            } else if normalized.contains("italic") {
                files.push("timesi.ttf".to_string());
            } else {
                files.push("times.ttf".to_string());
            }
        }
        // Courier New
        if normalized.contains("courier") {
            if normalized.contains("bold") {
                files.push("courbd.ttf".to_string());
            } else {
                files.push("cour.ttf".to_string());
            }
        }
        // Helvetica → Arial on Windows
        if normalized.contains("helvetica") {
            if normalized.contains("bold") {
                files.push("arialbd.ttf".to_string());
            } else {
                files.push("arial.ttf".to_string());
            }
        }
        // Calibri
        if normalized.contains("calibri") {
            if normalized.contains("bold") {
                files.push("calibrib.ttf".to_string());
            } else if normalized.contains("italic") {
                files.push("calibrii.ttf".to_string());
            } else {
                files.push("calibri.ttf".to_string());
            }
        }

        // Generic: try the normalized name directly
        files.push(format!("{}.ttf", normalized));

        files
    }

    /// Resolve a CID (2-byte character code) to a glyph ID for Type0/CID fonts.
    /// With Identity-H + CIDToGIDMap=Identity: CID maps directly to glyph ID.
    pub fn cid_to_glyph_id(entry: &FontEntry, cid: u16) -> Option<u16> {
        let parsed = entry.parsed.as_ref()?;
        if entry.cid_to_gid_identity {
            // Direct mapping: CID = GID
            if parsed.glyphs.contains_key(&cid) {
                return Some(cid);
            }
        }
        // ToUnicode CMap → font cmap (text-extraction CMap, but may help when the
        // CID space is Unicode-aligned, e.g. Adobe-Japan1 etc.)
        if let Some(&unicode_char) = entry.cid_to_unicode.get(&cid) {
            if let Some(&gid) = parsed.cmap.get(&(unicode_char as u32)) {
                return Some(gid);
            }
        }
        // Last resort: try the CID directly as a Unicode codepoint in cmap.
        parsed.cmap.get(&(cid as u32)).copied()
    }

    /// Extract ToUnicode CMap from font dictionary.
    /// Parses beginbfchar/beginbfrange entries: <srcCode> <dstUnicode>
    fn extract_to_unicode(font_dict: &Dictionary, doc: &Document) -> HashMap<u8, char> {
        let mut map = HashMap::new();
        let tu_obj = match font_dict.get(b"ToUnicode") {
            Ok(o) => Self::resolve_obj(o, doc),
            _ => return map,
        };
        let cmap_data = match tu_obj {
            Some(Object::Stream(stream)) => {
                stream.decompressed_content().unwrap_or_default()
            }
            _ => return map,
        };

        let cmap_str = String::from_utf8_lossy(&cmap_data);

        // Parse beginbfchar: <src> <dst> pairs
        // Parse beginbfrange: <srcLo> <srcHi> <dstStart> ranges
        let mut in_bfchar = false;
        let mut in_bfrange = false;

        for line in cmap_str.lines() {
            let line = line.trim();
            if line.contains("beginbfchar") { in_bfchar = true; continue; }
            if line.contains("endbfchar") { in_bfchar = false; continue; }
            if line.contains("beginbfrange") { in_bfrange = true; continue; }
            if line.contains("endbfrange") { in_bfrange = false; continue; }

            if (in_bfchar || in_bfrange) && line.starts_with('<') {
                // Parse hex values between < >
                let hex_values: Vec<u32> = line
                    .split('>')
                    .filter_map(|part| {
                        let hex = part.trim().trim_start_matches('<');
                        if hex.is_empty() { return None; }
                        u32::from_str_radix(hex, 16).ok()
                    })
                    .collect();

                if in_bfchar && hex_values.len() >= 2 {
                    let code = hex_values[0] as u8;
                    if let Some(ch) = char::from_u32(hex_values[1]) {
                        map.insert(code, ch);
                    }
                } else if in_bfrange && hex_values.len() >= 3 {
                    let lo = hex_values[0] as u8;
                    let hi = hex_values[1] as u8;
                    let dst_start = hex_values[2];
                    for code in lo..=hi {
                        let unicode = dst_start + (code - lo) as u32;
                        if let Some(ch) = char::from_u32(unicode) {
                            map.insert(code, ch);
                        }
                    }
                }
            }
        }

        map
    }

    /// Extract ToUnicode CMap for CID fonts (2-byte source codes).
    fn extract_cid_to_unicode(font_dict: &Dictionary, doc: &Document) -> HashMap<u16, char> {
        let mut map = HashMap::new();
        let tu_obj = match font_dict.get(b"ToUnicode") {
            Ok(o) => Self::resolve_obj(o, doc),
            _ => return map,
        };
        let cmap_data = match tu_obj {
            Some(Object::Stream(stream)) => {
                stream.decompressed_content().unwrap_or_default()
            }
            _ => return map,
        };

        let cmap_str = String::from_utf8_lossy(&cmap_data);
        let mut in_bfchar = false;
        let mut in_bfrange = false;

        for line in cmap_str.lines() {
            let line = line.trim();
            if line.contains("beginbfchar") { in_bfchar = true; continue; }
            if line.contains("endbfchar") { in_bfchar = false; continue; }
            if line.contains("beginbfrange") { in_bfrange = true; continue; }
            if line.contains("endbfrange") { in_bfrange = false; continue; }

            if (in_bfchar || in_bfrange) && line.starts_with('<') {
                let hex_values: Vec<u32> = line
                    .split('>')
                    .filter_map(|part| {
                        let hex = part.trim().trim_start_matches('<');
                        if hex.is_empty() { return None; }
                        u32::from_str_radix(hex, 16).ok()
                    })
                    .collect();

                if in_bfchar && hex_values.len() >= 2 {
                    let code = hex_values[0] as u16;
                    if let Some(ch) = char::from_u32(hex_values[1]) {
                        map.insert(code, ch);
                    }
                } else if in_bfrange && hex_values.len() >= 3 {
                    let lo = hex_values[0] as u16;
                    let hi = hex_values[1] as u16;
                    let dst_start = hex_values[2];
                    for code in lo..=hi {
                        let unicode = dst_start + (code - lo) as u32;
                        if let Some(ch) = char::from_u32(unicode) {
                            map.insert(code, ch);
                        }
                    }
                }
            }
        }

        map
    }

    /// Get decompressed stream data from an object (possibly a reference).
    fn get_stream_data(obj: &Object, doc: &Document) -> Option<Vec<u8>> {
        let resolved = Self::resolve_obj(obj, doc)?;
        match resolved {
            Object::Stream(stream) => stream.decompressed_content().ok(),
            _ => None,
        }
    }

    /// Resolve an object reference, following indirect references.
    fn resolve_obj(obj: &Object, doc: &Document) -> Option<Object> {
        match obj {
            Object::Reference(id) => doc.get_object(*id).ok().cloned(),
            other => Some(other.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_registry_new() {
        let registry = FontRegistry::new();
        assert!(registry.fonts.is_empty());
    }

    #[test]
    fn test_char_to_glyph_id_no_parsed_font() {
        let entry = FontEntry {
            parsed: None,
            encoding_name: None,
            differences: HashMap::new(),
            base_font: "TestFont".to_string(),
            is_cid: false,
            cid_to_gid_identity: false,
            to_unicode: HashMap::new(),
            cid_to_unicode: HashMap::new(),
            widths: HashMap::new(),
            default_width: 500.0,
            force_bold: false,
        };
        assert_eq!(FontRegistry::char_to_glyph_id(&entry, b'A'), None);
    }
}
