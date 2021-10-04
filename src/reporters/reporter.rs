pub use super::console_output::ConsoleOutput;
pub use super::json_output::JsonOutput;
pub use super::junit_output::JUnitOutput;
use crate::data::{CheckResult, Interaction, ReporterConfig, ReporterOutput};
use std::collections::HashMap;
use std::marker::Sync;
pub struct Reporter<'a> {
    pub outputs: Vec<Box<dyn ReporterOutput + 'a>>,
}
unsafe impl<'a> Sync for Reporter<'a> {}

impl<'a> Reporter<'a> {
    pub fn new(reporters: &HashMap<String, ReporterConfig>) -> Self {
        Reporter {
            outputs: reporters
                .iter()
                .map(|(key, cfg)| match key.as_ref() {
                    "json" => Box::new(JsonOutput::new(cfg)) as Box<dyn ReporterOutput>,
                    "console" => Box::new(ConsoleOutput::new(cfg)) as Box<dyn ReporterOutput>,
                    "junit" => Box::new(JUnitOutput::new(cfg)) as Box<dyn ReporterOutput>,
                    &_ => Box::new(ConsoleOutput::new(cfg)) as Box<dyn ReporterOutput>,
                })
                .collect(),
        }
    }
    pub fn start(&mut self, inter: &Interaction) {
        self.outputs.iter_mut().for_each(|r| r.start(inter))
    }

    pub fn report(&mut self, inter: &Interaction, check_result: &CheckResult) {
        self.outputs
            .iter_mut()
            .for_each(|r| r.report(inter, check_result))
    }

    pub fn end(&mut self, interactions: &[Interaction], results: &[CheckResult]) {
        self.outputs
            .iter_mut()
            .for_each(|r| r.end(interactions, results))
    }
}
