use crate::verbose;
use anyhow::Context;
use serde::Deserialize;

#[derive(Deserialize, PartialEq, Debug, Clone)]
pub struct GitlabJob {
    pub artifacts_expire_at: Option<String>,
    pub id: u32,
    pub commit: GitlabCommit,
    #[serde(alias = "ref")]
    pub job_ref: String,
    pub name: String,
    pub status: String,
    pub web_url: String,
}

#[derive(Deserialize, PartialEq, Debug, Clone)]
pub struct GitlabCommit {
    pub(crate) id: String,
}

pub(crate) fn deserialize_jobs(jobs_json: &str) -> anyhow::Result<Vec<GitlabJob>> {
    serde_json::from_str(jobs_json).context("deserialize jobs error")
}

pub async fn get_project_jobs(
    project_jobs_api_url: &str,
    page_num: u32,
    private_token: &str,
) -> anyhow::Result<Vec<GitlabJob>> {
    verbose!(
        "GET /jobs?scope=success&per_page=100&page={page_num}&private_token={}",
        if private_token.is_empty() {
            "".to_string()
        } else {
            "*".repeat(10)
        },
    );
    let url = format!(
        "{project_jobs_api_url}?scope=success&per_page=100&page={page_num}&private_token={private_token}",
    );
    let https = hyper_tls::HttpsConnector::new();
    let client = hyper::Client::builder().build::<_, hyper::Body>(https);
    let response = client
        .get(url.parse().context("parse url error")?)
        .await
        .context("Error while request the jobs")?;
    let buf = hyper::body::to_bytes(response)
        .await
        .context("Error while extract jobs body")?;
    let body_str = String::from_utf8(buf.to_vec()).context("buffer to String error")?;
    let jobs = deserialize_jobs(&body_str).context("Error while deserialize jobs")?;
    verbose!(" → {} jobs fetched", jobs.len());
    Ok(jobs)
}

#[cfg(test)]
mod tests {
    use crate::gitlab::{deserialize_jobs, get_project_jobs, GitlabCommit, GitlabJob};
    use httptest::matchers::request;
    use httptest::responders::status_code;
    use httptest::{Expectation, Server};
    #[test]
    fn test_deserialize_jobs() {
        let expected_job = GitlabJob {
            id: 123,
            name: "job_name".to_string(),
            job_ref: "azert".to_string(),
            web_url: "http...".to_string(),
            artifacts_expire_at: Some("2023...".to_string()),
            status: "success".to_string(),
            commit: GitlabCommit {
                id: "qsdfg".to_string(),
            },
        };
        let job_res = deserialize_jobs(
            r###"[
                    {
                        "artifacts_expire_at":"2023...",
                        "commit":{"id":"qsdfg"},
                        "id":123,
                        "name":"job_name",
                        "ref":"azert",
                        "status": "success",
                        "web_url":"http..."
                    }
                ]"###,
        );

        assert!(expected_job.eq(job_res.unwrap().get(0).unwrap()));
    }

    #[tokio::test]
    async fn test_get_project_jobs() {
        let server = Server::run();

        server.expect(
            Expectation::matching(request::method_path("GET", "/api/123/jobs/456")).respond_with(
                status_code(200).body(
                    r###"[
  {
    "artifacts_expire_at": "2023-03-12T19:59:33.250Z",
    "commit": {
      "id": "2121212121212121212121212121212121212121"
    },
    "id": 12345678,
    "name": "jobA",
    "ref": "branch1",
    "status": "success",
    "web_url": "https://gitlab.localhost/skip/skip-rs/-/jobs/12345678"
  },
  {
    "artifacts_expire_at": null,
    "commit": {
      "id": "3333333333333333333333333333333333333333"
    },
    "id": 12345679,
    "name": "jobA",
    "ref": "branch2",
    "status": "success",
    "web_url": "https://gitlab.localhost/skip/skip-rs/-/jobs/12345679"
  }
]"###,
                ),
            ),
        );
        let url = server.url_str("/api/123/jobs/456");
        let jobs = get_project_jobs(&url, 1, "__PRIVATE_TOKEN__")
            .await
            .unwrap();
        let expected_jobs = vec![
            GitlabJob {
                id: 12345678,
                name: "jobA".to_string(),
                job_ref: "branch1".to_string(),
                web_url: "https://gitlab.localhost/skip/skip-rs/-/jobs/12345678".to_string(),
                artifacts_expire_at: Some("2023-03-12T19:59:33.250Z".to_string()),
                status: "success".to_string(),
                commit: GitlabCommit {
                    id: "2121212121212121212121212121212121212121".to_string(),
                },
            },
            GitlabJob {
                id: 12345679,
                name: "jobA".to_string(),
                job_ref: "branch2".to_string(),
                web_url: "https://gitlab.localhost/skip/skip-rs/-/jobs/12345679".to_string(),
                artifacts_expire_at: None,
                status: "success".to_string(),
                commit: GitlabCommit {
                    id: "3333333333333333333333333333333333333333".to_string(),
                },
            },
        ];
        assert_eq!(jobs, expected_jobs);
    }

    #[tokio::test]
    async fn test_get_project_jobs_server_down() {
        let url = ("/api/123/jobs/456").to_string();
        let error = get_project_jobs(&url, 1, "")
            .await
            .err()
            .map(|e| format!("{e:#}"))
            .unwrap();
        assert_eq!(
            error,
            "Error while request the jobs: client requires absolute-form URIs"
        );
    }
}
