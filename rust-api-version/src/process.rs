use crate::artifact::extract_artifacts;
use crate::config::Config;
use crate::find_last_job_ok::find_last_job_ok;
use crate::jobs::GitlabJob;
use crate::process::ProcessResult::{JobFound, JobNotFound, Skip, SkipCiFileExists};
use crate::skip_ci_file::{check_skip_is_done, write_skip_done};
use crate::skipci_log::{green, red, yellow};
use crate::trace::{
    get_trace_url, parse_oldest_ancestor_from_job_trace, SKIP_CI_DONE_KEY,
    SKIP_CI_OLDEST_ANCESTOR_KEY,
};
use crate::verbose;
use tokio::time::Instant;

#[derive(Debug, PartialEq)]
pub enum ProcessResult {
    SkipCiFileExists(bool),
    JobFound(GitlabJob, String),
    JobNotFound,
    Skip,
}

async fn process(config: &Config) -> anyhow::Result<ProcessResult> {
    // 1. Check if the script has already been completed in the current job: check ci-skip file. If file exists, exit, else :
    let process_result = match check_skip_is_done(&config.ci_skip_path).await {
        // If file exists, exit
        Some(skip_ci) => SkipCiFileExists(skip_ci),
        None => {
            let process_result;
            if config.skip {
                process_result = Skip;
            } else {
                // 3. Get last successful jobs of the project
                let job_ok = find_last_job_ok(config).await?;

                // extract job artifact
                process_result = match job_ok {
                    Some(job) => {
                        extract_artifacts(config, &job).await?;
                        let trace_url =
                            get_trace_url(&config.jobs_api_url, job.id, &config.api_read_token);
                        let oldest_ancestor =
                            match parse_oldest_ancestor_from_job_trace(&trace_url).await {
                                Ok(Some(url)) => url,
                                _ => job.web_url.clone(),
                            };

                        // Important to keep for the futur job that will parse this trace
                        println!("{SKIP_CI_OLDEST_ANCESTOR_KEY}={oldest_ancestor}");

                        JobFound(job, oldest_ancestor)
                    }
                    None => JobNotFound,
                };
            }

            //     5.3. If the "git ls-tree" are equals, write true in ci-skip file and exit with code 0
            // 6. If no job found, write false in ci-skip file and exit with code > 0
            match process_result {
                JobFound(..) => write_skip_done(&config.ci_skip_path, true).await?,
                JobNotFound => write_skip_done(&config.ci_skip_path, false).await?,
                SkipCiFileExists(..) => {}
                Skip => {}
            };

            process_result
        }
    };
    println!("{}", SKIP_CI_DONE_KEY);
    Ok(process_result)
}

pub async fn process_with_exit_code(config_result: anyhow::Result<Config>) -> i32 {
    let start = Instant::now();

    let exit_code = match config_result {
        Ok(config) => {
            let result = process(&config).await;
            verbose!("result = {result:?}");
            match result {
                Ok(JobFound(job, oldest_ancestor)) => {
                    green(&format!("✅ tree found in job {}  ", &job.web_url));
                    green(&format!("✅ the oldest ancestor found : {oldest_ancestor}"));
                    0
                }
                Ok(JobNotFound) => {
                    yellow("❌ tree not found in last jobs of the project");
                    1
                }
                Ok(SkipCiFileExists(true)) => 0,
                Ok(SkipCiFileExists(false)) => 3,
                Err(e) => {
                    red(&format!("❌ PROCESS ERROR : \n{e:#?}"));
                    2
                }
                Ok(Skip) => {
                    yellow("Skip the SkipCi process");
                    3
                }
            }
        }
        Err(e) => {
            red(&format!("❌ CONFIG ERROR : \n{e:#?}"));
            6
        }
    };

    let duration_micro = start.elapsed().as_nanos() / 1_000;
    verbose!(
        "exit code = {exit_code} ; duration : {}.{} ms",
        duration_micro / 1_000,
        duration_micro % 1_000
    );
    exit_code
}

