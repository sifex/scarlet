#[cfg(test)]
mod tests {

    use crate::download::DownloadManager;
    use std::collections::HashSet;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn create_test_file(dir: &Path, path: &str, content: &str) -> std::io::Result<()> {
        let file_path = dir.join(path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = File::create(file_path)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }

    #[tokio::test]
    async fn test_cleanup_files_with_expected_files() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path();

        // Create test file structure
        create_test_file(base_path, "managed_dir1/file1.txt", "content")?;
        create_test_file(base_path, "managed_dir1/subdir/file2.txt", "content")?;
        create_test_file(base_path, "managed_dir1/subdir/unexpected.txt", "content")?;
        create_test_file(base_path, "managed_dir2/file3.txt", "content")?;
        create_test_file(
            base_path,
            "managed_dir2/unexpected_dir/unexpected.txt",
            "content",
        )?;
        create_test_file(base_path, "unmanaged_file.txt", "content")?;

        let mut expected_files = HashSet::new();
        expected_files.insert(PathBuf::from("managed_dir1/file1.txt"));
        expected_files.insert(PathBuf::from("managed_dir1/subdir/file2.txt"));
        expected_files.insert(PathBuf::from("managed_dir2/file3.txt"));

        let download_manager = DownloadManager::new();
        download_manager
            .cleanup_files(base_path, &expected_files)
            .await?;

        // Check that expected files still exist
        assert!(base_path.join("managed_dir1/file1.txt").exists());
        assert!(base_path.join("managed_dir1/subdir/file2.txt").exists());
        assert!(base_path.join("managed_dir2/file3.txt").exists());

        // Check that unexpected files within managed directories were removed
        assert!(!base_path
            .join("managed_dir1/subdir/unexpected.txt")
            .exists());
        assert!(!base_path
            .join("managed_dir2/unexpected_dir/unexpected.txt")
            .exists());

        // Check that unmanaged files were not touched
        assert!(base_path.join("unmanaged_file.txt").exists());

        // Check that empty directories were removed
        assert!(!base_path.join("managed_dir2/unexpected_dir").exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_cleanup_files_with_nested_empty_dirs() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path();

        // Create test file structure with empty directories
        fs::create_dir_all(base_path.join("example_managed_directory/@AAF_Modern/empty_dir"))?;
        create_test_file(base_path, "example_managed_directory/file1.txt", "content")?;

        let mut expected_files = HashSet::new();
        expected_files.insert(PathBuf::from("example_managed_directory/file1.txt"));

        let download_manager = DownloadManager::new();
        download_manager
            .cleanup_files(base_path, &expected_files)
            .await?;

        // Check that expected file still exists
        assert!(base_path
            .join("example_managed_directory/file1.txt")
            .exists());

        // Check that empty directories were removed
        assert!(!base_path
            .join("example_managed_directory/@AAF_Modern/empty_dir")
            .exists());
        assert!(!base_path
            .join("example_managed_directory/@AAF_Modern")
            .exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_cleanup_files_with_non_managed_dirs() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let base_path = temp_dir.path();

        // Create test file structure with non-managed directories
        create_test_file(base_path, "example_managed_directory/file1.txt", "content")?;
        create_test_file(base_path, "NonManaged/file2.txt", "content")?;
        create_test_file(base_path, "file2.txt", "content")?;

        let mut expected_files = HashSet::new();
        expected_files.insert(PathBuf::from("example_managed_directory/"));
        expected_files.insert(PathBuf::from("example_managed_directory/file1.txt"));

        let download_manager = DownloadManager::new();
        download_manager
            .cleanup_files(base_path, &expected_files)
            .await?;

        // Check that expected file still exists
        assert!(base_path
            .join("example_managed_directory/file1.txt")
            .exists());

        // Check that non-managed directory and its contents were not touched
        assert!(base_path.join("NonManaged/file2.txt").exists());
        assert!(base_path.join("file2.txt").exists());

        Ok(())
    }

    // #[tokio::test]
    // async fn test_download_and_verify() -> Result<(), Box<dyn std::error::Error>> {
    //     let temp_dir = TempDir::new()?;
    //     let base_path = temp_dir.path();
    //
    //     let test_file_content = "Test content";
    //     let test_file_hash = "d5ef9984be135ec74cbc5dee24825199ca433e1ffd7a2fb22db12a3c5324ea1d"; // SHA256 hash of "Test content"
    //
    //     let mut server = mockito::Server::new();
    //     let mock = server.mock("GET", "/test_file.txt")
    //         .with_status(200)
    //         .with_header("content-type", "text/plain")
    //         .with_body(test_file_content)
    //         .create();
    //
    //     let files = vec![
    //         FileToDownload {
    //             url: server.url() + "/test_file.txt",
    //             path: "test_file.txt".to_string(),
    //             sha256_hash: test_file_hash.to_string(),
    //         },
    //     ];
    //
    //     let download_manager = DownloadManager::new();
    //     download_manager.download(base_path, files).await?;
    //
    //     // Verify that the file was downloaded and has the correct content
    //     let downloaded_file_path = base_path.join("test_file.txt");
    //     assert!(downloaded_file_path.exists());
    //     let downloaded_content = fs::read_to_string(downloaded_file_path)?;
    //     assert_eq!(downloaded_content, test_file_content);
    //
    //     mock.assert();
    //
    //     Ok(())
    // }
    //
    // #[tokio::test]
    // async fn test_download_with_invalid_hash() -> Result<(), Box<dyn std::error::Error>> {
    //     let temp_dir = TempDir::new()?;
    //     let base_path = temp_dir.path();
    //
    //     let test_file_content = "Test content";
    //     let invalid_hash = "invalid_hash";
    //
    //     let mut server = mockito::Server::new();
    //     let mock = server.mock("GET", "/test_file.txt")
    //         .with_status(200)
    //         .with_header("content-type", "text/plain")
    //         .with_body(test_file_content)
    //         .create();
    //
    //     let files = vec![
    //         FileToDownload {
    //             url: server.url() + "/test_file.txt",
    //             path: "test_file.txt".to_string(),
    //             sha256_hash: invalid_hash.to_string(),
    //         },
    //     ];
    //
    //     let download_manager = DownloadManager::new();
    //     let result = download_manager.download(base_path, files).await;
    //
    //     assert!(result.is_err());
    //     assert!(matches!(result, Err(DownloadError::ChecksumMismatch)));
    //
    //     mock.assert();
    //
    //     Ok(())
    // }
    //
    // #[tokio::test]
    // async fn test_download_with_network_error() -> Result<(), Box<dyn std::error::Error>> {
    //     let temp_dir = TempDir::new()?;
    //     let base_path = temp_dir.path();
    //
    //     let mut server = mockito::Server::new();
    //     let mock = server.mock("GET", "/test_file.txt")
    //         .with_status(404)
    //         .create();
    //
    //     let files = vec![
    //         FileToDownload {
    //             url: server.url() + "/test_file.txt",
    //             path: "test_file.txt".to_string(),
    //             sha256_hash: "some_hash".to_string(),
    //         },
    //     ];
    //
    //     let download_manager = DownloadManager::new();
    //     let result = download_manager.download(base_path, files).await;
    //
    //     assert!(result.is_err());
    //     assert!(matches!(result, Err(DownloadError::HttpError(_))));
    //
    //     mock.assert();
    //
    //     Ok(())
    // }
}
