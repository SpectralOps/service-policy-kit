use crate::data::{Response, VarInfo};
use anyhow::{Context, Result as AnyResult};
use serde_json;
use serde_json::json;
use std::collections::HashMap;

pub fn extract(
    response: &Response,
    infos: &HashMap<String, VarInfo>,
) -> AnyResult<HashMap<String, String>> {
    let jsonres = json!({
        "body": response.body,
        "headers": response.headers,
        "status": response.status_code,
    });
    let mut res = HashMap::new();
    for (k, v) in infos {
        res.insert(k.to_string(), extract_var(&jsonres, v)?);
    }
    Ok(res)
}

pub fn extract_var(v: &serde_json::Value, info: &VarInfo) -> AnyResult<String> {
    let blank = json!("");
    let v = if info.kind == "json" {
        json!({
            "body": serde_json::from_str(v.get("body").context("body key not found")?.as_str().unwrap()).unwrap_or_else(|_| json!({})),
            "headers":v.get("headers").context("header key not found")?,
            "status":v.get("status").context("status key not found")?,
        })
    } else {
        v.clone()
    };

    let final_value = v
        .pointer(info.from.as_str())
        .cloned()
        .unwrap_or_else(|| info.default.as_ref().map_or(blank, |v| json!(v)));
    let str_value = if final_value.is_string() {
        final_value.as_str().unwrap().to_string()
    } else if final_value.is_number() {
        format!("{}", final_value.as_i64().unwrap())
    } else if final_value.is_boolean() {
        format!("{}", final_value.as_bool().unwrap())
    } else if final_value.is_null() {
        String::new()
    } else if final_value.is_f64() {
        format!("{}", final_value.as_f64().unwrap())
    } else if final_value.is_u64() {
        format!("{}", final_value.as_u64().unwrap())
    } else {
        format!("{final_value}")
    };
    Ok(match &info.expr {
        Some(expr) => {
            let re = fancy_regex::Regex::new(expr).unwrap();
            let caps = re.captures(str_value.as_str()).unwrap();
            caps.map_or_else(String::new, |c| {
                let cap = if c.len() > 1 { c.get(1) } else { c.get(0) };
                cap.unwrap().as_str().to_string()
            })
        }
        None => str_value,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use maplit::hashmap;

    #[test]
    fn test_vars() {
        let mut infos: HashMap<String, VarInfo> = HashMap::new();
        infos.insert(
            "auth".into(),
            VarInfo {
                expr: Some("Auth (.*)".into()),
                kind: "regex".into(),
                from: "/body".into(),
                default: None,
            },
        );
        infos.insert(
            "person_name".into(),
            VarInfo {
                expr: Some("(.*)".into()),
                from: "/body/person/name".into(),
                kind: "json".into(),
                default: None,
            },
        );
        infos.insert(
            "token".into(),
            VarInfo {
                expr: Some("Bearer (.*)".into()),
                kind: "regex".into(),
                from: "/headers/Authentication/0".into(),
                default: Some(String::new()),
            },
        );
        infos.insert(
            "body_default".into(),
            VarInfo {
                expr: Some("(.*)".into()),
                kind: "regex".into(),
                from: "body".into(),
                default: Some("meh-body".into()),
            },
        );
        infos.insert(
            "headers_default".into(),
            VarInfo {
                expr: Some("(.*)".into()),
                kind: "regex".into(),
                from: "/headers/Foobar".into(),
                default: Some("meh-headers".into()),
            },
        );

        let resp = Response {
            request_id: None,
            body: Some(json!({"person": {"name": "joe"}}).to_string()),
            status_code: Some("200".into()),
            headers: Some(HashMap::new()),
            vars: None,
        };
        let vars = extract(&resp, &infos).unwrap();
        assert_eq!(vars.get("person_name").unwrap(), "joe", "person_name");

        let resp = Response {
            request_id: None,
            body: Some("Auth 1337".into()),
            status_code: Some("200".into()),
            headers: Some(HashMap::new()),
            vars: None,
        };
        let vars = extract(&resp, &infos).unwrap();
        assert_eq!(vars.get("auth").unwrap(), "1337", "auth");

        let headers = hashmap! {
            "Authentication".to_string() => vec!["Bearer 000foobar000".to_string()],
        };
        let resp = Response {
            request_id: None,
            body: Some("hello world".into()),
            status_code: Some("200".into()),
            headers: Some(headers),
            vars: None,
        };
        let vars = extract(&resp, &infos).unwrap();
        assert_eq!(vars.get("token").unwrap(), "000foobar000", "token");
        assert_eq!(
            vars.get("body_default").unwrap(),
            "meh-body",
            "body_default"
        );
        assert_eq!(
            vars.get("headers_default").unwrap(),
            "meh-headers",
            "headers_default"
        );
    }
}
