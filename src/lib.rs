use std::{fs, thread};
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use downloader::{Download, Downloader, DownloadSummary, Error, Verification};
use md5::{Digest, Md5};
use neon::prelude::*;

struct FileToDownload {
    pub path: String,
    pub md5_hash: String,
}

struct DownloadHandle {
    cancel_flag: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

fn start_download(mut cx: FunctionContext) -> JsResult<JsObject> {
    let repo_url = cx.argument::<JsString>(0)?.value(&mut cx);
    let destination_folder = cx.argument::<JsString>(1)?.value(&mut cx);
    let files_to_download = cx.argument::<JsArray>(2)?;
    let progress_callback = cx.argument::<JsFunction>(3)?.root(&mut cx);

    let files: Vec<FileToDownload> = files_to_download
        .to_vec(&mut cx)?
        .into_iter()
        .map(|v| {
            let obj = v.downcast::<JsObject, _>(&mut cx).unwrap();
            let path = obj.get::<JsString, _, _>(&mut cx, "path").unwrap().value(&mut cx);
            let md5_hash = obj.get::<JsString, _, _>(&mut cx, "md5_hash").unwrap().value(&mut cx);
            FileToDownload { path, md5_hash }
        })
        .collect();

    let (deferred, promise) = cx.promise();
    let channel = cx.channel();

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let reporter = Arc::new(ScarletDownloadReporter::new(progress_callback, channel.clone(), cancel_flag.clone()));

    let cancel_flag_clone = cancel_flag.clone();

    let download_thread = thread::spawn(move || {
        let result = download_files(repo_url, destination_folder, files, reporter, cancel_flag_clone);

        deferred.settle_with(&channel, move |mut cx| {
            match result {
                Ok(download_summaries) => {
                    let js_results = JsArray::new(&mut cx, download_summaries.len());
                    for (i, summary_result) in download_summaries.iter().enumerate() {
                        let obj = cx.empty_object();
                        match summary_result {
                            Ok(summary) => {
                                let file_name = cx.string(summary.file_name.to_str().unwrap_or(""));
                                obj.set(&mut cx, "fileName", file_name)?;
                                let verified = cx.boolean(summary.verified == Verification::Ok);
                                obj.set(&mut cx, "verified", verified)?;
                                let status_array = JsArray::new(&mut cx, summary.status.len());
                                for (j, (url, status_code)) in summary.status.iter().enumerate() {
                                    let status_obj = cx.empty_object();
                                    let url = cx.string(url);
                                    status_obj.set(&mut cx, "url", url)?;
                                    let status_code = cx.number(*status_code as f64);
                                    status_obj.set(&mut cx, "statusCode", status_code)?;
                                    status_array.set(&mut cx, j as u32, status_obj)?;
                                }
                                obj.set(&mut cx, "status", status_array)?;
                            },
                            Err(e) => {
                                let error = cx.string(e.to_string());
                                obj.set(&mut cx, "error", error)?;
                            }
                        }
                        js_results.set(&mut cx, i as u32, obj)?;
                    }
                    Ok(js_results)
                }
                Err(e) => {
                    cx.throw_error(e.to_string())
                },
            }
        });
    });

    let download_handle = Arc::new(Mutex::new(DownloadHandle {
        cancel_flag,
        thread: Some(download_thread),
    }));

    let download_handle_ptr = Arc::into_raw(download_handle) as u64;

    let result = cx.empty_object();
    result.set(&mut cx, "promise", promise)?;
    let handle_ptr = cx.number(download_handle_ptr as f64);
    result.set(&mut cx, "handlePtr", handle_ptr)?;

    Ok(result)
}

fn stop_download(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let handle_ptr = cx.argument::<JsNumber>(0)?.value(&mut cx) as u64;
    let download_handle = unsafe { Arc::from_raw(handle_ptr as *const Mutex<DownloadHandle>) };

    let handle = download_handle.lock().unwrap();
    handle.cancel_flag.store(true, Ordering::SeqCst);

    Ok(cx.undefined())
}

fn download_files(
    repo_url: String,
    destination_folder: String,
    files: Vec<FileToDownload>,
    reporter: Arc<dyn downloader::progress::Reporter + Send + Sync>,
    cancel_flag: Arc<AtomicBool>,
) -> Result<Vec<Result<DownloadSummary, Error>>, Error> {
    let downloads: Vec<Download> = files
        .into_iter()
        .map(|file| {
            let full_url = format!("{}{}", repo_url, file.path);
            let file_path = Path::new(&file.path).strip_prefix("/").unwrap();
            let file_path = Path::new(&destination_folder).join(file_path);
            let file_hash = file.md5_hash.clone();

            fs::create_dir_all(file_path.parent().unwrap())
                .map_err(|e| Error::Setup(e.to_string())).unwrap();

            Download::new(&full_url)
                .file_name(&file_path)
                .progress(reporter.clone())
                .verify(Arc::new(move |path, _progress| {
                    let hash = md5_hash_file(path.as_path()).unwrap_or_default();
                    if hash == file_hash.clone() {
                        Verification::Ok
                    } else {
                        Verification::Failed
                    }
                }))
        })
        .collect::<Vec<_>>();

    let mut downloader = Downloader::builder()
        .download_folder(Path::new(&destination_folder))
        .parallel_requests(1)
        .build()?;

    let mut results = Vec::new();

    for download in downloads {
        if cancel_flag.load(Ordering::SeqCst) {
            return Err(Error::Setup("Download cancelled".to_string()));
        }

        let result = downloader.download(&[download]);
        match result {
            Ok(mut summaries) => {
                if let Some(summary) = summaries.pop() {
                    results.push(summary);
                } else {
                    results.push(Err(Error::DownloadDefinition("No summary".into())));
                }
            }
            Err(e) => results.push(Err(e)),
        }

        // Check cancellation after each download
        if cancel_flag.load(Ordering::SeqCst) {
            return Err(Error::Setup("Download cancelled".to_string()));
        }
    }

    Ok(results)
}


struct ScarletDownloadReporter {
    callback: Arc<Root<JsFunction>>,
    channel: Arc<Channel>,
    last_update: Mutex<std::time::Instant>,
    current_progress: Arc<Mutex<u64>>,
    max_progress: Mutex<Option<u64>>,
    message: Mutex<String>,
    cancel_flag: Arc<AtomicBool>,
}

impl ScarletDownloadReporter {
    fn new(callback: Root<JsFunction>, channel: Channel, cancel_flag: Arc<AtomicBool>) -> Self {
        Self {
            callback: Arc::new(callback),
            channel: Arc::new(channel),
            last_update: Mutex::new(std::time::Instant::now()),
            current_progress: Arc::new(Mutex::new(0)),
            max_progress: Mutex::new(None),
            message: Mutex::new(String::new()),
            cancel_flag,
        }
    }

