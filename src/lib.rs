#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_yaml;

extern crate env_logger;

extern crate histogram;
extern crate log;
extern crate reqwest;
pub mod bench;
pub mod cert;
pub mod content;
pub mod data;
pub mod discovery;
pub mod matcher;
pub mod reporters;
pub mod runner;
pub mod sender;
pub mod vars;

#[cfg(test)]
mod tests {
    use crate::data::{Context, Interaction, Violation};
    use crate::runner::SequenceRunner;
    use crate::sender::{SenderBuilder, SenderOptions};

    use mockito::{mock, server_address};
    use serde_json::json;
    use std::collections::HashMap;

    const ITC_SIMPLE: &str = include_str!("fixtures/simple.yaml");
    const ITC_JSON: &str = include_str!("fixtures/json.yaml");
    const ITC_WITH_DEFAULTS: &str = include_str!("fixtures/with-defaults.yaml");

    fn run_interactions(itc: &str) -> Vec<Violation> {
        let interactions = Interaction::sequence_interactions_from_yaml(itc).unwrap();
        let mut ctx = Context::new();
        ctx.vars_bag
            .insert("host".to_string(), server_address().to_string());

        let sender = SenderBuilder::build(SenderOptions { dry_run: None });
        let runner = SequenceRunner::new(sender.as_ref(), false, HashMap::new());
        let report = runner.run(&mut ctx, &interactions);
        report
            .results
            .iter()
            .flat_map(|r| r.violations.clone())
            .collect::<Vec<_>>()
    }

    #[test]
    fn test_passing_interaction_with_vars() {
        let _m1 = mock("GET", "/one").with_body("next: two").create();
        let _m2 = mock("GET", "/two").with_body("three").create();
        let results = run_interactions(ITC_SIMPLE);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_failing_interaction_with_vars() {
        let _m1 = mock("GET", "/one").with_body("next: two").create();

        // note: FAILING 'four' is not what we expect in interaction.
        let _m2 = mock("GET", "/two").with_body("four").create();

        let results = run_interactions(ITC_SIMPLE);
        assert_eq!(results[0].wire.clone().unwrap(), "four");
        assert_eq!(results[0].recorded, "three");
    }

    #[test]
    fn test_json_api() {
        let _m1 = mock("GET", "/api")
            .with_body(json!({"person":{"name":"joe"}}).to_string())
            .create();
        let _m2 = mock("GET", "/joe").with_body("hello, joe").create();
        let results = run_interactions(ITC_JSON);
        println!("{:?}", results);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_with_default_vars() {
        let _m1 = mock("GET", "/api")
            .with_body(json!({"person":{"pickup":"phone"}}).to_string())
            .create();
        let _m2 = mock("GET", "/armstrong")
            .with_body("hello, erlang")
            .create();
        let results = run_interactions(ITC_WITH_DEFAULTS);
        println!("{:?}", results);
        assert_eq!(results.len(), 0);
    }
}
