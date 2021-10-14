use crate::vars::extract;
use anyhow::anyhow;
use anyhow::Result as AnyResult;
use log::*;
use std::collections::HashMap;
use std::time::Duration;
use subprocess::{Popen, PopenConfig, Redirection};

pub trait Sender {
    fn send(&self, interaction: &Interaction) -> AnyResult<Response>;
}

pub struct PrepareOpts {
    pub var_placeholder_open: String,
    pub var_placeholder_close: String,
}

#[derive(Debug, Serialize)]
struct Message<'a> {
    request: &'a Request,
    responses: &'a ResponseBag,
}

fn get_vars_from_cmd(
    cmd: &str,
    request: &Request,
    response_bag: &ResponseBag,
) -> HashMap<String, String> {
    let message = Message {
        request,
        responses: response_bag,
    };
    let serialized_message = serde_json::to_string(&message).unwrap();
    debug!("Executing {}", cmd);
    debug!("with stdin:\n{}", serialized_message);
    match Popen::create(
        &[cmd],
        PopenConfig {
            stdout: Redirection::Pipe,
            stdin: Redirection::Pipe,
            ..Default::default()
        },
    ) {
        Ok(mut p) => {
            let (out, _err) = p.communicate(Some(&serialized_message)).unwrap();

            if let Some(_exit_status) = p.poll() {
                if let Some(output) = out {
                    debug!("got:\n{}", output);
                    let vars: HashMap<String, String> =
                        serde_json::from_str(output.as_str()).unwrap();
                    debug!("into vars:\n{:?}", vars);
                    return vars;
                }
            // the process has finished
            } else {
                // it is still running, terminate it
                p.terminate().unwrap();
            }
        }
        Err(err) => error!("error executing vars command '{}': {}", cmd, err),
    }
    HashMap::new()
}

