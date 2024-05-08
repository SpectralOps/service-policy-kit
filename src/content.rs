use std::time::Instant;

use crate::data::{Check, CheckResult, Context, Interaction, Sender};
use crate::matcher::RegexMatcher;
pub const NAME: &str = "content";

#[allow(clippy::module_name_repetitions)]
pub struct ContentCheck<'a> {
    sender: &'a dyn Sender,
}
impl<'a> ContentCheck<'a> {
    pub fn new(sender: &'a dyn Sender) -> Self {
        ContentCheck { sender }
    }
}

impl<'a> Check for ContentCheck<'a> {
    fn name(&self) -> &str {
        NAME
    }
    fn perform(&self, context: &mut Context, interaction: &Interaction) -> CheckResult {
        if interaction.response.is_some() {
            let now = Instant::now();
            // main func should always return check result
            // match here and move err into CheckResult.err
            let r = interaction.send_with_context(self.sender, context);
            match r {
                Ok(resp) => {
                    let matcher = RegexMatcher::new(NAME);
                    let vs = matcher.is_match(&resp, interaction.invalid.as_ref());
                    if vs.is_empty() {
                        return CheckResult::invalid_err(
                            self.name(),
                            interaction,
                            "matched invalid response",
                        );
                    }
                    let vs = matcher.is_match(&resp, interaction.response.as_ref());

                    CheckResult {
                        kind: NAME.to_string(),
                        request: interaction.request.clone(),
                        response: Some(resp),
                        violations: vs,
                        duration: Some(now.elapsed()),
                        error: None,
                    }
                }
                Err(err) => CheckResult {
                    kind: NAME.to_string(),
                    request: interaction.request.clone(),
                    response: None,
                    violations: vec![],
                    duration: Some(now.elapsed()),
                    error: Some(err.to_string()),
                },
            }
        } else {
            CheckResult::invalid(self.name(), interaction)
        }
    }
}
