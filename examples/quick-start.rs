use serde_yaml;
use service_policy_kit::data::{Context, SequenceInteractions};
use service_policy_kit::runner::{RunOptions, SequenceRunner};
use std::process::exit;

fn main() {
    let opts = RunOptions::default();
    let runner = SequenceRunner::from_opts(&opts);

    let sequence: SequenceInteractions = serde_yaml::from_str(
        r#"
http_interactions:
- request:
    id: step one
    uri: http://example.com
  response:
    status_code: "200"
"#,
    )
    .unwrap();
    let mut context = Context::new();
    let res = runner.run(&mut context, &sequence.http_interactions);
    exit(if res.ok { 0 } else { 1 })
}
