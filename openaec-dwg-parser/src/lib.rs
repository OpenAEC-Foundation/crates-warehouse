//! # openaec-dwg-parser
//!
//! A **clean-room** parser for the AutoCAD DWG binary drawing format.
//!
//! ## Clean-room status
//!
//! This crate was written **from scratch** using **only** the publicly
//! available *Open Design Specification for .dwg files* PDF published
//! by the [Open Design Alliance](https://www.opendesign.com/). It does
//! **not** link against, copy from, or otherwise derive from any
//! existing DWG implementation:
//!
//! - No LibreDWG code.
//! - No ODA Teigha / Drawings SDK code.
//! - No Autodesk RealDWG / AutoCAD source.
//! - No external DWG binaries are invoked (no `accoreconsole`,
//!   `dwgread`, ODA File Converter, DWG TrueView, etc.).
//!
//! Where the ODA spec is ambiguous, the source is marked with a
//! `TODO/UNSURE` comment rather than peeking at a reference
//! implementation. See [`SPEC_NOTES.md`](https://github.com/OpenAEC-Foundation/crates-warehouse/blob/main/openaec-dwg-parser/SPEC_NOTES.md)
//! for the canonical provenance log — paragraph-numbered citations
//! into the ODA PDF accompany every non-trivial decoding decision.
//!
//! ## Version coverage
//!
//! | DWG version | Magic codes | Decode status |
//! |-------------|-------------|---------------|
//! | R14         | `AC1014`    | partial — header + entity stream |
//! | R2000       | `AC1015`    | tables, blocks, entities |
//! | R2004       | `AC1018`    | tables, blocks, entities |
//! | R2007       | `AC1021`    | tables, blocks, entities (UTF-16 string stream) |
//! | R2010       | `AC1024`    | tables, blocks, entities (R2010+ OT encoding) |
//! | R2013       | `AC1027`    | tables, blocks, entities |
//! | R2018       | `AC1032`    | tables, blocks, entities (page-map / section-map) |
//!
//! Entity types decoded so far include `LINE`, `CIRCLE`, `ARC`,
//! `LWPOLYLINE`, `POLYLINE` (legacy 2D), `INSERT`, `HATCH`, `TEXT`,
//! `MTEXT`, `SOLID`, `DIMENSION` (most subtypes), `ELLIPSE`, `SPLINE`,
//! `POINT`, `3DFACE`. Anything not yet decoded surfaces as an
//! `unknown` object — the parser never silently drops data.
//!
//! ## Known-good fixtures
//!
//! The clean-room workflow is anchored to a handful of small DWG files
//! per major version (single-line, single-circle, single-arc plus a
//! `pair.dwg` containing both an entity and a header-dependent
//! reference). Byte-stable round-tripping of these fixtures is the
//! primary regression gate. Larger third-party corpora (NextGIS,
//! AcadSharp, LibreDWG-testdata) are used for stress testing only —
//! never as a code source.
//!
//! ## Quick start
//!
//! ```no_run
//! use dwg_parser::DwgParser;
//!
//! let bytes = std::fs::read("drawing.dwg").unwrap();
//! let mut parser = DwgParser::new();
//! let file = parser.parse(&bytes).unwrap();
//! println!("Detected DWG version: {}", file.version);
//! ```
//!
//! ## License
//!
//! Licensed under the MIT license — see `LICENSE` at the workspace
//! root. Contributions must respect the clean-room rules above; any
//! patch that references or paraphrases an existing DWG implementation
//! will be rejected.

/// Debug-print macro for the DWG parser.
///
/// Expands to `eprintln!` only when the `dwg-debug` cargo feature is
/// enabled; otherwise it expands to nothing and the formatting arguments
/// are not evaluated. Used for the verbose `[dwg-dbg]` traces that
/// document the clean-room reverse-engineering work — useful while
/// chasing a new fixture, but noise on a normal parse.
#[macro_export]
macro_rules! dwg_dbg {
    ($($arg:tt)*) => {{
        #[cfg(feature = "dwg-debug")]
        { eprintln!($($arg)*); }
        // In release builds the format args are formally consumed via
        // `format_args!` so that any locals only used inside debug
        // prints are not flagged as unused.
        #[cfg(not(feature = "dwg-debug"))]
        { let _ = format_args!($($arg)*); }
    }};
}

pub mod error;
pub mod bitreader;
pub mod r2007;
pub mod parser;

pub use parser::{DwgParser, DwgFile, DwgObject, DwgClass, DwgVersion};
pub use error::DwgError;
