use crate::data::{Cause, Check, CheckResult, Context, Interaction, Sender, Violation};
use histogram::Histogram;
use log::*;
use std::time::{Duration, Instant};
pub struct Bench<'a> {
    sender: &'a dyn Sender,
}
impl<'a> Bench<'a> {
    pub fn new(sender: &'a dyn Sender) -> Self {
        Self { sender }
    }
}

pub const NAME: &str = "bench";
impl<'a> Check for Bench<'a> {
    fn name(&self) -> &str {
        NAME
    }
    fn perform(&self, context: &mut Context, inter: &Interaction) -> CheckResult {
        let mut violations = vec![];
        if let Some(benchmark) = &inter.benchmark {
            let mut h = Histogram::new();
            let mut total: u128 = 0;

            let prepared = inter.prepare_with(context);
            if prepared.is_err() {
                return CheckResult {
                    kind: NAME.to_string(),
                    request: inter.request.clone(),
                    violations: vec![],
                    response: None,
                    duration: Some(Duration::new(0, 0)),
                    error: Some(prepared.err().unwrap().to_string()),
                };
            }

            let inter = prepared.ok().unwrap();
            for _ in 0..benchmark.times {
                debug!("Bench request: {:?}", &inter.request);
                let now = Instant::now();
                let res = inter.send(self.sender);
                match res {
                    Ok(_) => {}
                    Err(err) => {
                        return CheckResult {
                            kind: NAME.to_string(),
                            request: inter.request,
                            violations: vec![],
                            response: None,
                            duration: Some(now.elapsed()),
                            error: Some(err.to_string()),
                        };
                    }
                }
                let t = now.elapsed();
                let res = t.as_millis();
                h.increment(res as u64).unwrap();
                total += res;
            }

            let p95 = h.percentile(95.0).unwrap();
            if p95 > benchmark.p95_ms {
                violations.push(Violation {
                    kind: NAME.to_string(),
                    cause: Cause::Mismatch,
                    on: None,
                    subject: "p95".to_string(),
                    wire: Some(p95.to_string()),
                    recorded: benchmark.p95_ms.to_string(),
                })
            }
            // verify matching before considering as bench candidate
            let p99 = h.percentile(99.0).unwrap();
            if p99 > benchmark.p99_ms {
                violations.push(Violation {
                    kind: NAME.to_string(),
                    cause: Cause::Mismatch,
                    on: None,
                    subject: "p99".to_string(),
                    wire: Some(p99.to_string()),
                    recorded: benchmark.p99_ms.to_string(),
                })
            }

            let avg = h.mean().unwrap();
            if avg > benchmark.avg_ms {
                violations.push(Violation {
                    kind: NAME.to_string(),
                    cause: Cause::Mismatch,
                    on: None,
                    subject: "avg".to_string(),
                    wire: Some(avg.to_string()),
                    recorded: benchmark.avg_ms.to_string(),
                })
            }

            if total > u128::from(benchmark.time_ms) {
                violations.push(Violation {
                    kind: NAME.to_string(),
                    cause: Cause::Mismatch,
                    on: None,
                    subject: "time".to_string(),
                    wire: Some(total.to_string()),
                    recorded: benchmark.time_ms.to_string(),
                })
            }

            CheckResult {
                kind: NAME.to_string(),
                request: inter.request,
                violations,
                response: None,
                duration: None,
                error: None,
            }
        } else {
            CheckResult::invalid(NAME, inter)
        }
    }
}
