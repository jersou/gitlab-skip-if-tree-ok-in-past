use crate::artifact::extract_artifacts;
use crate::config::Config;
use crate::git::get_tree_of_paths;
use crate::gitlab::{get_project_jobs, GitlabJob};
use crate::log::{green, red, yellow};
use crate::trace::{
    get_trace_url, parse_oldest_ancestor_from_job_trace, SKIP_CI_DONE_KEY,
    SKIP_CI_OLDEST_ANCESTOR_KEY,
};
use crate::verbose;
use anyhow::Context;
use git2::Repository;
use std::path::Path;
use tokio::fs;
use tokio::time::Instant;

// check if the skip is already done, and return the result from the skip-ci file
async fn check_skip_is_done(path_str: &str) -> Option<bool> {
    let path = Path::new(path_str);
    if fs::try_exists(path).await.unwrap_or(false) {
        let content = fs::read_to_string(path).await;
        match content {
            Ok(content_str) => {
                let skip_is_ok = content_str.eq("true");
                verbose!("skip-ci file exists with this content : {}", content_str);
                Some(skip_is_ok)
            }
            Err(_) => {
                verbose!("skip-ci file read error");
                None
            }
        }
    } else {
        verbose!("skip-ci file doesn't exists");
        None
    }
}

// write the result to the skip-ci file
async fn write_skip_done(path_str: &str, result: bool) -> anyhow::Result<()> {
    verbose!("write {result} to skip-ci file {path_str}");
    let path = Path::new(path_str);
    let result_str = if result {
        "true".as_bytes()
    } else {
        "false".as_bytes()
    };
    fs::write(path, result_str)
        .await
        .context("write skip done error")?;
    Ok(())
}

async fn find_last_job_ok(config: &Config) -> anyhow::Result<Option<GitlabJob>> {
    let git_path = Path::new(&config.project_path).join(".git");
    let repo = Repository::open_bare(git_path.to_str().context("Git Repo path error")?)
        .context("Git Repo error")?;
    let head = repo
        .refname_to_id("HEAD")
        .context("Head retrieving error")?;
    verbose!("head = {head}");
    let skip_files_paths = config
        .files_to_check
        .split(' ')
        .map(Path::new)
        .collect::<Vec<&Path>>();

    // 2. Get the "git ls-tree" of the tree "$SKIP_IF_TREE_OK_IN_PAST" of the current HEAD
    let tree_of_head = get_tree_of_paths(&repo, head.to_string().as_str(), &skip_files_paths)?;

    let mut commit_to_check_same_ref = 0;
    let mut commit_to_check_same_job = 0;

    for page_num in 1..=config.page_to_fetch_max {
        let jobs = get_project_jobs(&config.jobs_api_url, page_num, &config.api_read_token).await?;
        let job_found = jobs
            .iter()
            // 4. Filter jobs : keep current job only
            .filter(|job| job.name == config.ci_job_name && job.status == "success")
            // 5. For each job :
            .find(|job| {
                commit_to_check_same_job += 1;
                if let Ok(ci_commit_ref_name) = config.ci_commit_ref_name.clone() {
                    if job.job_ref.eq(&ci_commit_ref_name) {
                        commit_to_check_same_ref += 1;
                    }
                }
                //     5.1. Get the "git ls-tree" of the tree "$SKIP_IF_TREE_OK_IN_PAST"
                let tree = get_tree_of_paths(&repo, &job.commit.id, &skip_files_paths);
                //     5.2. Check if this "git ls-tree" equals the current HEAD "git ls-tree" (see 2.)
                match tree {
                    Ok(tree_content) => tree_content.eq(&tree_of_head),
                    Err(_) => false,
                }
            });
        verbose!(
            "{commit_to_check_same_job} jobs checked, {commit_to_check_same_ref} with the same ref"
        );

        match job_found {
            Some(job) => {
                verbose!("job found in page {page_num} !");
                return Ok(Some(job.clone()));
            }
            None => {
                verbose!("job not found in page {page_num}");
                if commit_to_check_same_ref > config.commit_to_check_same_ref_max {
                    verbose!(
                        "commit_to_check_same_ref_max: {commit_to_check_same_ref} > {}",
                        config.commit_to_check_same_ref_max
                    );
                    return Ok(None);
                }
                if commit_to_check_same_job > config.commit_to_check_same_job_max {
                    verbose!(
                        "commit_to_check_same_job_max: {commit_to_check_same_job} > {}",
                        config.commit_to_check_same_job_max
                    );
                    return Ok(None);
                }
            }
        };
    }
    verbose!("job not found ! {commit_to_check_same_job} jobs checked, {commit_to_check_same_ref} with the same ref");
    Ok(None)
}

#[derive(Debug)]
pub struct ProcessResult {
    pub skip_ci: bool,
    pub found_job: Option<GitlabJob>,
    pub oldest_ancestor: Option<String>,
}

