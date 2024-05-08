use crate::data::{CheckResult, Interaction, ReporterOutput};

use console::style;
use console::Term;
use difference::Changeset;
use std::collections::HashMap;
use std::fmt::Write;
const FAIL_SIGN: &str = "✗";
const SUCCESS_SIGN: &str = "✔";
pub struct ConsoleOutput {
    buffer: String,
    verbose: bool,
}

pub fn diff_text(expected: &str, actual: &str) -> (String, String, String) {
    let expected = format!("{expected:?}");
    let expected = &expected[1..expected.len() - 1];

    let actual = format!("{actual:?}");
    let actual = &actual[1..actual.len() - 1];

    let Changeset { diffs, .. } = Changeset::new(expected, actual, " ");
    let diff = diffs
        .into_iter()
        .map(|diff| match diff {
            difference::Difference::Same(s) => s,
            difference::Difference::Rem(s) => format!("{}", style(s).bold().white().on_green()),
            difference::Difference::Add(s) => format!("{}", style(s).bold().white().on_red()),
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<String>>()
        .join(" != ");

    (expected.to_string(), actual.to_string(), diff)
}

impl ConsoleOutput {
    pub fn new(config: &HashMap<String, String>) -> Self {
        let buf = String::new();
        Self::new_with_buffer(buf, config.contains_key("verbose"))
    }
    pub const fn new_with_buffer(buffer: String, verbose: bool) -> Self {
        Self { buffer, verbose }
    }
    // fn _print_diff_short(&self, benchdiffs: &[Violation]) -> String {
    //     let mut out = String::new();
    //     for b in benchdiffs {
    //         out.push_str(
    //             format!(
    //                 "  {} {}: {}\n",
    //                 style("expected:").green(),
    //                 b.subject,
    //                 b.recorded
    //             )
    //             .as_str(),
    //         );
    //         out.push_str(
    //             format!(
    //                 "       {} {}: {:?}\n",
    //                 style("got:").red(),
    //                 b.subject,
    //                 b.wire
    //             )
    //             .as_str(),
    //         );
    //     }
    //     out
    // }
    // fn _print_diff(&self, filter: &str, matchdiffs: &[Violation]) -> String {
    //     let mut out = "".to_string();

    //     let diffs = matchdiffs
    //         .iter()
    //         .filter(|m| m.subject == filter)
    //         .collect::<Vec<_>>();
    //     if !diffs.is_empty() {
    //         out.push_str(format!("{}:\n", style(filter).bold()).as_str());
    //         for m in &diffs {
    //             out.push_str(
    //                 format!(
    //                     "  {} {}: {}\n",
    //                     style("expected:").green(),
    //                     m.on.as_ref().unwrap_or(&"N/A".to_string()),
    //                     m.recorded
    //                 )
    //                 .as_str(),
    //             );
    //             out.push_str(
    //                 format!(
    //                     "       {} {}: {:?}\n",
    //                     style("got:").red(),
    //                     m.on.as_ref().unwrap_or(&"N/A".to_string()),
    //                     m.wire
    //                 )
    //                 .as_str(),
    //             );
    //             let (_, _, diff) =
    //                 diff_text(&m.recorded, &m.wire.clone().unwrap_or_else(String::new));
    //             out.push_str(format!("      diff: {diff}\n").as_str());
    //         }
    //     }
    //     out
    // }

    fn overwrite_previous_term() {
        let term = Term::stdout();
        if term.is_term() {
            term.clear_last_lines(1).unwrap();
        }
    }

    fn buffer_to_term(&self) {
        let term = Term::stdout();
        term.write_str(&self.buffer).unwrap();
        term.flush().unwrap();
    }
}
impl ReporterOutput for ConsoleOutput {
    fn start(&mut self, interaction: &Interaction) {
        self.buffer.clear();
        writeln!(
            self.buffer,
            "• {}: {}",
            interaction.request.get_id(),
            style("started").magenta()
        )
        .unwrap();

        self.buffer_to_term();
    }
    fn report(&mut self, interaction: &Interaction, check_results: &CheckResult) {
        self.buffer.clear();
        if !check_results.violations.is_empty() {
            writeln!(
                self.buffer,
                "{} {}: {} {}",
                style(FAIL_SIGN).red(),
                interaction.request.get_id(),
                style("failed").red(),
                style(format!("{}ms", check_results.duration.unwrap().as_millis())).dim(),
            )
            .unwrap();
            if self.verbose {
                check_results.violations.iter().for_each(|v| {
                    let (_, _, diff) = diff_text(&v.recorded, &v.wire.clone().unwrap_or_default());
                    writeln!(self.buffer, "      {}: {}", v.subject, diff).unwrap();
                });
            }
        } else if check_results.error.is_some() {
            writeln!(
                self.buffer,
                "{} {}: {} {}",
                style(FAIL_SIGN).red(),
                interaction.request.get_id(),
                style("error").red(),
                style(format!("{}ms", check_results.duration.unwrap().as_millis())).dim(),
            )
            .unwrap();
            writeln!(
                self.buffer,
                "{} error: {}",
                style("└─").red(),
                check_results.error.clone().unwrap()
            )
            .unwrap();
        } else {
            writeln!(
                self.buffer,
                "{} {}: {} {}",
                style(SUCCESS_SIGN).green(),
                interaction.request.get_id(),
                style("ok").green(),
                style(format!("{}ms", check_results.duration.unwrap().as_millis())).dim(),
            )
            .unwrap();
        }

        Self::overwrite_previous_term();
        self.buffer_to_term();
    }

    fn end(&mut self, interactions: &[Interaction], results: &[CheckResult]) {
        self.buffer.clear();
        if interactions.is_empty() {
            writeln!(self.buffer, "No interactions found").unwrap();
        } else {
            let duration = results
                .iter()
                .filter_map(|c| c.duration)
                .reduce(|d, acc| d + acc);
            write!(
                self.buffer,
                "\nRan {} interactions with {} checks in {}\n",
                style(interactions.len()).yellow(),
                style(results.len()).yellow(),
                style(format!("{}ms", duration.unwrap().as_millis())).yellow(),
            )
            .unwrap();

            if self.verbose {
                write!(
                    self.buffer,
                    "\nSuccess: {}\nFailure: {}\n  Error: {}\nSkipped: {}\n",
                    style(
                        results
                            .iter()
                            .filter(|c| c.error.is_none()
                                && c.response.is_some()
                                && c.violations.is_empty())
                            .count()
                    )
                    .green(),
                    style(
                        results
                            .iter()
                            .filter(|c| c.error.is_none()
                                && c.response.is_some()
                                && !c.violations.is_empty())
                            .count()
                    )
                    .red(),
                    style(results.iter().filter(|c| c.error.is_some()).count()).red(),
                    style(
                        results
                            .iter()
                            .filter(|c| c.error.is_none() && c.response.is_none())
                            .count()
                    )
                    .dim(),
                )
                .unwrap();
            }
        }

        self.buffer_to_term();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Cause;
    use crate::data::Violation;
    use std::time::Duration;

    #[test]
    fn clicolor_behavior() {
        let inter = Interaction::from_yaml(
            r#"
      request:
        id: "postbin:validation"
        desc: "Postbin: valid key"
        uri: "https://postb.in/xx?q={{key1}}"
        headers:
          Authorization:
          - Bearer {{key2}}
      response:
        status_code: "200"
      examples:
        ok:
          status_code: "201"
          body: hello
        err:
          status_code: "500"
"#,
        )
        .unwrap();
        let buffer = String::new();

        let mut o = ConsoleOutput::new_with_buffer(buffer, true);
        o.start(&inter);
        assert_eq!(
            o.buffer.to_string(),
            "• postbin:validation: \u{1b}[35mstarted\u{1b}[0m\n"
        );
        let fake_result = CheckResult {
            kind: "content".to_string(),
            request: inter.request.clone(),
            response: None,
            duration: Some(Duration::new(2, 0)),
            error: None,
            violations: vec![Violation {
                kind: "content".to_string(),
                cause: Cause::WireMissing,
                subject: "content".to_string(),
                on: None,
                wire: None,
                recorded: String::new(),
            }],
        };
        o.report(&inter, &fake_result);
        assert_eq!(
            o.buffer.to_string(),
            "\u{1b}[31m✗\u{1b}[0m postbin:validation: \u{1b}[31mfailed\u{1b}[0m \u{1b}[2m2000ms\u{1b}[0m\n      content: \n"
        );

        o.end(&vec![inter], &vec![fake_result]);
        assert_eq!(o.buffer.to_string(), "\nRan \u{1b}[33m1\u{1b}[0m interactions with \u{1b}[33m1\u{1b}[0m checks in \u{1b}[33m2000ms\u{1b}[0m\n\nSuccess: \u{1b}[32m0\u{1b}[0m\nFailure: \u{1b}[31m0\u{1b}[0m\n  Error: \u{1b}[31m0\u{1b}[0m\nSkipped: \u{1b}[2m1\u{1b}[0m\n");
    }
}
