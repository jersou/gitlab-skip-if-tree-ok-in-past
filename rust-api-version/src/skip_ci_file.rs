use crate::verbose;
use anyhow::Context;
use std::path::Path;
use tokio::fs;

// check if the skip is already done, and return the result from the skip-ci file
pub async fn check_skip_is_done(path_str: &str) -> Option<bool> {
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
pub async fn write_skip_done(path_str: &str, result: bool) -> anyhow::Result<()> {
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

#[cfg(test)]
pub mod tests {
    use crate::skip_ci_file::{check_skip_is_done, write_skip_done};
    use std::fs;
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
}
