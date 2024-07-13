#[cfg(test)]
mod tests {
    use mockito;
    use tokio::runtime::Runtime;

    use crate::download::{DownloadManager, DownloadStatus, FileToDownload};

    fn create_test_files() -> Vec<FileToDownload> {
        vec![
            FileToDownload {
                url: "http://example.com/file1.txt".to_string(),
                path: "file1.txt".to_string(),
                md5_hash: "d41d8cd98f00b204e9800998ecf8427e".to_string(),
            },
            FileToDownload {
                url: "http://example.com/file2.txt".to_string(),
                path: "file2.txt".to_string(),
                md5_hash: "098f6bcd4621d373cade4e832627b4f6".to_string(),
            },
        ]
    }

    #[test]
    fn test_new_download_manager() {
        let dm = DownloadManager::new();
        let rt = Runtime::new().unwrap();
        let progress = rt.block_on(dm.get_progress());

        assert_eq!(progress.status, DownloadStatus::NotReady);
        assert_eq!(progress.files_total, 0);
        assert_eq!(progress.files_total_completed, 0);
        assert_eq!(progress.verification_total_completed, 0);
    }

    #[test]
    fn test_cancel_download() {
        let dm = DownloadManager::new();
        dm.cancel();
        assert!(dm.cancellation_flag.load(std::sync::atomic::Ordering::SeqCst));
    }

    // #[tokio::test]
    // async fn test_download_progress_update() {
    //     let dm = DownloadManager::new();
    //     let files = create_test_files();
    //     let destination = "test_destination";
    // 
    //     // Start the download in a separate task
    //     let dm_clone = dm.clone();
    //     tokio::spawn(async move {
    //         let _ = dm_clone.download(destination, files).await;
    //     });
    // 
    //     // Wait a bit for the download to start
    //     tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    // 
    //     let progress = dm.get_progress().await;
    // 
    //     // Check if the progress is updated correctly
    //     assert_ne!(progress.status, DownloadStatus::NotReady);
    //     assert_eq!(progress.files_total, 2);
    //     assert!(progress.files_total_completed <= 2);
    //     assert!(progress.verification_total_completed <= 2);
    //     assert!(progress.current_file_downloaded <= progress.current_file_total_size);
    // }
    // 
    // #[tokio::test]
    // async fn test_download_file_success() {
    //     let mut server = mockito::Server::new();
    //     let mock = server.mock("GET", "/file1.txt")
    //         .with_status(200)
    //         .with_header("content-type", "text/plain")
    //         .with_body("Hello, World!")
    //         .create();
    // 
    //     let dm = DownloadManager::new();
    //     let files = vec![
    //         FileToDownload {
    //             url: server.url() + "/file1.txt",
    //             path: "file1.txt".to_string(),
    //             md5_hash: "65a8e27d8879283831b664bd8b7f0ad4".to_string(), // MD5 of "Hello, World!"
    //         },
    //     ];
    // 
    //     let temp_dir = tempfile::tempdir().unwrap();
    //     let result = dm.download(temp_dir.path(), files).await;
    // 
    //     assert!(result.is_ok());
    //     mock.assert();
    // 
    //     let file_path = temp_dir.path().join("file1.txt");
    //     assert!(file_path.exists());
    // 
    //     let content = std::fs::read_to_string(file_path).unwrap();
    //     assert_eq!(content, "Hello, World!");
    // }
    // 
    // #[tokio::test]
    // async fn test_download_file_checksum_mismatch() {
    //     let mut server = mockito::Server::new();
    //     let mock = server.mock("GET", "/file1.txt")
    //         .with_status(200)
    //         .with_header("content-type", "text/plain")
    //         .with_body("Hello, World!")
    //         .create();
    // 
    //     let dm = DownloadManager::new();
    //     let files = vec![
    //         FileToDownload {
    //             url: server.url() + "/file1.txt",
    //             path: "file1.txt".to_string(),
    //             md5_hash: "incorrect_hash".to_string(),
    //         },
    //     ];
    // 
    //     let temp_dir = tempfile::tempdir().unwrap();
    //     let result = dm.download(temp_dir.path(), files).await;
    // 
    //     assert!(result.is_err());
    //     mock.assert();
    // 
    //     let file_path = temp_dir.path().join("file1.txt");
    //     assert!(file_path.exists()); // The file is still downloaded, but the checksum fails
    // }
    // 
    // #[tokio::test]
    // async fn test_download_file_network_error() {
    //     let mut server = mockito::Server::new();
    //     let mock = server.mock("GET", "/file1.txt")
    //         .with_status(404)
    //         .create();
    // 
    //     let dm = DownloadManager::new();
    //     let files = vec![
    //         FileToDownload {
    //             url: server.url() + "/file1.txt",
    //             path: "file1.txt".to_string(),
    //             md5_hash: "some_hash".to_string(),
    //         },
    //     ];
    // 
    //     let temp_dir = tempfile::tempdir().unwrap();
    //     let result = dm.download(temp_dir.path(), files).await;
    // 
    //     assert!(result.is_err());
    //     mock.assert();
    // 
    //     let file_path = temp_dir.path().join("file1.txt");
    //     assert!(!file_path.exists()); // The file should not be created due to the network error
    // }
}