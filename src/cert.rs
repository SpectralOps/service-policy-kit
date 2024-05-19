use chrono::prelude::*;
use native_tls::TlsConnector;

use crate::data::{Cause, Check, CheckResult, Context, Interaction, Violation};
use std::net::TcpStream;
use std::time::Instant;

use fancy_regex::Regex;
use x509_parser::parse_x509_der;
pub const NAME: &str = "cert";

#[derive(Default)]
pub struct Cert {}

impl Cert {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
fn error_violation(err: &str) -> Vec<Violation> {
    vec![Violation {
        kind: NAME.to_string(),
        cause: Cause::Error,
        on: Some("response".to_string()),
        subject: "request".to_string(),
        wire: Some(format!("error: {err}")),
        recorded: String::new(),
    }]
}
impl Check for Cert {
    fn name(&self) -> &str {
        NAME
    }
    fn perform(&self, _context: &mut Context, inter: &Interaction) -> CheckResult {
        let mut violations = vec![];
        if inter.cert.is_some() {
            let now = Instant::now();
            let connector = TlsConnector::new().unwrap();
            let url = match reqwest::Url::parse(inter.request.uri.as_str()) {
                Ok(url) => url,
                Err(err) => {
                    return CheckResult {
                        kind: NAME.to_string(),
                        request: inter.request.clone(),
                        violations: error_violation(&err.to_string()),
                        response: None,
                        duration: Some(now.elapsed()),
                        error: Some(err.to_string()),
                    }
                }
            };

            let der = match TcpStream::connect(format!("{}:443", url.host().unwrap())) {
                Ok(stream) => {
                    let stream = match connector
                        .connect(format!("{}", url.host().unwrap()).as_str(), stream)
                    {
                        Ok(stream) => stream,
                        Err(err) => {
                            return CheckResult {
                                kind: NAME.to_string(),
                                request: inter.request.clone(),
                                violations: error_violation(&err.to_string()),
                                response: None,
                                duration: Some(now.elapsed()),
                                error: None,
                            }
                        }
                    };
                    let cert = stream.peer_certificate().unwrap().unwrap();
                    cert.to_der().unwrap()
                }
                Err(err) => {
                    return CheckResult {
                        kind: NAME.to_string(),
                        request: inter.request.clone(),
                        violations: error_violation(&err.to_string()),
                        response: None,
                        duration: Some(now.elapsed()),
                        error: Some(err.to_string()),
                    }
                }
            };

            let (_, c) = parse_x509_der(&der).unwrap();
            let v = c.tbs_certificate.validity;
            let dt = Utc
                .ymd(
                    v.not_after.tm_year + 1900,
                    (v.not_after.tm_mon + 1) as u32,
                    v.not_after.tm_mday as u32,
                )
                .and_hms(
                    v.not_after.tm_hour as u32,
                    v.not_after.tm_min as u32,
                    v.not_after.tm_sec as u32,
                );
            if dt
                < chrono::Utc::now()
                    + chrono::Duration::days(inter.cert.as_ref().unwrap().max_days as i64)
            {
                violations.push(Violation {
                    kind: NAME.to_string(),
                    cause: Cause::Mismatch,
                    on: None,
                    subject: "expiry".to_string(),
                    wire: Some(format!(
                        "{:?}, ({} days left)",
                        dt,
                        (dt - chrono::Utc::now()).num_days()
                    )),
                    recorded: format!("> {} days", inter.cert.as_ref().unwrap().max_days),
                })
            }

            if let Some(issuer_expr) = inter.cert.as_ref().unwrap().issuer.as_ref() {
                let issuer = format!("{}", c.tbs_certificate.issuer);
                let match_re = Regex::new(issuer_expr).unwrap();
                if !match_re.is_match(&issuer).unwrap() {
                    violations.push(Violation {
                        kind: NAME.to_string(),
                        cause: Cause::Mismatch,
                        on: None,
                        subject: "issuer".to_string(),
                        wire: Some(issuer),
                        recorded: match_re.to_string(),
                    });
                }
            }

            if let Some(subject_expr) = inter.cert.as_ref().unwrap().subject.as_ref() {
                let subject = format!("{}", c.tbs_certificate.subject);
                let match_re = Regex::new(subject_expr).unwrap();
                if !match_re.is_match(&subject).unwrap() {
                    violations.push(Violation {
                        kind: NAME.to_string(),
                        cause: Cause::Mismatch,
                        on: None,
                        subject: "subject".to_string(),
                        wire: Some(subject),
                        recorded: match_re.to_string(),
                    });
                }
            }

            CheckResult {
                kind: NAME.to_string(),
                request: inter.request.clone(),
                violations,
                response: None,
                duration: Some(now.elapsed()),
                error: None,
            }
        } else {
            CheckResult::invalid(NAME, inter)
        }
    }
}
