use crate::data::{Cause, HeaderList, Response, Violation};
use fancy_regex::Regex;
use std::collections::HashMap;

#[allow(clippy::module_name_repetitions)]
pub struct RegexMatcher {
    pub kind: String,
}
impl RegexMatcher {
    #[must_use]
    pub fn new(kind: &str) -> Self {
        Self {
            kind: kind.to_string(),
        }
    }
    fn match_field(
        &self,
        name: &str,
        wire_field: &Option<String>,
        recorded_field: &Option<String>,
    ) -> Option<Violation> {
        if let Some(recorded_value) = recorded_field {
            if wire_field.is_none() {
                return Some(Violation {
                    kind: self.kind.clone(),
                    cause: Cause::WireMissing,
                    subject: name.to_string(),
                    on: Some(name.to_string()),
                    wire: None,
                    recorded: recorded_value.to_string(),
                });
            }

            let match_re = Regex::new(recorded_value).unwrap();
            if !match_re.is_match(wire_field.as_ref().unwrap()).unwrap() {
                return Some(Violation {
                    kind: self.kind.clone(),
                    cause: Cause::Mismatch,
                    subject: name.to_string(),
                    on: Some(name.to_string()),
                    wire: wire_field.clone(),
                    recorded: recorded_value.to_string(),
                });
            }
        }
        None
    }
    fn match_headers(
        &self,
        wire_headers: &Option<HashMap<String, HeaderList>>,
        recorded_headers: &Option<HashMap<String, HeaderList>>,
    ) -> Option<Violation> {
        if let Some(recorded_headers) = recorded_headers {
            if wire_headers.is_none() {
                return Some(Violation {
                    kind: self.kind.clone(),
                    cause: Cause::WireMissing,
                    subject: "headers".to_string(),
                    on: Some("all headers".to_string()),
                    wire: None,
                    recorded: format!("{recorded_headers:?}"),
                });
            }
            let wire_headers = wire_headers.as_ref().unwrap();
            let matches_headers = recorded_headers.iter().find(|(k, vs)| {
                let k = k.to_lowercase();
                if !wire_headers.contains_key(k.as_str()) {
                    return true;
                }
                let wire_header_values = &wire_headers[k.as_str()];
                !vs.iter().any(|v| {
                    let v_re = Regex::new(v.as_str()).unwrap();
                    wire_header_values
                        .iter()
                        .any(|wv| v_re.is_match(wv).unwrap())
                })
            });

            if let Some(matches_headers) = matches_headers {
                let (key, _) = matches_headers;
                let key = key.to_lowercase();
                return Some(Violation {
                    kind: self.kind.clone(),
                    cause: Cause::Mismatch,
                    subject: "headers".to_string(),
                    on: Some(key.to_string()),
                    wire: Some(format!(
                        "{:?}",
                        wire_headers.get(key.as_str()).unwrap_or(&vec![])
                    )),
                    recorded: format!("{:?}", matches_headers.1),
                });
            }
        }
        None
    }

    fn match_vars(
        &self,
        wire_vars: &Option<HashMap<String, String>>,
        recorded_vars: &Option<HashMap<String, String>>,
    ) -> Option<Violation> {
        if let Some(recorded_vars) = recorded_vars {
            if wire_vars.is_none() {
                return Some(Violation {
                    kind: self.kind.clone(),
                    cause: Cause::WireMissing,
                    subject: "vars".to_string(),
                    // XXX change 'on' to 'field'
                    on: Some("all vars".to_string()), // XXX should be None
                    wire: None,
                    recorded: format!("{recorded_vars:?}"),
                });
            }
            let wire_vars = wire_vars.as_ref().unwrap();
            let badly_matched_vars = recorded_vars.iter().find(|(k, v)| {
                let k = k.to_lowercase();
                if !wire_vars.contains_key(k.as_str()) {
                    return true;
                }
                let wire_var = &wire_vars[k.as_str()];
                let v_re = Regex::new(v.as_str()).unwrap();
                !v_re.is_match(wire_var).unwrap()
            });

            if let Some(badly_matched_vars) = badly_matched_vars {
                let (key, _) = badly_matched_vars;
                let key = key.to_lowercase();
                return Some(Violation {
                    kind: self.kind.clone(),
                    cause: Cause::Mismatch,
                    subject: "vars".to_string(),
                    on: Some(key.to_string()),
                    wire: Some(format!(
                        "{:?}",
                        wire_vars.get(key.as_str()).unwrap_or(&String::new())
                    )),
                    recorded: format!("{:?}", badly_matched_vars.1),
                });
            }
        }
        None
    }

    #[must_use]
    pub fn is_match(
        &self,
        wire_response: &Response,
        recorded_response: Option<&Response>,
    ) -> Vec<Violation> {
        recorded_response.map_or_else(
            || {
                vec![Violation {
                    kind: self.kind.clone(),
                    cause: Cause::RecordedMissing,
                    subject: "response".to_string(),
                    // XXX change 'on' to 'field'
                    on: None,
                    wire: None,
                    recorded: format!("{wire_response:?}"),
                }]
            },
            |recorded_response| {
                vec![
                    self.match_field("body", &wire_response.body, &recorded_response.body),
                    self.match_field(
                        "status_code",
                        &wire_response.status_code,
                        &recorded_response.status_code,
                    ),
                    self.match_headers(&wire_response.headers, &recorded_response.headers),
                    self.match_vars(&wire_response.vars, &recorded_response.vars),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
            },
        )
    }
}