async fn process(config: &Config) -> anyhow::Result<ProcessResult> {
    // 1. Check if the script has already been completed : check ci-skip file. If file exists, exit, else :
    let is_skip_done = check_skip_is_done(&config.ci_skip_path).await;
    // If file exists, exit
    let result = match is_skip_done {
        Some(skip_ci) => ProcessResult {
            skip_ci,
            found_job: None,
            oldest_ancestor: None,
        },
        None => {
            // 3. Get last successful jobs of the project
            let job_ok = find_last_job_ok(config).await?;

            //     5.3. If the "git ls-tree" are equals, write true in ci-skip file and exit with code 0
            // 6. If no job found, write false in ci-skip file and exit with code > 0

            // extract job artifact
            let process_result: ProcessResult = match job_ok {
                Some(job) => {
                    extract_artifacts(config, &job).await?;
                    let trace_url =
                        get_trace_url(&config.jobs_api_url, job.id, &config.api_read_token);
                    let oldest_ancestor =
                        match parse_oldest_ancestor_from_job_trace(&trace_url).await {
                            Ok(Some(url)) => url,
                            _ => job.web_url.clone(),
                        };

                    // Important to keep for the next job that will parse this trace
                    println!("{SKIP_CI_OLDEST_ANCESTOR_KEY}={oldest_ancestor}");
                    ProcessResult {
                        skip_ci: true,
                        found_job: Some(job),
                        oldest_ancestor: Some(oldest_ancestor),
                    }
                }
                None => ProcessResult {
                    skip_ci: false,
                    found_job: None,
                    oldest_ancestor: None,
                },
            };

            write_skip_done(&config.ci_skip_path, process_result.skip_ci).await?;

            process_result
        }
    };
    println!("{}", SKIP_CI_DONE_KEY);
    Ok(result)
}

