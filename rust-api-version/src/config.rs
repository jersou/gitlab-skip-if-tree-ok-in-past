use crate::verbose;
use anyhow::Context;
use std::env;
use std::env::VarError;
use std::fmt::{Display, Formatter};
use std::path::Path;

const DEFAULT_PAGE_TO_FETCH_MAX: u32 = 5;
const DEFAULT_COMMIT_TO_CHECK_SAME_REF_MAX: u32 = 3;
const DEFAULT_COMMIT_TO_CHECK_SAME_JOB_MAX: u32 = 100;

#[derive(Clone)]
pub struct Config {
    // API_READ_TOKEN
    pub api_read_token: String,
    // CI_COMMIT_REF_NAME
    pub ci_commit_ref_name: Result<String, VarError>,
    // CI_JOB_NAME
    pub ci_job_name: String,
    // CI_JOB_TOKEN
    pub ci_job_token: Result<String, VarError>,
    // SKIP_CI_VERBOSE
    pub verbose: bool,
    // SKIP_IF_TREE_OK_IN_PAST
    pub files_to_check: String,
    pub project_path: String,
    pub jobs_api_url: String,
    pub ci_skip_path: String,
    pub page_to_fetch_max: u32,
    pub commit_to_check_same_ref_max: u32,
    pub commit_to_check_same_job_max: u32,
    pub skip: bool,
}
impl Display for Config {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r###"
  project_path                 = {}
  ci_commit_ref_name           = {}
  ci_job_name                  = {}
  verbose                      = {}
  files_to_check               = {}
  project_path                 = {}
  jobs_api_url                 = {}
  ci_skip_path                 = {}
  api_read_token               = {}
  ci_job_token                 = {}
  page_to_fetch_max            = {}
  commit_to_check_same_ref_max = {}
  commit_to_check_same_job_max = {}"###,
            self.project_path.as_str(),
            self.ci_commit_ref_name.clone().unwrap_or_default(),
            self.ci_job_name,
            self.verbose,
            self.files_to_check,
            self.project_path,
            self.jobs_api_url,
            self.ci_skip_path,
            self.api_read_token,
            self.ci_job_token.clone().unwrap_or_default(),
            self.page_to_fetch_max,
            self.commit_to_check_same_ref_max,
            self.commit_to_check_same_job_max,
        )
    }
}

/// CI_PROJECT_DIR: The full path the repository is cloned to, and where the job runs from.
/// If the GitLab Runner CI_BUILDS_DIR parameter is set, this variable is set relative to the
/// value of builds_dir.
pub fn get_project_path(ci_builds_dir: &str, ci_project_dir: &str) -> anyhow::Result<String> {
    let project_path = if ci_project_dir.starts_with(ci_builds_dir) {
        String::from(ci_project_dir) + "/"
    } else {
        let build_dir_parent = Path::new(ci_builds_dir)
            .parent()
            .context("no ci_builds_dir parent ?")?;
        let full_path = build_dir_parent.join(Path::new(&ci_project_dir[1..]));
        full_path.to_str().unwrap_or_default().to_string() + "/"
    };
    verbose!("project_path={project_path}");
    Ok(project_path)
}

