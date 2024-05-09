use crate::data::{Interaction, Response, Sender};
use anyhow::Result as AnyResult;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use rusoto_core::{credential::AwsCredentials, signature::SignedRequest, Region};

use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

#[allow(clippy::module_name_repetitions)]
pub struct SenderOptions {
    pub dry_run: Option<String>,
}

#[allow(clippy::module_name_repetitions)]
pub struct SenderBuilder {}

impl SenderBuilder {
    #[must_use]
    pub fn build(opts: SenderOptions) -> Box<dyn Sender> {
        let s: Box<dyn Sender> = match opts.dry_run {
            Some(examples_key) => Box::new(DrySender::new(&examples_key)),
            None => Box::new(ReqwestSender::new()),
        };
        s
    }
}

#[derive(Default)]
#[allow(clippy::module_name_repetitions)]
pub struct ReqwestSender {}

impl ReqwestSender {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
impl Sender for ReqwestSender {
    #[allow(clippy::too_many_lines)]
    fn send(&self, inter: &Interaction) -> AnyResult<Response> {
        let request = &inter.request;
        // as_request -> RQRequest
        let uri = request.uri.clone();
        log::debug!("uri with vars: {}", uri);
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(request.timeout_ms.unwrap_or(10000)))
            .build()
            .unwrap();
        let method = request
            .method
            .as_ref()
            .unwrap_or(&"GET".to_string())
            .to_uppercase();

        let mut rq_builder = client.request(
            reqwest::Method::from_bytes(&method.clone().into_bytes()).unwrap(),
            &uri,
        );
        rq_builder = rq_builder.header("User-Agent", "keyscope/1");
        if let Some(basic) = &request.basic_auth {
            rq_builder = rq_builder.basic_auth(basic.user.clone(), basic.password.clone());
        }
        if let Some(form) = &request.form {
            rq_builder = rq_builder.form(form);
        }

        if let Some(aws) = &request.aws_auth {
            let credentials =
                AwsCredentials::new(aws.key.clone(), aws.secret.clone(), aws.token.clone(), None);
            let default_region = "us-east-1".to_string();
            let reg_str = aws.region.as_ref().unwrap_or(&default_region);

            let region: Region = Region::from_str(reg_str).unwrap_or_else(|_| Region::Custom {
                name: reg_str.to_string(),
                endpoint: aws.endpoint.clone().unwrap_or_default(),
            });

            let mut headers = HeaderMap::new();

            // note the path is '/' because at this point we only care about checking service-level access
            let mut signed_request =
                SignedRequest::new(method.as_str(), aws.service.as_str(), &region, "/");

            signed_request.set_payload(request.body.as_ref().map(|b| b.clone().into_bytes()));

            if let Some(content_type) = request
                .headers
                .as_ref()
                .and_then(|h| h.get("content-type").and_then(|c| c.iter().next()))
            {
                signed_request.set_content_type(content_type.to_string());
            }

            signed_request.sign(&credentials);

            let rh = signed_request.headers();

            for h in &[
                "x-amz-content-sha256",
                "x-amz-date",
                "authorization",
                "content-type",
                "host",
            ] {
                headers.insert(
                    (*h).to_string().parse::<HeaderName>().unwrap(),
                    String::from_utf8_lossy(&rh.get(*h).unwrap()[0])
                        .parse()
                        .unwrap(),
                );
            }

            if let Some(token) = aws.token.as_ref() {
                headers.insert("X-Amz-Security-Token", token.parse()?);
            }

            rq_builder = rq_builder.headers(headers);
        }

        if let Some(headers) = &request.headers {
            let mut headersmap = HeaderMap::new();
            for (key, val) in headers {
                for v in val {
                    headersmap.insert(
                        key.to_lowercase().parse::<HeaderName>().unwrap(),
                        HeaderValue::from_str(v.clone().as_str()).unwrap(),
                    );
                }
            }
            rq_builder = rq_builder.headers(headersmap);
        };

        if let Some(body) = &request.body {
            rq_builder = rq_builder.body(reqwest::blocking::Body::from(body.to_string()));
        }

        // GO!
        let rq_resp = rq_builder.send()?;

        // from_reqest -> RQResponse
        let mut headers: HashMap<String, Vec<String>> = HashMap::new();
        rq_resp.headers().iter().for_each(|(key, value)| {
            if value.to_str().is_ok() {
                let k = key.to_string();
                if !headers.contains_key(&k) {
                    headers.insert(k.to_string(), vec![]);
                }
                headers
                    .get_mut(&k)
                    .unwrap()
                    .push(value.to_str().unwrap().to_string());
            }
        });
        let resp = Response {
            status_code: Some(rq_resp.status().to_string()),
            headers: Some(headers),
            request_id: Some(request.get_id()),
            vars: None,
            body: Some(rq_resp.text().unwrap()),
        };

        Ok(resp)
    }
}

#[allow(clippy::module_name_repetitions)]
pub struct DrySender {
    example: String,
}

impl DrySender {
    #[must_use]
    pub fn new(example: &str) -> Self {
        Self {
            example: example.to_string(),
        }
    }
}

impl Sender for DrySender {
    fn send(&self, inter: &Interaction) -> AnyResult<Response> {
        let request = &inter.request;
        if let Some(examples) = &inter.examples {
            if let Some(ex) = examples.get(&self.example) {
                return Ok(ex.clone());
            }
            eprintln!("dry_send not found example: {}", self.example);
            eprintln!("examples: {:?}", inter.examples);
        }
        // no example was given
        Ok(Response {
            request_id: Some(request.get_id()),
            headers: None,
            status_code: Some("200".to_string()),
            body: Some("{ \"ok\": true }".to_string()),
            vars: None,
        })
    }
}