pub async fn process_with_exit_code(config_result: anyhow::Result<Config>) -> i32 {
    let start = Instant::now();

    let exit_code = match config_result {
        Ok(config) => {
            let result = process(&config).await;
            verbose!("result = {result:?}");
            match result {
                Err(e) => {
                    red(&format!("❌ PROCESS ERROR : \n{e:#?}"));
                    2
                }
                Ok(ProcessResult {
                    skip_ci: true,
                    found_job: None,
                    ..
                }) => 0,
                Ok(ProcessResult {
                    skip_ci: true,
                    found_job: Some(job),
                    oldest_ancestor,
                }) => {
                    green(&format!("✅ tree found in job {}  ", &job.web_url));
                    green(&format!(
                        "✅ the oldest ancestor found : {}  ",
                        oldest_ancestor.unwrap_or_default()
                    ));
                    0
                }
                Ok(ProcessResult { skip_ci: false, .. }) => {
                    yellow("❌ tree not found in last jobs of the project");
                    1
                }
            }
        }
        Err(e) => {
            red(&format!("❌ CONFIG ERROR : \n{e:#?}"));
            6
        }
    };

    let duration_micro = start.elapsed().as_nanos() / 1000;

    verbose!(
        "exit code = {exit_code} ; duration : {}.{} ms",
        duration_micro / 1000,
        duration_micro % 1000
    );
    exit_code
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::process::{
        check_skip_is_done, find_last_job_ok, process, process_with_exit_code, write_skip_done,
    };
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
    use tempdir::TempDir;

    #[tokio::test]
    async fn test_check_skip_is_done() {
        let tmp_dir = TempDir::new("test_write_skip_done").unwrap();
        let path = tmp_dir.path().join("skip-ci-done-ok");
        fs::write(&path, "true").unwrap();
        let res = check_skip_is_done(&path.to_str().unwrap()).await;
        assert_eq!(res, Some(true));
        let path = tmp_dir.path().join("skip-ci-done-ko");
        fs::write(&path, "false").unwrap();
        let res = check_skip_is_done(&path.to_str().unwrap()).await;
        assert_eq!(res, Some(false));
        let path = tmp_dir.path().join("skip-ci-done");
        fs::write(&path, "").unwrap();
        let res = check_skip_is_done(&path.to_str().unwrap()).await;
        assert_eq!(res, Some(false));
        let path = tmp_dir.path().join("skip-ci-missing");
        let res = check_skip_is_done(&path.to_str().unwrap()).await;
        assert_eq!(res, None);
        let res = check_skip_is_done("test/artifact.zip").await;
        assert_eq!(res, None);
    }

    #[tokio::test]
    async fn test_write_skip_done() {
        let tmp_dir = TempDir::new("test_write_skip_done").unwrap();
        let path = tmp_dir.path().join("skip-ci-done-ok");
        write_skip_done(&path.to_str().unwrap(), true)
            .await
            .unwrap();
        assert!(path.try_exists().unwrap());
        let content = fs::read_to_string(path).unwrap();
        assert_eq!(content, "true");
        let path = tmp_dir.path().join("skip-ci-done-ko");
        write_skip_done(path.to_str().unwrap(), false)
            .await
            .unwrap();
        assert!(path.try_exists().unwrap());
        let content = fs::read_to_string(path).unwrap();
        assert_eq!(content, "false");
        let err = write_skip_done("/zzzz/zzzz/zzzzz", false)
            .await
            .err()
            .map(|e| format!("{e:#}"))
            .unwrap();
        assert_eq!(
            err,
            "write skip done error: No such file or directory (os error 2)"
        );
    }

    fn create_config_ok(tmp_dir: &TempDir, url: &String) -> Config {
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
        }
    }

    #[tokio::test]
    async fn test_find_last_job_ok() {
        let (tmp_dir, repo) = prepare_tmp_repo();
        let server = Server::run();
        let url = add_jobs_expect(&server);

        // commit04
        repo.set_head_detached(Oid::from_str("5e694dadd2979a2680c98c88a2f98df9787947d2").unwrap())
            .unwrap();

        let config = create_config_ok(&tmp_dir, &url);
        let res = find_last_job_ok(&config).await;
        assert_eq!(res.unwrap().unwrap().id, 12345678);
    }

    #[tokio::test]
    async fn test_process_with_exit_code_6() {
        let res = process_with_exit_code(Err(Error::msg("error"))).await;
        assert_eq!(res, 6);
    }

    fn prepare_tmp_repo() -> (TempDir, Repository) {
        let tmp_dir = TempDir::new("test_get_tree_of_paths").unwrap();
        let repo_zip = Path::new("test/repo.zip");
        let zip_file = File::open(repo_zip).unwrap();
        let mut archive = zip::ZipArchive::new(zip_file).unwrap();
        archive.extract(&tmp_dir).unwrap();
        let git_path = tmp_dir.path().join(".git");
        let repo = Repository::open_bare(git_path.to_str().unwrap()).unwrap();
        (tmp_dir, repo)
    }

    // https://gitlab.localhost/skip/skip-rs/-/jobs/12345678

    fn add_jobs_expect(server: &Server) -> String {
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
        let res = process(&config).await.unwrap();
        assert_eq!(res.found_job.unwrap().id, 12345678);
        assert_eq!(
            res.oldest_ancestor.unwrap(),
            "http://gitlab-fake-api/api/projects/123/jobs/11"
        );
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
        let res = process(&config).await.unwrap();
        let job = res.found_job.unwrap();
        assert_eq!(job.id, 12345678);
        assert_eq!(res.oldest_ancestor.unwrap(), job.web_url);
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
        };
        let res = process(&config).await.unwrap();
        let job = res.found_job.unwrap();
        assert_eq!(job.id, 12345678);
        assert_eq!(res.oldest_ancestor.unwrap(), job.web_url);
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
        };
        let res = process(&config).await;
        assert!(res.unwrap().found_job.is_none());
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
        };
        let res = process(&config).await.unwrap();
        assert!(res.found_job.is_none());
        assert!(!res.skip_ci);
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
        };
        let res = process(&config).await.unwrap();
        assert!(res.found_job.is_none());
        assert!(!res.skip_ci);
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
        assert!(res.found_job.is_none());
        assert!(res.skip_ci);
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
        assert!(res.found_job.is_none());
        assert!(!res.skip_ci);
        let res = process_with_exit_code(Ok(config)).await;
        assert_eq!(res, 1);
    }

    #[tokio::test]
    async fn test_find_last_job_ok_git_ko() {
        let tmp_dir = TempDir::new("test_get_tree_of_paths").unwrap();
        let repo_zip = Path::new("test/repo.zip");
        let zip_file = File::open(repo_zip).unwrap();
        let mut archive = zip::ZipArchive::new(zip_file).unwrap();
        archive.extract(&tmp_dir).unwrap();
        fs::remove_file(tmp_dir.path().join(".git/HEAD")).unwrap();
        let config = Config {
            api_read_token: "aaa".to_string(),
            ci_commit_ref_name: Ok("branch1".to_string()),
            ci_job_name: "jobA".to_string(),
            ci_job_token: Ok("bbb".to_string()),
            verbose: false,
            files_to_check: "root-1 Service-A/file-A1".to_string(),
            project_path: tmp_dir.path().to_str().unwrap().to_string(),
            jobs_api_url: "____".parse().unwrap(),
            ci_skip_path: tmp_dir.path().join("ci-skip").to_str().unwrap().to_string(),
            page_to_fetch_max: 2,
            commit_to_check_same_ref_max: 2,
            commit_to_check_same_job_max: 2,
        };
        let res = find_last_job_ok(&config).await;
        assert_eq!(res.err().map(|e|format!("{e:#}")).unwrap(),
                   format!("Git Repo error: path is not a repository: {}/.git; class=Repository (6); code=NotFound (-3)", tmp_dir.path().to_str().unwrap()));
    }
}