pub fn config_from_env() -> anyhow::Result<Config> {
    let ci_api_v4_url = env::var("CI_API_V4_URL").context("CI_API_V4_URL is not defined")?;
    let ci_builds_dir = env::var("CI_BUILDS_DIR").unwrap_or_default();
    let ci_project_dir = env::var("CI_PROJECT_DIR").context("CI_PROJECT_DIR is not defined")?;
    let ci_project_id = env::var("CI_PROJECT_ID").context("CI_PROJECT_ID is not defined")?;
    let ci_job_id = env::var("CI_JOB_ID").context("CI_JOB_ID is not defined")?;

    let project_path =
        get_project_path(&ci_builds_dir, &ci_project_dir).context("get_project_path error:")?;
    let jobs_api_url = format!("{ci_api_v4_url}/projects/{ci_project_id}/jobs");
    let ci_skip_path = format!("{project_path}ci-skip-{ci_project_id}-{ci_job_id}");

    let page_to_fetch_max = match env::var("SKIP_CI_PAGE_TO_FETCH_MAX") {
        Ok(s) => s.parse::<u32>().unwrap_or(DEFAULT_PAGE_TO_FETCH_MAX),
        _ => DEFAULT_PAGE_TO_FETCH_MAX,
    };

    let commit_to_check_same_ref_max = match env::var("SKIP_CI_COMMIT_TO_CHECK_SAME_REF_MAX") {
        Ok(s) => s
            .parse::<u32>()
            .unwrap_or(DEFAULT_COMMIT_TO_CHECK_SAME_REF_MAX),
        _ => DEFAULT_COMMIT_TO_CHECK_SAME_REF_MAX,
    };

    let commit_to_check_same_job_max = match env::var("SKIP_CI_COMMIT_TO_CHECK_SAME_JOB_MAX") {
        Ok(s) => s
            .parse::<u32>()
            .unwrap_or(DEFAULT_COMMIT_TO_CHECK_SAME_JOB_MAX),
        _ => DEFAULT_COMMIT_TO_CHECK_SAME_JOB_MAX,
    };

    let config = Config {
        api_read_token: env::var("API_READ_TOKEN").context("API_READ_TOKEN is not defined")?,
        ci_commit_ref_name: env::var("CI_COMMIT_REF_NAME"),
        ci_job_name: env::var("CI_JOB_NAME").context("CI_JOB_NAME is not defined")?,
        ci_job_token: env::var("CI_JOB_TOKEN"),
        verbose: env::var("SKIP_CI_VERBOSE")
            .map(|v| v == "true")
            .unwrap_or(false),
        files_to_check: env::var("SKIP_IF_TREE_OK_IN_PAST")
            .context("SKIP_IF_TREE_OK_IN_PAST is not defined")?,
        project_path,
        jobs_api_url,
        ci_skip_path,
        page_to_fetch_max,
        commit_to_check_same_ref_max,
        commit_to_check_same_job_max,
        skip: env::var("SKIP_SKIP_CI")
            .map(|v| v == "true")
            .unwrap_or_default(),
    };
    verbose!("config = {config}");
    Ok(config)
}

///
///
///

#[cfg(test)]
mod tests {
    use crate::config::{
        config_from_env, DEFAULT_COMMIT_TO_CHECK_SAME_JOB_MAX,
        DEFAULT_COMMIT_TO_CHECK_SAME_REF_MAX, DEFAULT_PAGE_TO_FETCH_MAX,
    };
    use crate::config::{get_project_path, Config};
    use std::env::VarError;

    #[test]
    fn test_get_project_path() {
        assert_eq!(
            get_project_path("/aa/bb", "/bb/cc").unwrap(),
            ("/aa/bb/cc/")
        );
    }

    #[test]
    fn test_get_project_path_same_prefix() {
        assert_eq!(
            get_project_path("/aa/bb", "/aa/bb/cc").unwrap(),
            "/aa/bb/cc/"
        );
    }
    #[test]
    fn test_config_ok_min() {
        temp_env::with_vars(
            [
                ("SKIP_CI_VERBOSE", Some("true")),
                ("CI_API_V4_URL", Some("http://localhost/gitlab/api")),
                ("CI_PROJECT_DIR", Some("/aa/bb/cc")),
                ("CI_PROJECT_ID", Some("123")),
                ("CI_JOB_ID", Some("456")),
                ("API_READ_TOKEN", Some("__API_READ_TOKEN__")),
                ("CI_JOB_TOKEN", Some("__CI_JOB_TOKEN__")),
                ("CI_JOB_NAME", Some("__CI_JOB_NAME__")),
                ("SKIP_IF_TREE_OK_IN_PAST", Some("file1 file2")),
                ("CI_COMMIT_REF_NAME", Some("branch_name")),
                ("SKIP_CI_PAGE_TO_FETCH_MAX", None),
                ("SKIP_CI_COMMIT_TO_CHECK_SAME_REF_MAX", None),
                ("SKIP_CI_COMMIT_TO_CHECK_SAME_JOB_MAX", None),
            ],
            || {
                let config = config_from_env().unwrap();
                assert_eq!(config.api_read_token, "__API_READ_TOKEN__");
                assert_eq!(config.ci_commit_ref_name.unwrap(), "branch_name");
                assert_eq!(config.ci_job_name, "__CI_JOB_NAME__");
                assert_eq!(config.ci_job_token.unwrap(), "__CI_JOB_TOKEN__");
                assert!(config.verbose);
                assert_eq!(config.files_to_check, "file1 file2");
                assert_eq!(config.project_path, "/aa/bb/cc/");
                assert_eq!(
                    config.jobs_api_url,
                    "http://localhost/gitlab/api/projects/123/jobs"
                );
                assert_eq!(config.ci_skip_path, "/aa/bb/cc/ci-skip-123-456");
                assert_eq!(config.page_to_fetch_max, DEFAULT_PAGE_TO_FETCH_MAX);
                assert_eq!(
                    config.commit_to_check_same_ref_max,
                    DEFAULT_COMMIT_TO_CHECK_SAME_REF_MAX
                );
                assert_eq!(
                    config.commit_to_check_same_job_max,
                    DEFAULT_COMMIT_TO_CHECK_SAME_JOB_MAX
                );
            },
        );
    }

