use crate::config::Config;
use crate::git::get_tree_of_paths;
use crate::jobs::{get_project_jobs, GitlabJob};
use crate::verbose;
use anyhow::Context;
use git2::Repository;
use std::path::Path;

pub async fn find_last_job_ok(config: &Config) -> anyhow::Result<Option<GitlabJob>> {
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
                verbose!("Check job {}", job.id);
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

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::find_last_job_ok::find_last_job_ok;
    use crate::process::tests::{add_jobs_expect, create_config_ok, prepare_tmp_repo};
    use git2::Oid;
    use httptest::Server;
    use std::fs;
    use std::fs::File;
    use std::path::Path;
    use tempdir::TempDir;

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
