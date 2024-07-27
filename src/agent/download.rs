use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::StreamExt;
// Changed from md5 to sha2
use reqwest::Client;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::sync::Mutex;
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DownloadStatus {
    Ready,
    InitialCheck,
    Downloading,
    Verifying,
    Done,
    Error,
}

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub status: DownloadStatus,
    pub files_total: usize,
    pub files_total_completed: usize,
    pub verification_total_completed: usize,
    pub current_file_downloaded: u64,
    pub current_file_total_size: u64,
    pub current_file_path: String,
}

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Checksum mismatch")]
    ChecksumMismatch,
    #[error("Download cancelled")]
    Cancelled,
}

#[derive(Clone)]
pub struct FileToDownload {
    pub url: String,
    pub path: String,
    pub sha256_hash: String, // Changed from md5_hash to sha256_hash
}

pub struct DownloadManager {
    client: Client,
    progress: Arc<Mutex<DownloadProgress>>,
    pub(crate) cancellation_flag: Arc<std::sync::atomic::AtomicBool>,
}

impl DownloadManager {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            progress: Arc::new(Mutex::new(DownloadProgress {
                status: DownloadStatus::Ready,
                files_total: 0,
                files_total_completed: 0,
                verification_total_completed: 0,
                current_file_downloaded: 0,
                current_file_total_size: 0,
                current_file_path: String::new(),
            })),
            cancellation_flag: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub async fn download(
        &self,
        destination_folder: impl AsRef<Path>,
        files: Vec<FileToDownload>,
    ) -> Result<(), DownloadError> {
        let destination_folder = destination_folder.as_ref().to_path_buf();
        let num_files = files.len();

        self.cancellation_flag
            .store(false, std::sync::atomic::Ordering::SeqCst);

        self.initialize_progress(num_files).await;

        let mut expected_files = HashSet::new();

        for (_, file) in files.iter().enumerate() {
            if self.is_cancelled() {
                self.reset_progress().await;
                return Err(DownloadError::Cancelled);
            }

            self.update_progress_for_file(file).await;

            let formatted_file_path = PathBuf::from(file.path.trim_start_matches('/'));
            expected_files.insert(PathBuf::from(&formatted_file_path));
            let file_path = destination_folder.join(&formatted_file_path);

            if self.file_is_valid(&file_path, &file.sha256_hash).await {
                self.update_progress_for_completed_file().await;
                continue;
            }

            self.download_file(file, &file_path).await?;

            self.update_progress_for_completed_file().await;
        }

        self.cleanup_files(&destination_folder, &expected_files)
            .await?;

        self.finalize_progress().await;

        Ok(())
    }

    async fn download_file(
        &self,
        file: &FileToDownload,
        file_path: &Path,
    ) -> Result<(), DownloadError> {
        self.prepare_for_download().await;

        let response = self.client.get(&file.url).send().await?;
        let total_size = response.content_length().unwrap_or(0);

        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file_handle = File::create(file_path)?;
        let mut downloaded = 0;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            if self.is_cancelled() {
                self.reset_progress().await;
                return Err(DownloadError::Cancelled);
            }

            let chunk = chunk?;
            file_handle.write_all(&chunk)?;
            downloaded += chunk.len() as u64;

            self.update_download_progress(downloaded, total_size).await;
        }