    #[test]
    fn test_config_ok() {
        temp_env::with_vars(
            [
                ("SKIP_CI_VERBOSE", Some("true")),
                ("CI_API_V4_URL", Some("http://localhost/gitlab/api")),
                ("CI_PROJECT_DIR", Some("/aa/bb/cc")),
                ("CI_PROJECT_ID", Some("123")),
                ("CI_JOB_ID", Some("456")),
                ("API_READ_TOKEN", Some("__API_READ_TOKEN__")),
                ("CI_JOB_TOKEN", Some("__CI_JOB_TOKEN__")),
                ("CI_JOB_NAME", Some("__CI_JOB_NAME__")),
                ("SKIP_IF_TREE_OK_IN_PAST", Some("file1 file2")),
                ("CI_COMMIT_REF_NAME", Some("branch_name")),
                ("SKIP_CI_PAGE_TO_FETCH_MAX", Some("3")),
                ("SKIP_CI_COMMIT_TO_CHECK_SAME_REF_MAX", Some("2")),
                ("SKIP_CI_COMMIT_TO_CHECK_SAME_JOB_MAX", Some("100")),
            ],
            || {
                let config = config_from_env().unwrap();
                assert_eq!(config.api_read_token, "__API_READ_TOKEN__");
                assert_eq!(config.ci_commit_ref_name.unwrap(), "branch_name");
                assert_eq!(config.ci_job_name, "__CI_JOB_NAME__");
                assert_eq!(config.ci_job_token.unwrap(), "__CI_JOB_TOKEN__");
                assert!(config.verbose);
                assert_eq!(config.files_to_check, "file1 file2");
                assert_eq!(config.project_path, "/aa/bb/cc/");
                assert_eq!(
                    config.jobs_api_url,
                    "http://localhost/gitlab/api/projects/123/jobs"
                );
                assert_eq!(config.ci_skip_path, "/aa/bb/cc/ci-skip-123-456");
                assert_eq!(config.page_to_fetch_max, 3);
                assert_eq!(config.commit_to_check_same_ref_max, 2);
                assert_eq!(config.commit_to_check_same_job_max, 100);
            },
        );
    }

