//! GEF parser. Public entry point: `parse(text) -> Result<Cpt>` (see Task 8).

pub mod columns;
pub mod header;
pub mod data;

pub use self::data::parse;