#[cfg(test)]
pub mod tests {
    use crate::config::Config;
    use crate::process::ProcessResult::{JobFound, JobNotFound, SkipCiFileExists};
    use crate::process::{process, process_with_exit_code};
    use anyhow::Error;
    use git2::{Oid, Repository};
    use httptest::matchers::*;
    use httptest::responders::status_code;
    use httptest::{all_of, Expectation, Server};
    use std::env::VarError;
    use std::fs;
    use std::fs::File;
    use std::path::Path;
    use std::string::String;
    use tempfile::{tempdir, TempDir};

    pub fn create_config_ok(tmp_dir: &TempDir, url: &String) -> Config {
        Config {
            api_read_token: "aaa".to_string(),
            ci_commit_ref_name: Ok("branch1".to_string()),
            ci_job_name: "jobA".to_string(),
            ci_job_token: Ok("bbb".to_string()),
            verbose: false,
            files_to_check: "root-1 Service-A/file-A1".to_string(),
            project_path: tmp_dir.path().to_str().unwrap().to_string(),
            jobs_api_url: url.clone(),
            ci_skip_path: tmp_dir.path().join("ci-skip").to_str().unwrap().to_string(),
            page_to_fetch_max: 2,
            commit_to_check_same_ref_max: 2,
            commit_to_check_same_job_max: 2,
            skip: false,
        }
    }

    fn create_config_no_url(tmp_dir: &TempDir) -> Config {
        Config {
            api_read_token: "aaa".to_string(),
            ci_commit_ref_name: Ok("branch1".to_string()),
            ci_job_name: "jobA".to_string(),
            ci_job_token: Ok("bbb".to_string()),
            verbose: false,
            files_to_check: "root-1 Service-A/file-A1 Service-A/file-A2".to_string(),
            project_path: tmp_dir.path().to_str().unwrap().to_string(),
            jobs_api_url: "____".to_string(),
            ci_skip_path: tmp_dir.path().join("skip-ci").to_str().unwrap().to_string(),
            page_to_fetch_max: 1,
            commit_to_check_same_ref_max: 10,
            commit_to_check_same_job_max: 0,
            skip: false,
        }
    }

    #[tokio::test]
    async fn test_process_with_exit_code_6() {
        let res = process_with_exit_code(Err(Error::msg("error"))).await;
        assert_eq!(res, 6);
    }

    pub fn prepare_tmp_repo() -> (TempDir, Repository) {
        let tmp_dir = tempdir().unwrap();
        let repo_zip = Path::new("test/repo.zip");
        let zip_file = File::open(repo_zip).unwrap();
        let mut archive = zip::ZipArchive::new(zip_file).unwrap();
        archive.extract(&tmp_dir).unwrap();
        let git_path = tmp_dir.path().join(".git");
        let repo = Repository::open_bare(git_path.to_str().unwrap()).unwrap();
        (tmp_dir, repo)
    }

    // https://gitlab.localhost/skip/skip-rs/-/jobs/12345678

