mod console_output;
mod json_output;
mod junit_output;
mod reporter;
pub use self::reporter::Reporter;
use crate::data::ReporterConfig;
use std::collections::HashMap;

#[must_use]
pub fn create_reporter<'a>(config: &HashMap<String, ReporterConfig>) -> Reporter<'a> {
    Reporter::new(config)
}
