use std::time::Duration;
use serial_test::serial;
use tempfile::tempdir;

use super::*;



struct TestReporter;

impl downloader::progress::Reporter for TestReporter {
    fn setup(&self, _max_progress: Option<u64>, _message: &str) {}
    fn progress(&self, _current: u64) {
        if CANCELLATION_FLAG.load(Ordering::SeqCst) {
            panic!("Download cancelled");
        }
    }
    fn set_message(&self, _message: &str) {}
    fn done(&self) {}
}

#[serial]
#[test]
fn test_download_files() {
    let temp_dir = tempdir().unwrap();
    let destination_folder = temp_dir.path().to_str().unwrap().to_string();
    let repo_url = "https://www.electronjs.org";
    let files_to_download = vec![
        FileToDownload {
            path: "/assets/img/logo.svg".to_string(),
            md5_hash: "bbe5da8f172a8af961362b0f8ad84175".to_string(),
        }
    ];

    let reporter = Arc::new(TestReporter);

    let result = download_files(
        repo_url.to_string(),
        destination_folder.clone(),
        files_to_download,
        reporter,
    );

    assert!(result.is_ok(), "Download should succeed");
    let summaries = result.unwrap();
    assert_eq!(summaries.len(), 1, "Should have one download summary");

    for summary_result in summaries {
        match summary_result {
            Ok(summary) => {
                assert_eq!(summary.verified, Verification::Ok, "File {} should be verified", summary.file_name.display());
                assert!(!summary.status.is_empty(), "Status should not be empty");
            }
            Err(e) => panic!("Download failed: {:?}", e),
        }
    }
}

#[serial]
#[test]
fn test_download_file_twice() {
    let temp_dir = tempdir().unwrap();
    let destination_folder = temp_dir.path().to_str().unwrap().to_string();
    let repo_url = "https://www.electronjs.org";
    let files_to_download = vec![
        FileToDownload {
            path: "/assets/img/logo.svg".to_string(),
            md5_hash: "bbe5da8f172a8af961362b0f8ad84175".to_string(),
        }
    ];

    let reporter = Arc::new(TestReporter);

    let result = download_files(
        repo_url.to_string(),
        destination_folder.clone(),
        files_to_download,
        reporter,
    );

    let files_to_download = vec![
        FileToDownload {
            path: "/assets/img/logo.svg".to_string(),
            md5_hash: "bbe5da8f172a8af961362b0f8ad84175".to_string(),
        }
    ];

    let reporter = Arc::new(TestReporter);

    let result = download_files(
        repo_url.to_string(),
        destination_folder.clone(),
        files_to_download,
        reporter,
    );

    assert!(result.is_ok(), "Download should succeed");
    let summaries = result.unwrap();
    assert_eq!(summaries.len(), 1, "Should have one download summary");

    for summary_result in summaries {
        match summary_result {
            Ok(summary) => {
                assert_eq!(summary.verified, Verification::Ok, "File {} should be verified", summary.file_name.display());
                assert!(!summary.status.is_empty(), "Status should not be empty");
            }
            Err(e) => panic!("Download failed: {:?}", e),
        }
    }
}

#[serial]
#[test]
fn test_cancelling_download() {
    let temp_dir = tempdir().unwrap();
    let destination_folder = temp_dir.path().to_str().unwrap().to_string();
    let repo_url = "https://www.electronjs.org";
    let files_to_download = vec![
        FileToDownload {
            path: "/assets/img/logo.svg".to_string(),
            md5_hash: "bbe5da8f172a8af961362b0f8ad84175".to_string(),
        },
        // Add more files to make the download take longer
        FileToDownload {
            path: "/assets/img/hero-cloud.png".to_string(),
            md5_hash: "fake_hash_to_force_redownload".to_string(),
        },
    ];

    let reporter = Arc::new(TestReporter);

    let (tx, rx) = std::sync::mpsc::channel();

    let download_thread = thread::spawn(move || {
        let result = download_files(
            repo_url.to_string(),
            destination_folder.clone(),
            files_to_download,
            reporter,
        );
        tx.send(result).unwrap();
    });

    // Wait a bit to ensure the download has started
    thread::sleep(Duration::from_millis(100));

    // Cancel the download
    CANCELLATION_FLAG.store(true, Ordering::SeqCst);

    // Wait for the download thread to finish
    download_thread.join().unwrap();

    // Check the result
    match rx.recv() {
        Ok(Ok(a)) => {
            panic!("Download should have been cancelled, but got: {:?}", a);
        },
        Ok(Err(e)) if format!("{:?}", e).contains("Download cancelled") => {
            // This is the expected outcome: the download was cancelled
            assert!(true, "Download was successfully cancelled");
        },
        Ok(Err(e)) => panic!("Unexpected error: {:?}", e),
        Err(e) => panic!("Channel error: {:?}", e),
    }

    // Verify that no files (or only partial files) were downloaded
    let downloaded_files: Vec<_> = temp_dir.path().read_dir().unwrap().collect();
    assert!(downloaded_files.len() <= 1, "At most one file should be partially downloaded");
}