// fmtstring: {{var}} -> var is being replaced with real name to create the placeholder:
// ?q={{host}}, -> {{'var'->host}} -> {{host}} -> ?q=v
fn render_with_vars(text: String, vars: &HashMap<String, String>, fmtstring: &str) -> String {
    vars.iter().fold(text, |acc, (k, v)| {
        acc.replace(fmtstring.replace("var", k).as_str(), v)
    })
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VarInfo {
    pub expr: Option<String>,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub default: Option<String>,
}

pub trait Check {
    fn name(&self) -> &str;
    fn perform(&self, _context: &mut Context, _interaction: &Interaction) -> CheckResult;
}

#[derive(Debug, Serialize)]
pub struct CheckResult {
    pub request: Request,
    pub response: Option<Response>,
    pub violations: Vec<Violation>,
    pub duration: Option<Duration>,
    pub error: Option<String>,
    pub kind: String,
}
impl CheckResult {
    pub fn invalid_err(kind: &str, interaction: &Interaction, text: &str) -> Self {
        CheckResult {
            request: interaction.request.clone(),
            response: None,
            violations: vec![],
            duration: Some(Duration::new(0, 0)),
            error: Some(text.to_string()),
            kind: kind.to_string(),
        }
    }
    pub fn invalid(kind: &str, interaction: &Interaction) -> Self {
        CheckResult {
            request: interaction.request.clone(),
            response: None,
            violations: vec![],
            duration: Some(Duration::new(0, 0)),
            error: Some("Invalid check".to_string()),
            kind: kind.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Violation {
    pub kind: String,
    pub cause: Cause,
    pub subject: String,
    pub on: Option<String>,
    pub wire: Option<String>,
    pub recorded: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum Cause {
    WireMissing,
    RecordedMissing,
    Mismatch,
    Error,
}

pub trait ReporterOutput: Sync {
    fn start(&mut self, _interaction: &Interaction) {}
    fn report(&mut self, _interaction: &Interaction, _check_results: &CheckResult) {}
    fn end(&mut self, _interactions: &[Interaction], _results: &[CheckResult]) {}
}

pub type HeaderList = Vec<String>;
pub type ReporterConfig = HashMap<String, String>;
pub type ResponseBag = HashMap<String, Response>;
pub type VarsBag = HashMap<String, String>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    var_braces: Option<String>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Context {
    pub vars_bag: VarsBag,
    pub response_bag: ResponseBag,
    pub config: Config,
}
impl Default for Context {
    fn default() -> Self {
        Context {
            vars_bag: HashMap::new(),
            response_bag: HashMap::new(),
            config: Config { var_braces: None },
        }
    }
}
impl Context {
    pub fn new() -> Self {
        Context::default()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Runner {
    pub exit_on_failure: bool,
}
impl Default for Runner {
    fn default() -> Runner {
        Runner {
            exit_on_failure: false,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SequenceInteractions {
    pub http_interactions: Vec<Interaction>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Interaction {
    pub request: Request,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<Response>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invalid: Option<Response>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<HashMap<String, Response>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub benchmark: Option<Benchmark>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cert: Option<CertificateDetail>,
}
impl Interaction {
    pub fn sequence_interactions_from_yaml(content: &str) -> AnyResult<Vec<Interaction>> {
        let result: SequenceInteractions = serde_yaml::from_str(content)?;
        Ok(result.http_interactions)
    }
    pub fn from_yaml(content: &str) -> AnyResult<Interaction> {
        let result = serde_yaml::from_str(content)?;
        Ok(result)
    }
    pub fn types(&self) -> Vec<&str> {
        let mut v = vec![];
        if self.benchmark.is_some() {
            v.push("benchmark");
        }
        if self.response.is_some() {
            v.push("verify");
        }
        if self.cert.is_some() {
            v.push("cert");
        }
        v
    }
    pub fn prepare_with(&self, context: &mut Context) -> AnyResult<Self> {
        self.ensure_requirements(context)?;
        let mut res = self.clone();
        let mut req = res.request;
        let fmtstring = context
            .config
            .var_braces
            .clone()
            .unwrap_or_else(|| "{{var}}".to_string());
        let responses = &context.response_bag;
        let response_vars = &context.vars_bag;

        let mut vars: HashMap<String, String> = if let Some(command) = &req.vars_command {
            get_vars_from_cmd(command, &req, responses)
        } else {
            HashMap::new()
        };
        response_vars.iter().for_each(|(k, v)| {
            vars.insert(k.to_string(), v.to_string());
        });

        req.uri = render_with_vars(req.uri.clone(), &vars, &fmtstring);
        req.uri_list = req.uri_list.map(|uri_list| {
            uri_list
                .iter()
                .map(|uri| render_with_vars(uri.clone(), &vars, &fmtstring))
                .collect::<Vec<_>>()
        });
        if let Some(basic) = req.basic_auth.as_mut() {
            basic.user = render_with_vars(basic.user.clone(), &vars, &fmtstring);
            if let Some(password) = basic.password.as_ref() {
                basic.password = Some(render_with_vars(password.clone(), &vars, &fmtstring))
            }
        }

        if let Some(aws) = req.aws_auth.as_mut() {
            aws.key = render_with_vars(aws.key.clone(), &vars, &fmtstring);
            aws.secret = render_with_vars(aws.secret.clone(), &vars, &fmtstring);
            aws.service = render_with_vars(aws.service.clone(), &vars, &fmtstring);
            aws.region = aws
                .region
                .as_ref()
                .map(|region| render_with_vars(region.clone(), &vars, &fmtstring));
        }

        // mising uri_list
        req.headers = req.headers.map(|mut headers| {
            for val in headers.values_mut() {
                let rendered = val
                    .iter()
                    .map(|v| render_with_vars(v.clone(), &vars, &fmtstring))
                    .collect::<Vec<_>>();
                *val = rendered;
            }
            headers
        });
        req.body = req
            .body
            .map(|body| render_with_vars(body, &vars, &fmtstring));

        res.request = req;
        Ok(res)
    }
    pub fn send(&self, sender: &dyn Sender) -> AnyResult<Response> {
        let mut resp = sender.send(self)?;

        if let Some(vars) = &self.request.vars {
            let response_vars = extract(&resp, vars);
            resp.vars = Some(response_vars)
        }

        Ok(resp)
    }
    pub fn send_with_context(
        &self,
        sender: &dyn Sender,
        context: &mut Context,
    ) -> AnyResult<Response> {
        let prepared = self.prepare_with(context)?;
        let r = prepared.send(sender)?;
        r.save_vars(context);
        r.save_response(context);
        Ok(r)
    }

    pub fn ensure_requirements(&self, context: &Context) -> AnyResult<()> {
        if let Some(params) = self.request.params.as_ref() {
            let missing_params = params
                .iter()
                .filter(|p| !context.vars_bag.contains_key(&p.name))
                .collect::<Vec<_>>();
            if !missing_params.is_empty() {
                return Err(anyhow!(
                    "Missing required params:\n{}",
                    missing_params
                        .iter()
                        .map(|p| format!("name: {}\ndescription: {}\n", p.name, p.desc))
                        .collect::<Vec<_>>()
                        .join("\n")
                ));
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Benchmark {
    pub times: u64,
    pub avg_ms: u64,
    pub p95_ms: u64,
    pub p99_ms: u64,
    pub time_ms: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CertificateDetail {
    pub max_days: u64,
    pub subject: Option<String>,
    pub issuer: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Param {
    pub name: String,
    pub desc: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BasicAuth {
    pub user: String,
    pub password: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AWSAuth {
    pub region: Option<String>,
    pub service: String,
    pub key: String,
    pub secret: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Request {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Vec<Param>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub basic_auth: Option<BasicAuth>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub aws_auth: Option<AWSAuth>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, HeaderList>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub form: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri_list: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vars_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vars: Option<HashMap<String, VarInfo>>,
}
impl Request {
    pub fn get_id(&self) -> String {
        self.id
            .as_ref()
            .map_or_else(|| "request".to_string(), std::string::ToString::to_string)
    }
    pub fn get_desc(&self) -> String {
        format!(
            "{} ({})",
            self.desc.as_ref().unwrap_or(&"".to_string()),
            self.uri
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Response {
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, HeaderList>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vars: Option<HashMap<String, String>>,
}

impl Response {
    pub fn save_vars(&self, context: &mut Context) {
        let vars_bag = &mut context.vars_bag;
        if let Some(vars) = &self.vars {
            vars.iter().for_each(|(k, v)| {
                vars_bag.insert(k.clone(), v.clone());
            })
        }
    }
    pub fn save_response(&self, context: &mut Context) {
        if let Some(request_id) = self.request_id.as_ref() {
            let response_bag = &mut context.response_bag;
            response_bag.insert(request_id.clone(), self.clone());
        }
    }
}
