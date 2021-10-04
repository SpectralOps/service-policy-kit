use crate::data::{CheckResult, Interaction, ReporterConfig, ReporterOutput};
use junit_report::{Duration as JUnitDuration, Report, TestCase, TestSuite};
use std::fs;
pub struct JUnitOutput {
    config: ReporterConfig,
}
unsafe impl Sync for JUnitOutput {}

impl JUnitOutput {
    pub fn new(config: &ReporterConfig) -> JUnitOutput {
        JUnitOutput {
            config: config.clone(),
        }
    }
}
impl ReporterOutput for JUnitOutput {
    fn end(&mut self, _interactions: &[Interaction], results: &[CheckResult]) {
        let mut suite = TestSuite::new("Violation Checks");
        let mut cases = vec![];
        results.iter().for_each(|res| {
            let success = res.violations.is_empty();
            let test_name = format!("[{}] {}", res.kind, res.request.get_id());
            if success {
                cases.push(TestCase::success(
                    test_name.as_str(),
                    JUnitDuration::from_std(res.duration.unwrap()).unwrap(),
                ));
            } else {
                cases.push(TestCase::failure(
                    test_name.as_str(),
                    JUnitDuration::from_std(res.duration.unwrap()).unwrap(),
                    "ERROR",
                    serde_yaml::to_string(&res.violations).unwrap().as_str(),
                ));
            }
        });
        suite.add_testcases(cases.into_iter());
        let mut junit_report = Report::new();
        junit_report.add_testsuite(suite);
        let mut out: Vec<u8> = Vec::new();
        junit_report.write_xml(&mut out).unwrap();

        let junit_xml = "junit-out".to_string();
        let pref = self.config.get("folder").unwrap_or(&junit_xml);
        if !std::path::Path::new(pref).exists() {
            fs::create_dir(pref).unwrap();
        }
        let f = format!("{}/junit.xml", pref);
        println!("wrote: {}", f);
        fs::write(&f, &out).unwrap();
    }
}
