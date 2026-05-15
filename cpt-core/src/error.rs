//! Error type — see Task 3.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CptError {
    #[error("placeholder")]
    Placeholder,
}
