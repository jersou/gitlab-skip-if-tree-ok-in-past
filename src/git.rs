use crate::verbose;
use anyhow::{anyhow, Context};
use git2::{Oid, Repository};
use std::path::Path;
use std::string::String;

pub fn get_tree_of_paths(
    repo: &Repository,
    commit_hash: &str,
    paths: &Vec<&Path>,
) -> anyhow::Result<String> {
    verbose!("get_tree_of_paths of {commit_hash} : {paths:?}");
    if paths.is_empty() {
        return Err(anyhow!("paths is empty".to_string()));
    }

    let commit_oid = Oid::from_str(commit_hash).context("commit hash error")?;

    let commit = repo
        .find_commit(commit_oid)
        .with_context(|| format!("commit {commit_hash} not found error:"))?;

    let tree = commit
        .tree()
        .with_context(|| format!("tree of commit {commit_hash} not found error"))?;

    let mut tree_of_job_files = String::new();
    for path in paths {
        let tree_id = tree
            .get_path(path)
            .with_context(|| format!("tree of commit {commit_hash} not found error"))?
            .id();
        let path_str = path.to_str().context("path empty")?;
        tree_of_job_files.push_str(&format!("{tree_id} {path_str}\n"));
    }

    verbose!("{}\n{tree_of_job_files}{}", "-".repeat(80), "-".repeat(80));
    Ok(tree_of_job_files)
}

#[cfg(test)]
mod tests {
    use crate::git::get_tree_of_paths;
    use git2::Repository;
    use std::fs::File;
    use std::path::Path;
    use tempfile::{tempdir, TempDir};

    fn get_tmp_repo() -> (TempDir, Repository) {
        let tmp_dir = tempdir().unwrap();
        let repo_zip = Path::new("test/repo.zip");
        let zip_file = File::open(repo_zip).unwrap();
        let mut archive = zip::ZipArchive::new(zip_file).unwrap();
        archive.extract(&tmp_dir).unwrap();

        let git_path = tmp_dir.path().join(".git");
        let repo = Repository::open_bare(git_path.to_str().unwrap()).unwrap();
        (tmp_dir, repo)
    }

    #[test]
    fn test_get_tree_of_paths_ok_2_files() {
        let (_tmp_dir, repo) = get_tmp_repo();
        let paths = vec![Path::new("root-1"), Path::new("Service-A/file-A1")];

        let tree_of_paths =
            get_tree_of_paths(&repo, "ef08d93fdeabf23734248d6f95ab4ff3952e9856", &paths).unwrap();
        assert_eq!(tree_of_paths,"d00491fd7e5bb6fa28c517a0bb32b8b506539d4d root-1\nd00491fd7e5bb6fa28c517a0bb32b8b506539d4d Service-A/file-A1\n");
    }

    #[test]
    fn test_get_tree_of_paths_ok_1_file() {
        let (_tmp_dir, repo) = get_tmp_repo();
        let tree_of_paths = get_tree_of_paths(
            &repo,
            "ef08d93fdeabf23734248d6f95ab4ff3952e9856",
            &vec![Path::new("root-1")],
        )
        .unwrap();
        assert_eq!(
            tree_of_paths,
            "d00491fd7e5bb6fa28c517a0bb32b8b506539d4d root-1\n"
        );
    }

    #[test]
    fn test_get_tree_of_paths() {
        let (_tmp_dir, repo) = get_tmp_repo();
        let paths = vec![Path::new("root-1"), Path::new("Service-A/file-A1")];
        let tree_of_paths = get_tree_of_paths(&repo, "zz", &paths);
        assert_eq!(tree_of_paths.err().map(|e| format!("{e:#}")).unwrap(),"commit hash error: unable to parse OID - contains invalid characters; class=Invalid (3)");
    }

    #[test]
    fn test_get_tree_of_paths_commit_not_found_error() {
        let (_tmp_dir, repo) = get_tmp_repo();
        let paths = vec![Path::new("root-1"), Path::new("Service-A/file-A1")];
        let tree_of_paths =
            get_tree_of_paths(&repo, "0000000000000000000000000000000000000000", &paths);
        let err = tree_of_paths.err().unwrap();
        assert_eq!(
            err.to_string(),
            "commit 0000000000000000000000000000000000000000 not found error:"
        );
        assert_eq!(
            err.root_cause().to_string(),
            "odb: cannot read object: null OID cannot exist; class=Odb (9); code=NotFound (-3)"
        );
    }

    #[test]
    fn test_get_tree_of_paths_paths_is_empty() {
        let (_tmp_dir, repo) = get_tmp_repo();
        let tree_of_paths =
            get_tree_of_paths(&repo, "ef08d93fdeabf23734248d6f95ab4ff3952e9856", &vec![]);
        assert_eq!(
            tree_of_paths.err().map(|e| format!("{:#}", e)).unwrap(),
            "paths is empty"
        );
    }

    #[test]
    fn test_get_tree_of_paths_not_found_error() {
        let (_tmp_dir, repo) = get_tmp_repo();
        let paths = "root-1 Service-A/file-A1 file-not-found"
            .split(' ')
            .map(Path::new)
            .collect::<Vec<&Path>>();
        let tree_of_paths =
            get_tree_of_paths(&repo, "ef08d93fdeabf23734248d6f95ab4ff3952e9856", &paths);
        let mut err_msg = String::new();
        err_msg
            .push_str("tree of commit ef08d93fdeabf23734248d6f95ab4ff3952e9856 not found error: ");
        err_msg.push_str("the path 'file-not-found' does not exist in the given tree; class=Tree (14); code=NotFound (-3)");
        assert_eq!(tree_of_paths.err().map(|e| format!("{e:#}")), Some(err_msg));
    }
}