        self.verify_file(file_path, &file.sha256_hash).await
    }

    async fn verify_file(
        &self,
        file_path: &Path,
        expected_hash: &str,
    ) -> Result<(), DownloadError> {
        let mut progress = self.progress.lock().await;
        progress.status = DownloadStatus::Verifying;
        drop(progress);

        let calculated_hash = self.calculate_sha256(file_path).await?;
        if calculated_hash != expected_hash {
            let mut progress = self.progress.lock().await;
            progress.status = DownloadStatus::Error;
            return Err(DownloadError::ChecksumMismatch);
        }

        let mut progress = self.progress.lock().await;
        progress.verification_total_completed += 1;
        Ok(())
    }

    pub async fn cleanup_files(
        &self,
        destination_folder: &Path,
        expected_files: &HashSet<PathBuf>,
    ) -> std::io::Result<()> {
        let mut progress = self.progress.lock().await;
        progress.status = DownloadStatus::Verifying;
        drop(progress);

        let base_path = PathBuf::from(destination_folder);

        // Extract managed directories
        let managed_dirs: HashSet<PathBuf> = expected_files
            .iter()
            .filter_map(|path| path.components().next())
            .map(|comp| base_path.join(comp.as_os_str()))
            .collect();

        // Collect all paths that should be kept
        let mut keep_paths = HashSet::new();
        for expected_file in expected_files {
            let full_path = base_path.join(expected_file);
            keep_paths.insert(full_path.clone());
            // Add all parent directories to keep_paths
            for ancestor in full_path.ancestors().skip(1) {
                if ancestor.starts_with(&base_path) {
                    keep_paths.insert(ancestor.to_path_buf());
                } else {
                    break;
                }
            }
        }

        // Walk the directory tree in reverse order (bottom-up)
        for entry in WalkDir::new(destination_folder).contents_first(true) {
            let entry = entry?;
            let path = entry.path().to_path_buf();

            // Only process files and directories within managed directories
            if !managed_dirs.iter().any(|dir| path.starts_with(dir)) {
                continue;
            }

            if !keep_paths.contains(&path) {
                if entry.file_type().is_dir() {
                    // Check if directory is empty before removing
                    if fs::read_dir(&path)?.next().is_none() {
                        println!("Removing empty directory: {:?}", path);
                        fs::remove_dir(path)?;
                    }
                } else {
                    println!("Removing file: {:?}", path);
                    fs::remove_file(path)?;
                }
            }
        }

        Ok(())
    }

    async fn reset_progress(&self) {
        let mut progress = self.progress.lock().await;
        progress.status = DownloadStatus::Ready;
        progress.files_total = 0;
        progress.files_total_completed = 0;
        progress.verification_total_completed = 0;
        progress.current_file_downloaded = 0;
        progress.current_file_total_size = 0;
        progress.current_file_path = String::new();
    }

    async fn initialize_progress(&self, num_files: usize) {
        let mut progress = self.progress.lock().await;
        progress.status = DownloadStatus::InitialCheck;
        progress.files_total = num_files;
        progress.files_total_completed = 0;
        progress.verification_total_completed = 0;
        progress.current_file_downloaded = 0;
        progress.current_file_total_size = 0;
        progress.current_file_path = String::new();
    }

    async fn update_progress_for_file(&self, file: &FileToDownload) {
        let mut progress = self.progress.lock().await;
        progress.status = DownloadStatus::InitialCheck;
        progress.current_file_path = file.path.clone();
    }

    async fn update_progress_for_completed_file(&self) {
        let mut progress = self.progress.lock().await;
        progress.files_total_completed += 1;
        progress.verification_total_completed += 1;
        progress.current_file_downloaded = 0;
        progress.current_file_total_size = 0;
    }

    async fn finalize_progress(&self) {
        let mut progress = self.progress.lock().await;
        progress.status = DownloadStatus::Done;
        progress.verification_total_completed = progress.files_total;
    }

    async fn file_is_valid(&self, file_path: &Path, expected_hash: &str) -> bool {
        if let Ok(existing_hash) = self.calculate_sha256(file_path).await {
            existing_hash == expected_hash
        } else {
            false
        }
    }

    async fn prepare_for_download(&self) {
        let mut progress = self.progress.lock().await;
        progress.status = DownloadStatus::Downloading;
        progress.current_file_downloaded = 0;
        progress.current_file_total_size = 0;
    }

    async fn update_download_progress(&self, downloaded: u64, total_size: u64) {
        let mut progress = self.progress.lock().await;
        progress.current_file_downloaded = downloaded;
        progress.current_file_total_size = total_size;
    }

    async fn calculate_sha256(&self, file_path: &Path) -> Result<String, std::io::Error> {
        let mut file = File::open(file_path)?;
        let mut hasher = Sha256::new();
        std::io::copy(&mut file, &mut hasher)?;
        let result = hasher.finalize();
        Ok(result.iter().map(|b| format!("{:02x}", b)).collect())
    }

    fn is_cancelled(&self) -> bool {
        self.cancellation_flag
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn cancel(&self) {
        self.cancellation_flag
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub async fn get_progress(&self) -> DownloadProgress {
        self.progress.lock().await.clone()
    }
}
