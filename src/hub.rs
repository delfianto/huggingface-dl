use crate::{
    cli::{Cli, Repo},
    error::{Error, Result},
    plan::RemoteFile,
};
use reqwest::{
    Client, StatusCode, Url,
    header::{AUTHORIZATION, LINK},
};
use serde::Deserialize;

#[derive(Clone)]
pub struct Hub {
    client: Client,
    endpoint: Url,
    token: Option<String>,
}
#[derive(Deserialize)]
struct TreeEntry {
    #[serde(rename = "type")]
    kind: String,
    path: String,
    size: Option<u64>,
}

impl Hub {
    pub fn new(cli: &Cli) -> Result<Self> {
        Ok(Self {
            client: Client::builder().build()?,
            endpoint: Url::parse(&cli.endpoint).map_err(|_| Error::Http {
                status: StatusCode::BAD_REQUEST,
                message: "invalid --endpoint URL".into(),
            })?,
            token: cli.token.clone(),
        })
    }
    fn request(&self, url: Url) -> reqwest::RequestBuilder {
        let r = self.client.get(url);
        if let Some(t) = &self.token {
            r.header(AUTHORIZATION, format!("Bearer {t}"))
        } else {
            r
        }
    }
    fn api_url(&self, repo: &Repo, revision: &str) -> Result<Url> {
        let mut u = self.endpoint.join("api/models").map_err(|_| Error::Http {
            status: StatusCode::BAD_REQUEST,
            message: "invalid endpoint URL".into(),
        })?;
        {
            let mut p = u.path_segments_mut().map_err(|_| Error::Http {
                status: StatusCode::BAD_REQUEST,
                message: "endpoint cannot be a base URL".into(),
            })?;
            p.push(&repo.owner);
            p.push(&repo.name);
            p.push("tree");
            p.push(revision);
        }
        u.query_pairs_mut()
            .append_pair("recursive", "true")
            .append_pair("expand", "false")
            .append_pair("limit", "1000");
        Ok(u)
    }
    pub async fn tree(&self, repo: &Repo, revision: &str) -> Result<Vec<RemoteFile>> {
        let mut next = self.api_url(repo, revision)?;
        let mut files = Vec::new();
        loop {
            let response = self.request(next.clone()).send().await?;
            let status = response.status();
            if !status.is_success() {
                return Err(status_error(
                    status,
                    &repo.full(),
                    response.text().await.unwrap_or_default(),
                ));
            }
            let link = response
                .headers()
                .get(LINK)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            let entries: Vec<TreeEntry> = response.json().await?;
            files.extend(
                entries
                    .into_iter()
                    .filter(|e| e.kind == "file")
                    .map(|e| RemoteFile {
                        path: e.path,
                        size: e.size.unwrap_or(0),
                    }),
            );
            let Some(link) = link.and_then(|l| next_link(&l)) else {
                break;
            };
            next = next
                .join(&link)
                .or_else(|_| Url::parse(&link))
                .map_err(|_| Error::Http {
                    status: StatusCode::BAD_REQUEST,
                    message: "invalid pagination link".into(),
                })?;
        }
        Ok(files)
    }
    pub fn file_url(&self, repo: &Repo, revision: &str, path: &str) -> Result<Url> {
        let mut u = self.endpoint.clone();
        u.set_path("");
        let mut s = u.path_segments_mut().map_err(|_| Error::Http {
            status: StatusCode::BAD_REQUEST,
            message: "endpoint cannot be a base URL".into(),
        })?;
        s.push(&repo.owner);
        s.push(&repo.name);
        s.push("resolve");
        s.push(revision);
        for part in path.split('/') {
            s.push(part);
        }
        drop(s);
        Ok(u)
    }
    pub fn get(&self, url: Url) -> reqwest::RequestBuilder {
        self.request(url)
    }
}
fn next_link(link: &str) -> Option<String> {
    link.split(',').find_map(|part| {
        if part.contains("rel=\"next\"") || part.contains("rel=next") {
            part.split('<').nth(1)?.split('>').next().map(str::to_owned)
        } else {
            None
        }
    })
}
pub fn status_error(status: StatusCode, subject: &str, message: String) -> Error {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Error::Unauthorized(subject.into()),
        StatusCode::NOT_FOUND => Error::NotFound(subject.into()),
        StatusCode::TOO_MANY_REQUESTS => Error::RateLimited,
        _ => Error::Http { status, message },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_next_link() {
        assert_eq!(
            next_link("<x?cursor=a>; rel=\"next\""),
            Some("x?cursor=a".into())
        );
    }
}