    pub fn add_jobs_expect(server: &Server) -> String {
        server.expect(
            Expectation::matching(request::method_path("GET", "/api/123/jobs"))
                .times(1..)
                .respond_with(
                    status_code(200).body(
                        r###"[
  {
    "artifacts_expire_at": null,
    "commit": {
      "id": "3333333333333333333333333333333333333333"
    },
    "id": 1,
    "name": "jobA",
    "ref": "branch1",
    "status": "success",
    "web_url": "https://gitlab.localhost/skip/skip-rs/-/jobs/12345679"
  },
  {
    "artifacts_expire_at": null,
    "commit": {
      "id": "71caf060ef3022468ffd8b4a70e680d7fec78000"
    },
    "id": 12345678,
    "name": "jobA",
    "ref": "branch1",
    "status": "success",
    "web_url": ""###
                            .to_owned()
                            + server.url_str("/skip/skip-rs/-/jobs/12345678").as_str()
                            + r###""
  }
]"###,
                    ),
                ),
        );
        server.url_str("/api/123/jobs")
    }

    #[tokio::test]
    async fn test_process_ok_12345678() {
        let (tmp_dir, repo) = prepare_tmp_repo();
        let server = Server::run();
        let raw = fs::read_to_string(Path::new(
            "test/integration/api/projects/123/jobs/12345679/raw",
        ))
        .unwrap();
        server.expect(
            Expectation::matching(all_of!(
                request::method_path("GET", "/api/123/jobs/12345678/trace",),
                request::query(url_decoded(contains(("private_token", "aaa"))))
            ))
            .respond_with(status_code(200).body(raw)),
        );
        let url = add_jobs_expect(&server);
        // commit04
        repo.set_head_detached(Oid::from_str("5e694dadd2979a2680c98c88a2f98df9787947d2").unwrap())
            .unwrap();
        let config = create_config_ok(&tmp_dir, &url);
        match process(&config).await.unwrap() {
            JobFound(job, oldest_ancestor) => {
                assert_eq!(job.id, 12345678);
                assert_eq!(
                    oldest_ancestor,
                    "http://gitlab-fake-api/api/projects/123/jobs/11"
                );
            }
            _ => panic!(),
        };
    }

    #[tokio::test]
    async fn test_process_ok_12345678_no_trace() {
        let (tmp_dir, repo) = prepare_tmp_repo();
        let server = Server::run();
        server.expect(
            Expectation::matching(all_of!(
                request::method_path("GET", "/api/123/jobs/12345678/trace",),
                request::query(url_decoded(contains(("private_token", "aaa"))))
            ))
            .respond_with(status_code(200).body("")),
        );
        let url = add_jobs_expect(&server);
        // commit04
        repo.set_head_detached(Oid::from_str("5e694dadd2979a2680c98c88a2f98df9787947d2").unwrap())
            .unwrap();
        let config = create_config_ok(&tmp_dir, &url);
        match process(&config).await.unwrap() {
            JobFound(job, oldest_ancestor) => {
                assert_eq!(job.id, 12345678);
                assert_eq!(oldest_ancestor, job.web_url);
            }
            _ => panic!(),
        };
    }

    #[tokio::test]
    async fn test_process_ok_12345678_no_job_token() {
        let (tmp_dir, repo) = prepare_tmp_repo();
        let server = Server::run();
        let url = add_jobs_expect(&server);
        server.expect(
            Expectation::matching(all_of!(
                request::method_path("GET", "/api/123/jobs/12345678/trace",),
                request::query(url_decoded(contains(("private_token", "aaa"))))
            ))
            .respond_with(status_code(200).body("")),
        );
        // commit04
        repo.set_head_detached(Oid::from_str("5e694dadd2979a2680c98c88a2f98df9787947d2").unwrap())
            .unwrap();
        let config = Config {
            api_read_token: "aaa".to_string(),
            ci_commit_ref_name: Ok("branch1".to_string()),
            ci_job_name: "jobA".to_string(),
            ci_job_token: Err(VarError::NotPresent),
            verbose: false,
            files_to_check: "root-1 Service-A/file-A1".to_string(),
            project_path: tmp_dir.path().to_str().unwrap().to_string(),
            jobs_api_url: url.clone(),
            ci_skip_path: tmp_dir.path().join("ci-skip").to_str().unwrap().to_string(),
            page_to_fetch_max: 2,
            commit_to_check_same_ref_max: 2,
            commit_to_check_same_job_max: 2,
            skip: false,
        };
        match process(&config).await.unwrap() {
            JobFound(job, oldest_ancestor) => {
                assert_eq!(job.id, 12345678);
                assert_eq!(oldest_ancestor, job.web_url);
            }
            _ => panic!(),
        };
    }

    #[tokio::test]
    async fn test_process_with_exit_code_ok_12345678() {
        let (tmp_dir, repo) = prepare_tmp_repo();
        let server = Server::run();
        let raw = fs::read_to_string(Path::new(
            "test/integration/api/projects/123/jobs/12345679/raw",
        ))
        .unwrap();
        server.expect(
            Expectation::matching(all_of!(
                request::method_path("GET", "/api/123/jobs/12345678/trace",),
                request::query(url_decoded(contains(("private_token", "aaa"))))
            ))
            .respond_with(status_code(200).body(raw)),
        );
        let url = add_jobs_expect(&server);

        // commit04
        repo.set_head_detached(Oid::from_str("5e694dadd2979a2680c98c88a2f98df9787947d2").unwrap())
            .unwrap();

        let config = create_config_ok(&tmp_dir, &url);
        let res = process_with_exit_code(Ok(config.clone())).await;
        assert_eq!(res, 0);
        let res = process_with_exit_code(Ok(config)).await;
        assert_eq!(res, 0);
    }

    #[tokio::test]
    async fn test_process_with_exit_code_job_not_found() {
        let (tmp_dir, _) = prepare_tmp_repo();
        let server = Server::run();
        let url = add_jobs_expect(&server);
        let config = Config {
            api_read_token: "aaa".to_string(),
            ci_commit_ref_name: Ok("branch1".to_string()),
            ci_job_name: "jobA".to_string(),
            ci_job_token: Ok("bbb".to_string()),
            verbose: false,
            files_to_check: "root-2 Service-A/file-A1 Service-A/file-A2".to_string(),
            project_path: tmp_dir.path().to_str().unwrap().to_string(),
            jobs_api_url: url.clone(),
            ci_skip_path: tmp_dir.path().join("skip-ci").to_str().unwrap().to_string(),
            page_to_fetch_max: 1,
            commit_to_check_same_ref_max: 10,
            commit_to_check_same_job_max: 0,
            skip: false,
        };
        let res = process_with_exit_code(Ok(config.clone())).await;
        assert_eq!(res, 1);
    }

    #[tokio::test]
    async fn test_process_with_exit_code_skip() {
        let (tmp_dir, _) = prepare_tmp_repo();
        let config = Config {
            api_read_token: "aaa".to_string(),
            ci_commit_ref_name: Ok("branch1".to_string()),
            ci_job_name: "jobA".to_string(),
            ci_job_token: Ok("bbb".to_string()),
            verbose: false,
            files_to_check: "root-2 Service-A/file-A1 Service-A/file-A2".to_string(),
            project_path: tmp_dir.path().to_str().unwrap().to_string(),
            jobs_api_url: String::from("none"),
            ci_skip_path: tmp_dir.path().join("skip-ci").to_str().unwrap().to_string(),
            page_to_fetch_max: 1,
            commit_to_check_same_ref_max: 10,
            commit_to_check_same_job_max: 0,
            skip: true,
        };
        let res = process_with_exit_code(Ok(config.clone())).await;
        assert_eq!(res, 3);
    }

    #[tokio::test]
    async fn test_process_none_job_d() {
        let (tmp_dir, _) = prepare_tmp_repo();
        let server = Server::run();
        let url = add_jobs_expect(&server);
        let config = Config {
            api_read_token: "aaa".to_string(),
            ci_commit_ref_name: Ok("branch1".to_string()),
            ci_job_name: "job--D".to_string(),
            ci_job_token: Ok("bbb".to_string()),
            verbose: false,
            files_to_check: "root-1 Service-A/file-A1".to_string(),
            project_path: tmp_dir.path().to_str().unwrap().to_string(),
            jobs_api_url: url.clone(),
            ci_skip_path: tmp_dir.path().join("ci-skip").to_str().unwrap().to_string(),
            page_to_fetch_max: 2,
            commit_to_check_same_ref_max: 2,
            commit_to_check_same_job_max: 2,
            skip: false,
        };
        let res = process(&config).await.unwrap();
        assert!(matches!(res, JobNotFound));
    }

    #[tokio::test]
    async fn test_process_none_job_a() {
        let (tmp_dir, _) = prepare_tmp_repo();
        let server = Server::run();
        let url = add_jobs_expect(&server);
        let config = Config {
            api_read_token: "aaa".to_string(),
            ci_commit_ref_name: Ok("branch1".to_string()),
            ci_job_name: "jobA".to_string(),
            ci_job_token: Ok("bbb".to_string()),
            verbose: false,
            files_to_check: "root-2 Service-A/file-A1 Service-A/file-A2".to_string(),
            project_path: tmp_dir.path().to_str().unwrap().to_string(),
            jobs_api_url: url.clone(),
            ci_skip_path: tmp_dir.path().join("ci-skip").to_str().unwrap().to_string(),
            page_to_fetch_max: 1,
            commit_to_check_same_ref_max: 0,
            commit_to_check_same_job_max: 1,
            skip: false,
        };
        let res = process(&config).await.unwrap();
        assert!(matches!(res, JobNotFound));
    }

    #[tokio::test]
    async fn test_process_none_job_a2() {
        let (tmp_dir, _) = prepare_tmp_repo();
        let server = Server::run();
        let url = add_jobs_expect(&server);
        let config = Config {
            api_read_token: "aaa".to_string(),
            ci_commit_ref_name: Ok("branch1".to_string()),
            ci_job_name: "jobA".to_string(),
            ci_job_token: Ok("bbb".to_string()),
            verbose: false,
            files_to_check: "root-2 Service-A/file-A1 Service-A/file-A2".to_string(),
            project_path: tmp_dir.path().to_str().unwrap().to_string(),
            jobs_api_url: url.clone(),
            ci_skip_path: tmp_dir.path().join("skip-ci").to_str().unwrap().to_string(),
            page_to_fetch_max: 1,
            commit_to_check_same_ref_max: 10,
            commit_to_check_same_job_max: 0,
            skip: false,
        };
        let res = process(&config).await.unwrap();
        assert!(matches!(res, JobNotFound));
    }

    #[tokio::test]
    async fn test_process_with_exit_code_2() {
        let (tmp_dir, _) = prepare_tmp_repo();
        let config = create_config_no_url(&tmp_dir);
        let res = process_with_exit_code(Ok(config)).await;
        assert_eq!(res, 2);
    }

    #[test]
    fn test_process_none_skip_ci_verbose() {
        temp_env::with_var("SKIP_CI_VERBOSE", None::<String>, || {
            let (tmp_dir, _) = prepare_tmp_repo();
            let config = Config {
                api_read_token: "aaa".to_string(),
                ci_commit_ref_name: Ok("branch1".to_string()),
                ci_job_name: "jobA".to_string(),
                ci_job_token: Ok("bbb".to_string()),
                verbose: false,
                files_to_check: "root-1 Service-A/file-A1 Service-A/file-A2".to_string(),
                project_path: tmp_dir.path().to_str().unwrap().to_string(),
                jobs_api_url: "____".to_string(),
                ci_skip_path: tmp_dir.path().join("skip-ci").to_str().unwrap().to_string(),
                page_to_fetch_max: 1,
                commit_to_check_same_ref_max: 10,
                commit_to_check_same_job_max: 0,
                skip: false,
            };
            let _res = process_with_exit_code(Ok(config));
        });
    }

    #[tokio::test]
    async fn test_process_none_skip_ci() {
        let (tmp_dir, _) = prepare_tmp_repo();
        let config = create_config_no_url(&tmp_dir);
        let path = tmp_dir.path().join("skip-ci");
        fs::write(&path, "true").unwrap();
        let res = process(&config).await.unwrap();
        assert!(matches!(res, SkipCiFileExists(true)));
    }
    #[tokio::test]
    async fn test_process_none_skip_ci_err() {
        let (tmp_dir, _) = prepare_tmp_repo();
        let config = Config {
            api_read_token: "aaa".to_string(),
            ci_commit_ref_name: Ok("branch1".to_string()),
            ci_job_name: "jobA".to_string(),
            ci_job_token: Ok("bbb".to_string()),
            verbose: false,
            files_to_check: "root-1 Service-A/file-A1 Service-A/file-A2".to_string(),
            project_path: tmp_dir.path().to_str().unwrap().to_string(),
            jobs_api_url: "____".to_string(),
            ci_skip_path: "/zzzz/z/zzzzz/zzz".to_string(),
            page_to_fetch_max: 0,
            commit_to_check_same_ref_max: 0,
            commit_to_check_same_job_max: 0,
            skip: false,
        };
        let path = tmp_dir.path().join("skip-ci");
        fs::write(&path, "true").unwrap();
        let res = process(&config).await;
        assert_eq!(
            res.err().map(|e| format!("{e:#}")).unwrap(),
            "write skip done error: No such file or directory (os error 2)"
        );
    }

    #[tokio::test]
    async fn test_process_none_skip_ci_false() {
        let (tmp_dir, _) = prepare_tmp_repo();
        let config = create_config_no_url(&tmp_dir);
        let path = tmp_dir.path().join("skip-ci");
        fs::write(&path, "false").unwrap();
        let res = process(&config).await.unwrap();
        assert!(matches!(res, SkipCiFileExists(false)));
        let res = process_with_exit_code(Ok(config)).await;
        assert_eq!(res, 3);
    }
}