    fn call_js_callback(&self, current: u64, max: Option<u64>, message: String) {
        let callback = self.callback.clone();
        let channel = self.channel.clone();

        channel.send(move |mut cx| {
            let callback = callback.to_inner(&mut cx);
            let this = cx.undefined();
            let args = vec![
                cx.number(current as f64).upcast(),
                match max {
                    Some(m) => cx.number(m as f64).upcast(),
                    None => cx.undefined().upcast(),
                },
                cx.string(&message).upcast(),
            ];

            callback.call(&mut cx, this, args)?;
            Ok(())
        });
    }
}

impl downloader::progress::Reporter for ScarletDownloadReporter {
    fn setup(&self, max_progress: Option<u64>, message: &str) {
        *self.max_progress.lock().unwrap() = max_progress;
        *self.message.lock().unwrap() = message.to_owned();
        self.call_js_callback(0, max_progress, message.to_owned());
    }

    fn progress(&self, current: u64) {
        if self.cancel_flag.load(Ordering::SeqCst) {
            panic!("Download cancelled");
        }

        let mut progress = self.current_progress.lock().unwrap();
        *progress = current;
        let mut last_update = self.last_update.lock().unwrap();
        if last_update.elapsed().as_millis() >= 100 {
            *last_update = std::time::Instant::now();
            let max_progress = *self.max_progress.lock().unwrap();
            let message = self.message.lock().unwrap().clone();
            self.call_js_callback(current, max_progress, message);
        }
    }

    fn set_message(&self, message: &str) {
        *self.message.lock().unwrap() = message.to_owned();
        let current = *self.current_progress.lock().unwrap();
        let max_progress = *self.max_progress.lock().unwrap();
        self.call_js_callback(current, max_progress, message.to_owned());
    }

    fn done(&self) {
        let max_progress = *self.max_progress.lock().unwrap();
        let message = self.message.lock().unwrap().clone();
        let current = *self.current_progress.lock().unwrap();
        self.call_js_callback(current, max_progress, message);
    }
}

fn md5_hash(mut cx: FunctionContext) -> JsResult<JsString> {
    let file_path = cx.argument::<JsString>(0)?.value(&mut cx);
    let hash = md5_hash_file(Path::new(&file_path)).or_else(|e| cx.throw_error(e.to_string()))?;
    Ok(cx.string(hash))
}

fn md5_hash_file(path: &Path) -> Result<String, std::io::Error> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Md5::new();
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    hasher.update(&buffer);

    Ok(format!("{:x}", hasher.finalize()))
}

fn ping(mut cx: FunctionContext) -> JsResult<JsString> {
    Ok(cx.string("pong"))
}

#[neon::main]
fn main(mut cx: ModuleContext) -> NeonResult<()> {
    cx.export_function("md5_hash", md5_hash)?;
    cx.export_function("start_download", start_download)?;
    cx.export_function("stop_download", stop_download)?;
    cx.export_function("ping", ping)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    struct TestReporter;

    impl downloader::progress::Reporter for TestReporter {
        fn setup(&self, _max_progress: Option<u64>, _message: &str) {}
        fn progress(&self, _current: u64) {}
        fn set_message(&self, _message: &str) {}
        fn done(&self) {}
    }

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
        let cancel_flag = Arc::new(AtomicBool::new(false));

        let result = download_files(
            repo_url.to_string(),
            destination_folder.clone(),
            files_to_download,
            reporter,
            cancel_flag,
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
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let cancel_flag_clone = cancel_flag.clone();

        let (tx, rx) = std::sync::mpsc::channel();

        let download_thread = thread::spawn(move || {
            let result = download_files(
                repo_url.to_string(),
                destination_folder.clone(),
                files_to_download,
                reporter,
                cancel_flag_clone,
            );
            tx.send(result).unwrap();
        });

        // Wait a bit to ensure the download has started
        thread::sleep(Duration::from_millis(100));

        // Cancel the download
        cancel_flag.store(true, Ordering::SeqCst);

        // Wait for the download thread to finish
        download_thread.join().unwrap();

        // Check the result
        match rx.recv() {
            Ok(Ok(_)) => panic!("Download completed despite cancellation"),
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
}