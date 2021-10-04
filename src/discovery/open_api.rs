use crate::data::{Interaction, Request, Response};
use anyhow::Result as AnyResult;
use openapi;
use std::collections::HashMap;
use std::io::Read;

// WIP: OpenAPI support
pub struct OpenAPI {
    #[allow(dead_code)]
    opts: HashMap<String, String>,
}

impl OpenAPI {
    pub fn new(opts: HashMap<String, String>) -> Self {
        Self { opts }
    }
    // XXX 'top' is not used
    pub fn discover<R: Read>(&self, source: &mut R, _top: i32) -> AnyResult<Vec<Interaction>> {
        let interactions = match openapi::from_reader(source) {
            Ok(spec) => spec
                .paths
                .iter()
                .map(|(path, item)| {
                    let verb = if item.get.is_some() {
                        "get"
                    } else if item.post.is_some() {
                        "post"
                    } else if item.delete.is_some() {
                        "delete"
                    } else if item.put.is_some() {
                        "put"
                    } else {
                        return None;
                    };
                    Some(Interaction {
                        request: Request {
                            params: None,
                            method: Some(verb.to_string()),
                            basic_auth: None,
                            aws_auth: None,
                            form: None,
                            uri: format!("http://{{{{host}}}}{}", path),
                            id: None,
                            desc: None,
                            timeout_ms: None,
                            headers: None,
                            body: None,
                            uri_list: None,
                            vars_command: None,
                            vars: None,
                        },
                        response: Some(Response {
                            headers: None,
                            status_code: Some("200".to_string()),
                            body: None,
                            vars: None,
                            request_id: None,
                        }),
                        benchmark: None,
                        cert: None,
                        examples: None,
                    })
                })
                .flatten()
                .collect::<Vec<_>>(),
            Err(_e) => vec![], // XXX not used
        };
        Ok(interactions)
    }
}