    #[test]
    fn test_config_ok_bad_max_type() {
        temp_env::with_vars(
            [
                ("SKIP_CI_VERBOSE", None),
                ("CI_API_V4_URL", Some("http://localhost/gitlab/api")),
                ("CI_PROJECT_DIR", Some("/aa/bb/cc")),
                ("CI_PROJECT_ID", Some("123")),
                ("CI_JOB_ID", Some("456")),
                ("API_READ_TOKEN", Some("__API_READ_TOKEN__")),
                ("CI_JOB_TOKEN", Some("__CI_JOB_TOKEN__")),
                ("CI_JOB_NAME", Some("__CI_JOB_NAME__")),
                ("SKIP_IF_TREE_OK_IN_PAST", Some("file1 file2")),
                ("CI_COMMIT_REF_NAME", Some("branch_name")),
                ("SKIP_CI_PAGE_TO_FETCH_MAX", Some("A")),
                ("SKIP_CI_COMMIT_TO_CHECK_SAME_REF_MAX", Some("A")),
                ("SKIP_CI_COMMIT_TO_CHECK_SAME_JOB_MAX", Some("A")),
            ],
            || {
                let config = config_from_env().unwrap();
                assert_eq!(config.api_read_token, "__API_READ_TOKEN__");
                assert_eq!(config.ci_commit_ref_name.unwrap(), "branch_name");
                assert_eq!(config.ci_job_name, "__CI_JOB_NAME__");
                assert_eq!(config.ci_job_token.unwrap(), "__CI_JOB_TOKEN__");
                assert!(!config.verbose);
                assert_eq!(config.files_to_check, "file1 file2");
                assert_eq!(config.project_path, "/aa/bb/cc/");
                assert_eq!(
                    config.jobs_api_url,
                    "http://localhost/gitlab/api/projects/123/jobs"
                );
                assert_eq!(config.ci_skip_path, "/aa/bb/cc/ci-skip-123-456");
                assert_eq!(config.page_to_fetch_max, DEFAULT_PAGE_TO_FETCH_MAX);
                assert_eq!(
                    config.commit_to_check_same_ref_max,
                    DEFAULT_COMMIT_TO_CHECK_SAME_REF_MAX
                );
                assert_eq!(
                    config.commit_to_check_same_job_max,
                    DEFAULT_COMMIT_TO_CHECK_SAME_JOB_MAX
                );
            },
        );
    }

    #[test]
    fn test_config_ci_api_v4_url_is_not_defined() {
        temp_env::with_var("CI_API_V4_URL", None::<String>, || {
            assert_eq!(
                config_from_env().err().unwrap().to_string(),
                "CI_API_V4_URL is not defined"
            );
        });
    }

    #[test]
    fn test_config_ci_project_dir_is_not_defined() {
        temp_env::with_vars(
            [("CI_API_V4_URL", Some("http://localhost/gitlab/api"))],
            || {
                let err = config_from_env().err();
                assert_eq!(err.unwrap().to_string(), "CI_PROJECT_DIR is not defined");
            },
        );
    }

    #[test]
    fn test_config_ci_project_id_is_not_defined() {
        temp_env::with_vars(
            [
                ("CI_API_V4_URL", Some("http://localhost/gitlab/api")),
                ("CI_PROJECT_DIR", Some("/aa/bb/cc")),
            ],
            || {
                let err = config_from_env().err();
                assert_eq!(err.unwrap().to_string(), "CI_PROJECT_ID is not defined");
            },
        );
    }

    #[test]
    fn test_config_ci_job_id_is_not_defined() {
        temp_env::with_vars(
            [
                ("CI_API_V4_URL", Some("http://localhost/gitlab/api")),
                ("CI_PROJECT_DIR", Some("/aa/bb/cc")),
                ("CI_PROJECT_ID", Some("123")),
            ],
            || {
                let err = config_from_env().err();
                assert_eq!(err.unwrap().to_string(), "CI_JOB_ID is not defined");
            },
        );
    }

    #[test]
    fn test_config_api_read_token_is_not_defined() {
        temp_env::with_vars(
            [
                ("CI_API_V4_URL", Some("http://localhost/gitlab/api")),
                ("CI_PROJECT_DIR", Some("/aa/bb/cc")),
                ("CI_PROJECT_ID", Some("123")),
                ("CI_JOB_ID", Some("456")),
            ],
            || {
                let err = config_from_env().err().unwrap().to_string();
                assert_eq!(err, "API_READ_TOKEN is not defined");
            },
        );
    }

    #[test]
    fn test_config_ci_job_name_is_not_defined() {
        temp_env::with_vars(
            [
                ("CI_API_V4_URL", Some("http://localhost/gitlab/api")),
                ("CI_PROJECT_DIR", Some("/aa/bb/cc")),
                ("CI_PROJECT_ID", Some("123")),
                ("CI_JOB_ID", Some("456")),
                ("API_READ_TOKEN", Some("__API_READ_TOKEN__")),
            ],
            || {
                let err = config_from_env().err();
                assert_eq!(err.unwrap().to_string(), "CI_JOB_NAME is not defined");
            },
        );
    }

