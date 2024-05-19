use crate::data::{CheckResult, Interaction, ReporterConfig, ReporterOutput};
use serde::Serialize;
use serde_json;

#[derive(Serialize)]
pub struct EndEvent<'a> {
    interactions: &'a [Interaction],
    results: &'a [CheckResult],
}
pub struct JsonOutput {}
unsafe impl Sync for JsonOutput {}

impl JsonOutput {
    pub const fn new(_config: &ReporterConfig) -> Self {
        Self {}
    }
}
impl ReporterOutput for JsonOutput {
    fn end(&mut self, interactions: &[Interaction], results: &[CheckResult]) {
        println!(
            "{}",
            serde_json::to_value(EndEvent {
                interactions,
                results,
            })
            .unwrap()
        );
    }
}
