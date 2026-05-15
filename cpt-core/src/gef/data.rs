//! GEF data section parser — implemented in Task 8.

use crate::error::CptError;
use crate::domain::Cpt;

pub fn parse(_text: &str) -> Result<Cpt, CptError> {
    Err(CptError::InvalidGef("not yet implemented".into()))
}