    #[test]
    fn test_config_skip_if_tree_ok_in_past_is_not_defined() {
        temp_env::with_vars(
            [
                ("CI_API_V4_URL", Some("http://localhost/gitlab/api")),
                ("CI_PROJECT_DIR", Some("/aa/bb/cc")),
                ("CI_PROJECT_ID", Some("123")),
                ("CI_JOB_ID", Some("456")),
                ("API_READ_TOKEN", Some("__API_READ_TOKEN__")),
                ("CI_JOB_NAME", Some("__CI_JOB_NAME__")),
            ],
            || {
                let err = config_from_env().err();
                assert_eq!(
                    err.unwrap().to_string(),
                    "SKIP_IF_TREE_OK_IN_PAST is not defined"
                );
            },
        );
    }

    #[test]
    fn test_config_no_job_token() {
        temp_env::with_vars(
            [
                ("CI_API_V4_URL", Some("http://localhost/gitlab/api")),
                ("CI_PROJECT_DIR", Some("/aa/bb/cc")),
                ("CI_PROJECT_ID", Some("123")),
                ("CI_JOB_ID", Some("456")),
                ("API_READ_TOKEN", Some("__API_READ_TOKEN__")),
                ("CI_JOB_NAME", Some("__CI_JOB_NAME__")),
                ("SKIP_IF_TREE_OK_IN_PAST", Some("file1 file2")),
            ],
            || {
                let err = config_from_env().err();
                assert!(err.is_none());
            },
        );
    }

    #[test]
    fn test_config_display() {
        let config = Config {
            api_read_token: "__API_READ_TOKEN__".to_string(),
            ci_commit_ref_name: Ok("__CI_COMMIT_REF_NAME__".to_string()),
            ci_job_name: "".to_string(),
            ci_job_token: Ok("__CI_JOB_TOKEN__".to_string()),
            verbose: false,
            files_to_check: "__files_to_check__".to_string(),
            project_path: "__project_path__".to_string(),
            jobs_api_url: "__jobs_api_url__".to_string(),
            ci_skip_path: "__ci_skip_path__".to_string(),
            page_to_fetch_max: 0,
            commit_to_check_same_ref_max: 0,
            commit_to_check_same_job_max: 0,
            skip: false,
        };
        let out = format!("{config}");
        assert_eq!(
            out,
            r###"
  project_path                 = __project_path__
  ci_commit_ref_name           = __CI_COMMIT_REF_NAME__
  ci_job_name                  = 
  verbose                      = false
  files_to_check               = __files_to_check__
  project_path                 = __project_path__
  jobs_api_url                 = __jobs_api_url__
  ci_skip_path                 = __ci_skip_path__
  api_read_token               = __API_READ_TOKEN__
  ci_job_token                 = __CI_JOB_TOKEN__
  page_to_fetch_max            = 0
  commit_to_check_same_ref_max = 0
  commit_to_check_same_job_max = 0"###
        );
    }

    #[test]
    fn test_config_display_empty_token() {
        let config = Config {
            api_read_token: "".to_string(),
            ci_commit_ref_name: Ok("__CI_COMMIT_REF_NAME__".to_string()),
            ci_job_name: "".to_string(),
            ci_job_token: Err(VarError::NotPresent),
            verbose: false,
            files_to_check: "__files_to_check__".to_string(),
            project_path: "__project_path__".to_string(),
            jobs_api_url: "__jobs_api_url__".to_string(),
            ci_skip_path: "__ci_skip_path__".to_string(),
            page_to_fetch_max: 0,
            commit_to_check_same_ref_max: 0,
            commit_to_check_same_job_max: 0,
            skip: false,
        };
        let out = format!("{config}");
        assert_eq!(
            out,
            r###"
  project_path                 = __project_path__
  ci_commit_ref_name           = __CI_COMMIT_REF_NAME__
  ci_job_name                  = 
  verbose                      = false
  files_to_check               = __files_to_check__
  project_path                 = __project_path__
  jobs_api_url                 = __jobs_api_url__
  ci_skip_path                 = __ci_skip_path__
  api_read_token               = 
  ci_job_token                 = 
  page_to_fetch_max            = 0
  commit_to_check_same_ref_max = 0
  commit_to_check_same_job_max = 0"###
        );
    }
}
