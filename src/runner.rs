use crate::reporters::create_reporter;
use std::collections::HashMap;

use crate::content::ContentCheck;
use crate::data::{Check, CheckResult, Context, Interaction, ReporterConfig, Sender};
use crate::sender::{SenderBuilder, SenderOptions};

pub struct RunOptions {
    pub sender: Box<dyn Sender>,
    pub flip: bool,
    pub reporters: HashMap<String, ReporterConfig>,
}
impl Default for RunOptions {
    fn default() -> Self {
        RunOptions::build(None, false, Some("console".into()), true)
    }
}

impl RunOptions {
    pub fn build(
        dry_run: Option<String>,
        flip: bool,
        reporter: Option<String>,
        verbose: bool,
    ) -> Self {
        let sender = SenderBuilder::build(SenderOptions { dry_run });
        let mut reporters = HashMap::new();
        let mut rc = HashMap::new();
        if verbose {
            rc.insert("verbose".to_string(), "true".to_string());
        }
        reporters.insert(reporter.unwrap_or_else(|| "console".to_string()), rc);
        RunOptions {
            sender,
            reporters,
            flip,
        }
    }
}
pub struct SequenceRunner<'a> {
    sender: &'a dyn Sender,
    flip: bool,
    reporters: HashMap<String, ReporterConfig>,
}

impl<'a> SequenceRunner<'a> {
    pub fn new(
        sender: &'a dyn Sender,
        flip: bool,
        reporters: HashMap<String, ReporterConfig>,
    ) -> Self {
        SequenceRunner {
            flip,
            sender,
            reporters,
        }
    }

    pub fn from_opts(run_opts: &'a RunOptions) -> Self {
        SequenceRunner {
            flip: run_opts.flip,
            sender: run_opts.sender.as_ref(),
            reporters: run_opts.reporters.clone(),
        }
    }

    pub fn run(&self, context: &mut Context, sequence: &[Interaction]) -> RunnerReport {
        let mut reporter = create_reporter(&self.reporters);
        let results = sequence
            .iter()
            .map(|interaction| {
                let checker = ContentCheck::new(self.sender);
                reporter.start(interaction);
                let res = checker.perform(context, interaction);
                reporter.report(interaction, &res);
                res
            })
            .collect::<Vec<_>>();

        reporter.end(sequence, &results);
        let ok = if self.flip {
            results.iter().all(|r| !r.violations.is_empty())
                && results.iter().all(|r| r.error.is_none())
        } else {
            results.iter().all(|r| r.violations.is_empty())
                && results.iter().all(|r| r.error.is_none())
        };
        RunnerReport { ok, results }
    }
}

pub struct RunnerReport {
    pub ok: bool,
    pub results: Vec<CheckResult>,
}

#[cfg(test)]
mod tests {
    use crate::sender::{SenderBuilder, SenderOptions};
    use crate::data::{Context, Interaction};
    use crate::runner::SequenceRunner;
    use mockito::{mock, server_address};
    use std::collections::HashMap;

    const ITC_OK: &str = include_str!("fixtures/ok.yaml");
    const ITC_OK_THEN_ERROR: &str = include_str!("fixtures/ok-then-error.yaml");

    #[test]
    fn test_runner_return_status_no_violations() {
        let _m1 = mock("GET", "/api/ok").with_status(200).create();
        let interactions = Interaction::sequence_interactions_from_yaml(ITC_OK).unwrap();
        let mut ctx = Context::new();
        ctx.vars_bag
            .insert("host".to_string(), server_address().to_string());

        let sender = SenderBuilder::build(SenderOptions { dry_run: None });
        let runner = SequenceRunner::new(sender.as_ref(), false, HashMap::new());
        let report = runner.run(&mut ctx, &interactions);
        assert_eq!(report.ok, true);
    }

    #[test]
    fn test_runner_return_status_with_violations() {
        let _m1 = mock("GET", "/api/ok").with_status(400).create();
        let interactions = Interaction::sequence_interactions_from_yaml(ITC_OK).unwrap();
        let mut ctx = Context::new();
        ctx.vars_bag
            .insert("host".to_string(), server_address().to_string());

        let sender = SenderBuilder::build(SenderOptions { dry_run: None });
        let runner = SequenceRunner::new(sender.as_ref(), false, HashMap::new());
        let report = runner.run(&mut ctx, &interactions);
        assert_eq!(report.ok, false);
    }

    #[test]
    fn test_runner_return_status_with_some_violations() {
        let _m1 = mock("GET", "/api/ok").with_status(200).create();
        let _m2 = mock("GET", "/api/error").with_status(200).create();
        let interactions = Interaction::sequence_interactions_from_yaml(ITC_OK_THEN_ERROR).unwrap();
        let mut ctx = Context::new();
        ctx.vars_bag
            .insert("host".to_string(), server_address().to_string());

        let sender = SenderBuilder::build(SenderOptions { dry_run: None });
        let runner = SequenceRunner::new(sender.as_ref(), false, HashMap::new());
        let report = runner.run(&mut ctx, &interactions);
        assert_eq!(report.ok, false);
    }

    #[test]
    fn test_runner_flip_return_status_no_violations() {
        let _m1 = mock("GET", "/api/ok").create();
        let interactions = Interaction::sequence_interactions_from_yaml(ITC_OK).unwrap();
        let mut ctx = Context::new();
        ctx.vars_bag
            .insert("host".to_string(), server_address().to_string());

        let sender = SenderBuilder::build(SenderOptions { dry_run: None });
        let runner = SequenceRunner::new(sender.as_ref(), true, HashMap::new());
        let report = runner.run(&mut ctx, &interactions);
        assert_eq!(report.ok, false);
    }

    #[test]
    fn test_runner_flip_return_status_with_violations() {
        let _m1 = mock("GET", "/api/ok").with_status(400).create();
        let interactions = Interaction::sequence_interactions_from_yaml(ITC_OK).unwrap();
        let mut ctx = Context::new();
        ctx.vars_bag
            .insert("host".to_string(), server_address().to_string());

        let sender = SenderBuilder::build(SenderOptions { dry_run: None });
        let runner = SequenceRunner::new(sender.as_ref(), true, HashMap::new());
        let report = runner.run(&mut ctx, &interactions);
        assert_eq!(report.ok, true);
    }

    #[test]
    fn test_runner_flip_return_status_with_some_violations() {
        let _m1 = mock("GET", "/api/ok").with_status(200).create();
        let _m2 = mock("GET", "/api/error").with_status(200).create();
        let interactions = Interaction::sequence_interactions_from_yaml(ITC_OK_THEN_ERROR).unwrap();
        let mut ctx = Context::new();
        ctx.vars_bag
            .insert("host".to_string(), server_address().to_string());

        let sender = SenderBuilder::build(SenderOptions { dry_run: None });
        let runner = SequenceRunner::new(sender.as_ref(), true, HashMap::new());
        let report = runner.run(&mut ctx, &interactions);
        assert_eq!(report.ok, false);
    }
}
