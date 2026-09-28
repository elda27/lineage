//! GitHub API boundary. Credentials never enter core or persisted requests.
use crate::{domain::ports::CredentialStore, infra::credentials::OsCredentialStore};
use anyhow::{Context, Result, ensure};
use reqwest::{Method, blocking::Client};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::time::Duration;

pub struct GitHub {
    client: Client,
    base: String,
    token: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: u64,
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub html_url: String,
    pub updated_at: String,
    #[serde(default)]
    pub pull_request: Option<Value>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IssueInput {
    pub title: Option<String>,
    pub body: Option<String>,
    pub state: Option<String>,
}
impl GitHub {
    pub fn authenticated() -> Result<Self> {
        let token = OsCredentialStore
            .secret("github")?
            .context("register GitHub credentials with credential set --provider github")?;
        Self::new("https://api.github.com".into(), token)
    }
    fn new(base: String, token: String) -> Result<Self> {
        ensure!(!token.trim().is_empty(), "GitHub token is empty");
        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(60))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            base,
            token,
        })
    }
    pub fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<T> {
        let mut request = self
            .client
            .request(method, format!("{}{path}", self.base))
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "lineage-runner");
        if let Some(body) = body {
            request = request.json(body);
        }
        // Never automatically retry writes: timeout may mean the remote write succeeded.
        let response = request.send().context("GitHub request failed; write outcome may be unknown, inspect remote state before retry")?;
        ensure!(
            response.status().is_success(),
            "GitHub API returned HTTP {}",
            response.status()
        );
        response.json().context("invalid GitHub response")
    }
    pub fn issue(&self, repo: &str, number: u64) -> Result<Issue> {
        validate_repo(repo)?;
        ensure!(number > 0, "issue number must be positive");
        let issue: Issue =
            self.request(Method::GET, &format!("/repos/{repo}/issues/{number}"), None)?;
        ensure!(
            issue.pull_request.is_none(),
            "pull requests are not supported as issues"
        );
        Ok(issue)
    }
    pub fn write_issue(&self, repo: &str, number: Option<u64>, input: IssueInput) -> Result<Issue> {
        validate_repo(repo)?;
        if let Some(title) = &input.title {
            ensure!(!title.trim().is_empty(), "title is empty");
        }
        if let Some(state) = &input.state {
            ensure!(
                state == "open" || state == "closed",
                "state must be open or closed"
            );
        }
        if let Some(n) = number {
            self.issue(repo, n)?;
        } else {
            ensure!(
                input.title.is_some() && input.state.is_none(),
                "new issue requires title and no state"
            );
        }
        let mut body = serde_json::to_value(input)?;
        body.as_object_mut().unwrap().retain(|_, v| !v.is_null());
        ensure!(!body.as_object().unwrap().is_empty(), "empty issue update");
        self.request(
            if number.is_some() {
                Method::PATCH
            } else {
                Method::POST
            },
            &format!(
                "/repos/{repo}/issues{}",
                number.map(|n| format!("/{n}")).unwrap_or_default()
            ),
            Some(&body),
        )
    }
}
pub fn validate_repo(repo: &str) -> Result<()> {
    let parts: Vec<_> = repo.split('/').collect();
    ensure!(
        parts.len() == 2
            && parts.iter().all(|p| !p.is_empty()
                && *p != "."
                && *p != ".."
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))),
        "repository must be owner/name"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };
    fn mock(status: &str, body: &str) -> (GitHub, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let api = GitHub::new(
            format!("http://{}", listener.local_addr().unwrap()),
            "test-secret".into(),
        )
        .unwrap();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let handle = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut chunk = [0; 4096];
                let n = socket.read(&mut chunk).unwrap();
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..n]);
                if let Some(index) = bytes.windows(4).position(|x| x == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..index]).to_ascii_lowercase();
                    let size = header
                        .lines()
                        .find_map(|v| v.strip_prefix("content-length: "))
                        .map(|v| v.parse::<usize>().unwrap())
                        .unwrap_or(0);
                    if bytes.len() >= index + 4 + size {
                        break;
                    }
                }
            }
            socket.write_all(response.as_bytes()).unwrap();
            String::from_utf8(bytes).unwrap()
        });
        (api, handle)
    }
    #[test]
    fn issue_creation_sends_authenticated_json_without_null_fields() {
        let (api, handle) = mock(
            "201 Created",
            r#"{"id":1,"number":2,"title":"title","body":"body","html_url":"https://github.com/o/r/issues/2","updated_at":"now"}"#,
        );
        let issue = api
            .write_issue(
                "o/r",
                None,
                IssueInput {
                    title: Some("title".into()),
                    body: Some("body".into()),
                    state: None,
                },
            )
            .unwrap();
        assert_eq!(issue.number, 2);
        let request = handle.join().unwrap();
        assert!(request.starts_with("POST /repos/o/r/issues "));
        assert!(
            request
                .to_lowercase()
                .contains("authorization: bearer test-secret")
        );
        let body: Value = serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(body, serde_json::json!({"title":"title","body":"body"}));
    }
    #[test]
    fn permissions_error_does_not_echo_remote_body_or_secret() {
        let (api, handle) = mock("403 Forbidden", r#"{"message":"test-secret"}"#);
        let error = api.issue("o/r", 1).unwrap_err().to_string();
        handle.join().unwrap();
        assert!(error.contains("403"));
        assert!(!error.contains("test-secret"));
    }
    #[test]
    fn repository_is_not_an_arbitrary_url_or_path() {
        for repo in [
            "o/r/../../secrets",
            "https://example.com",
            "o/r?x=1",
            "../r",
            "o/",
        ] {
            assert!(validate_repo(repo).is_err());
        }
    }
}
