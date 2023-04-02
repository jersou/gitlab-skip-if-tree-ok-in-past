use crate::config::Config;
use crate::jobs::GitlabJob;
use crate::skipci_log::yellow;
use crate::verbose;
use anyhow::{anyhow, Context};
use hyper::body::HttpBody;
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;

pub async fn extract_artifacts(config: &Config, job: &GitlabJob) -> anyhow::Result<bool> {
    match job.artifacts_expire_at.clone() {
        Some(artifacts_expire_at) => {
            verbose!("Artifact expire_at : {artifacts_expire_at}");
            let tmp_dir = tempdir().context("Create temp dir error")?;
            let tmp_file = tmp_dir.path().join("artifact.zip");
            let tmp_file_path = tmp_file.to_str().context("Error path to str")?;
            let token = &config
                .ci_job_token
                .clone()
                .context("CI_JOB_TOKEN undefined")?;
            let artifact_url = format!("{}/{}/artifacts", &config.jobs_api_url, &job.id);
            verbose!("download artifact {artifact_url} to {tmp_file_path}");
            let artifact_url = format!("{artifact_url}?job_token={token}");
            let download_ok = download_file(&artifact_url, tmp_file_path).await?;
            if download_ok {
                extract_archive(tmp_file_path, &config.project_path)?;
                verbose!("extract_artifacts is OK");
            } else {
                yellow("artifact not found");
            }
            Ok(true)
        }
        None => {
            verbose!("Artifact expire_at is empty → skip its download.");
            Ok(false)
        }
    }
}

async fn download_file(url: &str, file_path: &str) -> anyhow::Result<bool> {
    verbose!("download_file to {file_path}");
    let mut file = File::create(file_path).context("Error while creating downloaded file")?;

    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_webpki_roots()
        .https_or_http()
        .enable_http1()
        .build();
    let client: hyper::Client<_, hyper::Body> = hyper::Client::builder().build(https);

    let mut response = client
        .get(url.parse().context("parse url error")?)
        .await
        .context("Error while request the file")?;
    verbose!("download_file status {:?}", response.status());
    if response.status().is_success() {
        while let Some(chunk) = response.body_mut().data().await {
            file.write_all(&chunk.context("Error while request the file")?)
                .context("Error while request the file")?;
        }
    }
    Ok(response.status().is_success())
}

