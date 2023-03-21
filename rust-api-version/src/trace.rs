use crate::verbose;
use anyhow::Context;
use hyper::body::HttpBody;

pub const SKIP_CI_DONE_KEY: &str = "[skip-ci-done]";
pub const SKIP_CI_DONE_KEY_U8: &[u8] = SKIP_CI_DONE_KEY.as_bytes();
pub const SKIP_CI_OLDEST_ANCESTOR_KEY: &str = "[skip-ci-oldest-ancestor]";
pub const SKIP_CI_OLDEST_ANCESTOR_KEY_U8: &[u8] = SKIP_CI_OLDEST_ANCESTOR_KEY.as_bytes();

const MAX_TRACE_SIZE: usize = 100_000;

pub fn get_trace_url(jobs_api_url: &str, job_id: u32, api_read_token: &str) -> String {
    format!("{jobs_api_url}/{job_id}/trace?private_token={api_read_token}")
}

// find the [skip-ci...] data in the job log "url" (.../jobs/JOB_ID/raw)
pub async fn parse_oldest_ancestor_from_job_trace(url: &str) -> anyhow::Result<Option<String>> {
    verbose!("parse_job_trace from {url}");

    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_webpki_roots()
        .https_or_http()
        .enable_http1()
        .build();
    let client: hyper::Client<_, hyper::Body> = hyper::Client::builder().build(https);

    let mut response = client
        .get(url.parse().context("parse url error")?)
        .await
        .context("Error while request the trace")?;
    verbose!("parse_job_trace status {:?}", response.status());

    if response.status().is_success() {
        let mut chunk_tot: usize = 0;

        let ln = "\n".as_bytes();
        while let Some(Ok(chunk)) = response.body_mut().data().await {
            let index_res = chunk
                .windows(SKIP_CI_OLDEST_ANCESTOR_KEY_U8.len())
                .position(|window| window == SKIP_CI_OLDEST_ANCESTOR_KEY_U8);
            if let Some(index) = index_res {
                let slice = &chunk[index..];
                let end_pos = slice.windows(1).position(|window| window == ln);
                if let Some(end) = end_pos {
                    let start = SKIP_CI_OLDEST_ANCESTOR_KEY_U8.len() + 1;
                    let found = String::from_utf8_lossy(&slice[start..end]);
                    verbose!("parse_job_trace found={found}");
                    return Ok(Some(found.parse()?));
                }
            } else if chunk
                .windows(SKIP_CI_DONE_KEY_U8.len())
                .any(|window| window == SKIP_CI_DONE_KEY_U8)
            {
                verbose!("SKIP_CI_DONE found  → stop the parsing");
                return Ok(None);
            }

            chunk_tot += chunk.len();
            if chunk_tot > MAX_TRACE_SIZE {
                verbose!(
                    "chunk_tot({chunk_tot}) > MAX_TRACE_SIZE({MAX_TRACE_SIZE}) → stop the parsing"
                );
                return Ok(None);
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use crate::trace::{
        get_trace_url, parse_oldest_ancestor_from_job_trace, SKIP_CI_OLDEST_ANCESTOR_KEY_U8,
    };
    use httptest::matchers::request;
    use httptest::responders::status_code;
    use httptest::{Expectation, Server};
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_get_trace_url() {
        assert_eq!(
            get_trace_url(
                "http://gitlab-fake-api/api/projects/123/jobs",
                456,
                "___API_READ_TOKEN___"
            ),
            "http://gitlab-fake-api/api/projects/123/jobs/456/trace?private_token=___API_READ_TOKEN___"
        );
    }

    #[tokio::test]
    async fn test_parse_job_trace_found() {
        let raw = fs::read_to_string(Path::new(
            "test/integration/api/projects/123/jobs/12345679/raw",
        ))
        .unwrap();
        let server = Server::run();
        server.expect(
            Expectation::matching(request::method_path(
                "GET",
                "/api/projects/123/jobs/12345679/raw",
            ))
            .respond_with(status_code(200).body(raw)),
        );
        let url = server.url_str("/api/projects/123/jobs/12345679/raw");
        let res = parse_oldest_ancestor_from_job_trace(&url).await;
        assert_eq!(
            res.unwrap().unwrap(),
            "http://gitlab-fake-api/api/projects/123/jobs/11"
        );
    }

    #[tokio::test]
    async fn test_parse_job_trace_done() {
        let raw = fs::read_to_string(Path::new("test/raw_done")).unwrap();
        let server = Server::run();
        server.expect(
            Expectation::matching(request::method_path(
                "GET",
                "/api/projects/123/jobs/12345679/raw",
            ))
            .respond_with(status_code(200).body(raw)),
        );
        let url = server.url_str("/api/projects/123/jobs/12345679/raw");

        let res = parse_oldest_ancestor_from_job_trace(&url).await;
        println!("{:?}", res);
        assert!(res.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_parse_job_trace_max_size() {
        let zero: &[u8; 300000] = &[0; 300000];
        let raw = [
            zero,
            SKIP_CI_OLDEST_ANCESTOR_KEY_U8,
            "=http://gitlab-fake-api/api/projects/123/jobs/11\n\n".as_bytes(),
        ]
        .concat();

        let server = Server::run();
        server.expect(
            Expectation::matching(request::method_path(
                "GET",
                "/api/projects/123/jobs/12345679/raw",
            ))
            .respond_with(status_code(200).body(raw)),
        );
        let url = server.url_str("/api/projects/123/jobs/12345679/raw");

        let res = parse_oldest_ancestor_from_job_trace(&url).await;
        println!("{:?}", res);
        assert!(res.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_parse_job_trace_404() {
        let raw = fs::read_to_string(Path::new(
            "test/integration/api/projects/123/jobs/12345679/raw",
        ))
        .unwrap();
        let server = Server::run();
        server.expect(
            Expectation::matching(request::method_path(
                "GET",
                "/api/projects/123/jobs/12345679/raw",
            ))
            .respond_with(status_code(404).body(raw)),
        );
        let url = server.url_str("/api/projects/123/jobs/12345679/raw");

        let res = parse_oldest_ancestor_from_job_trace(&url).await;
        println!("{:?}", res);
        assert!(res.unwrap().is_none());
    }
}
