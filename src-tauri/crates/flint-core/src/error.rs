use std::fmt::Display;

pub fn background_error(error: impl Display) -> String {
    format!("Background task failed: {error}")
}