fn extract_archive(archive_path: &str, output_path: &str) -> anyhow::Result<()> {
    verbose!("extract_archive {archive_path} to {output_path}");
    if archive_path.is_empty() {
        Err(anyhow!("archive_path is empty"))
    } else if output_path.is_empty() {
        Err(anyhow!("output_path is empty"))
    } else {
        let repo_zip = std::path::Path::new(archive_path);
        let zip_file = File::open(repo_zip).context("Error while opening archive file")?;
        let mut archive =
            zip::ZipArchive::new(zip_file).context("Error while decoding archive file")?;
        archive
            .extract(output_path)
            .context("Error while extracting archive file")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::artifact::{download_file, extract_archive, extract_artifacts};
    use crate::config::Config;
    use crate::jobs::{GitlabCommit, GitlabJob};
    use httptest::{matchers::*, responders::*, Expectation, Server};
    use hyper::http;
    use std::env::VarError;
    use std::fs::File;
    use std::io::{BufReader, Read};
    use tempfile::{tempdir, TempDir};
    use tokio::fs;

    #[tokio::test]
    async fn test_download_file_ok() {
        let server = Server::run();
        server.expect(
            Expectation::matching(request::method_path("GET", "/api/123/jobs/456/artifact"))
                .respond_with(status_code(200).body("abc")),
        );
        let url = server.url_str("/api/123/jobs/456/artifact");
        let tmp_dir = tempdir().unwrap();
        let tmp_path = tmp_dir.path().join("artifact.txt");
        let tmp_path_str = tmp_path.to_str().unwrap();
        let res = download_file(&url, tmp_path_str).await.unwrap();
        assert!(res);
        let content = fs::read_to_string(tmp_path_str).await.unwrap();
        assert_eq!(content, "abc");
    }

    #[tokio::test]
    async fn test_download_file_404() {
        let server = Server::run();
        server.expect(
            Expectation::matching(request::method_path("GET", "/api/123/jobs/456/artifact"))
                .respond_with(status_code(404).body("abc")),
        );
        let url = server.url_str("/api/123/jobs/456/artifact");
        let tmp_dir = tempdir().unwrap();
        let tmp_path = tmp_dir.path().join("artifact.txt");
        let tmp_path_str = tmp_path.to_str().unwrap();
        let res = download_file(&url, tmp_path_str).await.unwrap();
        assert!(!res);
    }

    #[tokio::test]
    async fn test_download_file_ko_connect_error() {
        let tmp_dir = tempdir().unwrap();
        let tmp_path = tmp_dir.path().join("artifact.txt");
        let tmp_path_str = tmp_path.to_str().unwrap();
        let res = download_file("http://localhost:45546/zzzzz/gitlab/api", tmp_path_str)
            .await
            .err()
            .unwrap();
        assert_eq!(res.to_string(), "Error while request the file");
        assert_eq!(
            res.root_cause().to_string(),
            "Connection refused (os error 111)"
        );
    }

    #[tokio::test]
    async fn test_download_file_ko() {
        let err = download_file("url", "/zzzz/zzzz/zzzzz")
            .await
            .err()
            .map(|e| format!("{e:#}"))
            .unwrap();
        assert_eq!(
            err,
            "Error while creating downloaded file: No such file or directory (os error 2)"
        );
    }
    fn prepare_tmpdir_and_server() -> (TempDir, Server, String, Config) {
        let tmp_dir = tempdir().unwrap();
        let server = Server::run();
        let url = server.url_str("/api/123/jobs");
        let config = Config {
            api_read_token: "".to_string(),
            ci_commit_ref_name: Ok("__CI_COMMIT_REF_NAME__".to_string()),
            ci_job_name: "".to_string(),
            ci_job_token: Ok("__CI_JOB_TOKEN__".to_string()),
            verbose: false,
            files_to_check: "".to_string(),
            project_path: tmp_dir.path().to_str().unwrap().to_string(),
            jobs_api_url: url.clone(),
            ci_skip_path: "".to_string(),
            page_to_fetch_max: 1,
            commit_to_check_same_ref_max: 2,
            commit_to_check_same_job_max: 3,
        };
        (tmp_dir, server, url, config)
    }

    fn add_artifact_expect(server: &Server) {
        let artifact_zip = File::open("test/artifact.zip").unwrap();
        let mut reader = BufReader::new(artifact_zip);
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer).unwrap();

        let resp = http::Response::builder()
            .status(200)
            .header("Content-type", "application/zip")
            .body(buffer)
            .unwrap();
        server.expect(
            Expectation::matching(request::method_path("GET", "/api/123/jobs/456/artifacts"))
                .respond_with(resp),
        );
    }

    #[tokio::test]
    async fn test_extract_artifacts_ok() {
        let (tmp_dir, server, _, config) = prepare_tmpdir_and_server();
        add_artifact_expect(&server);

        let job = GitlabJob {
            id: 456,
            name: "job_name".to_string(),
            job_ref: "azert".to_string(),
            web_url: "http:...".to_string(),
            status: "success".to_string(),
            artifacts_expire_at: Some("2023-03-12T19:59:33.250Z".to_string()),
            commit: GitlabCommit {
                id: "qsdfg".to_string(),
            },
        };
        assert!(extract_artifacts(&config, &job).await.unwrap());

        assert!(fs::try_exists(tmp_dir.path().join("artifact/folder1/d"))
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_extract_artifacts_404() {
        let (tmp_dir, server, _, config) = prepare_tmpdir_and_server();

        let resp = http::Response::builder().status(404).body(vec![]).unwrap();
        server.expect(
            Expectation::matching(request::method_path("GET", "/api/123/jobs/456/artifacts"))
                .respond_with(resp),
        );
        let job = GitlabJob {
            id: 456,
            name: "job_name".to_string(),
            job_ref: "azert".to_string(),
            web_url: "http:...".to_string(),
            status: "success".to_string(),
            artifacts_expire_at: Some("2023-03-12T19:59:33.250Z".to_string()),
            commit: GitlabCommit {
                id: "qsdfg".to_string(),
            },
        };
        assert!(extract_artifacts(&config, &job).await.unwrap());

        assert!(!fs::try_exists(tmp_dir.path().join("artifact/folder1/d"))
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_extract_artifacts_not_found() {
        let (_, _, _, config) = prepare_tmpdir_and_server();

        let job = GitlabJob {
            id: 456,
            name: "job_name".to_string(),
            job_ref: "azert".to_string(),
            web_url: "http:...".to_string(),
            status: "success".to_string(),
            artifacts_expire_at: None,
            commit: GitlabCommit {
                id: "qsdfg".to_string(),
            },
        };
        assert!(!extract_artifacts(&config, &job).await.unwrap());
    }

    #[tokio::test]
    async fn test_extract_artifacts_ci_job_token_undefined() {
        let config = Config {
            api_read_token: "__api_read_token__".to_string(),
            ci_commit_ref_name: Ok("__CI_COMMIT_REF_NAME__".to_string()),
            ci_job_name: "".to_string(),
            ci_job_token: Err(VarError::NotPresent),
            verbose: false,
            files_to_check: "__files_to_check__".to_string(),
            project_path: "__project_path__".to_string(),
            jobs_api_url: "__jobs_api_url__".to_string(),
            ci_skip_path: "__ci_skip_path__".to_string(),
            page_to_fetch_max: 1,
            commit_to_check_same_ref_max: 2,
            commit_to_check_same_job_max: 3,
        };
        let job = GitlabJob {
            id: 456,
            name: "job_name".to_string(),
            job_ref: "azert".to_string(),
            web_url: "http:...".to_string(),
            status: "success".to_string(),
            artifacts_expire_at: Some("2023".to_string()),
            commit: GitlabCommit {
                id: "qsdfg".to_string(),
            },
        };
        let err = extract_artifacts(&config, &job)
            .await
            .err()
            .map(|e| format!("{e}"))
            .unwrap();
        assert_eq!(err, "CI_JOB_TOKEN undefined");
    }

    #[tokio::test]
    async fn test_extract_archive() {
        let tmp_dir = tempdir().unwrap();
        extract_archive("test/artifact.zip", tmp_dir.path().to_str().unwrap()).unwrap();
        assert!(fs::try_exists(tmp_dir.path().join("artifact/folder1/d"))
            .await
            .unwrap());
    }

    #[test]
    fn test_extract_archive_archive_path_is_empty() {
        let err = extract_archive("", "out")
            .err()
            .map(|e| format!("{e}"))
            .unwrap();
        assert_eq!(err, "archive_path is empty");
    }

    #[test]
    fn test_extract_archive_output_path_is_empty() {
        let err = extract_archive("arch", "")
            .err()
            .map(|e| format!("{e}"))
            .unwrap();
        assert_eq!(err, "output_path is empty");
    }

    #[test]
    fn test_extract_archive_invalid_zip() {
        let tmp_dir = tempdir().unwrap();
        let err = extract_archive("test/gen-test-repo.sh", tmp_dir.path().to_str().unwrap())
            .err()
            .map(|e| format!("{e:#}"))
            .unwrap();
        assert_eq!(err, "Error while decoding archive file: invalid Zip archive: Could not find central directory end");
    }

    #[test]
    fn test_extract_archive_out_err() {
        let err = extract_archive("test/artifact.zip", "/zzz/zzz/zzz/zzz/zz")
            .err()
            .map(|e| format!("{e}"))
            .unwrap();
        assert_eq!(err, "Error while extracting archive file");
    }
}
