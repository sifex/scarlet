use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};

use downloader::{Download, Downloader, DownloadSummary, Error, Verification};
use md5::{Digest, Md5};
use neon::prelude::*;

struct FileToDownload {
    pub path: String,
    pub md5_hash: String,
}

fn md5_hash(mut cx: FunctionContext) -> JsResult<JsString> {
    let file_path = cx.argument::<JsString>(0)?.value(&mut cx);
    let hash = md5_hash_file(Path::new(&file_path)).or_else(|e| cx.throw_error(e.to_string()))?;
    Ok(cx.string(hash))
}

fn delete_file(mut cx: FunctionContext) -> JsResult<JsBoolean> {
    let file_path = cx.argument::<JsString>(0)?.value(&mut cx);
    match fs::remove_file(file_path) {
        Ok(_) => Ok(cx.boolean(true)),
        Err(_) => Ok(cx.boolean(false))
    }
}

fn download_files(
    repo_url: String,
    destination_folder: String,
    files: Vec<FileToDownload>,
    reporter: Arc<dyn downloader::progress::Reporter + Send + Sync>,
) -> Result<Vec<Result<DownloadSummary, Error>>, Error> {
    let downloads: Vec<Download> = files
        .into_iter()
        .map(|file| {
            let full_url = format!("{}{}", repo_url, file.path);
            let file_path = Path::new(&file.path).strip_prefix("/").unwrap();
            let file_path = Path::new(&destination_folder).join(file_path);
            let file_hash = file.md5_hash.clone();

            // Ensure folder path exists
            fs::create_dir_all(file_path.parent().unwrap()).map_err(|e| Error::Setup(e.to_string())).unwrap();

            return Download::new(&full_url)
                .file_name(&file_path)
                .progress(reporter.clone())
                .verify(Arc::new(move |path, _progress| {
                    let hash = md5_hash_file(path.as_path()).unwrap_or_default();
                    if hash == file_hash.clone() {
                        Verification::Ok
                    } else {
                        Verification::Failed
                    }
                }));
        })
        .collect();

    let mut downloader = Downloader::builder()
        .download_folder(Path::new(&destination_folder))
        .parallel_requests(1)
        .build()
        .map_err(|e| Error::Setup(e.to_string()))?;

    return downloader.download(&downloads);
}

fn sync_scarlet_mods(mut cx: FunctionContext) -> JsResult<JsPromise> {
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

    let reporter = Arc::new(ScarletDownloadReporter::new(progress_callback, channel.clone()));

    // Reduce the number of files to only those that don't match their hashes.
    let files = files.into_iter().filter(|file| {
        let file_path = Path::new(&file.path).strip_prefix("/").unwrap();
        let file_path = Path::new(&destination_folder).join(file_path);
        let hash = md5_hash_file(file_path.as_path()).unwrap_or_default();
        hash != file.md5_hash
    }).collect();
    
    std::thread::spawn(move || {
        let result = download_files(repo_url, destination_folder, files, reporter);

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

                                    // Set the URL
                                    let url = cx.string(url);
                                    status_obj.set(&mut cx, "url", url)?;

                                    // Set the status code
                                    let status_code = cx.number(*status_code as f64);
                                    status_obj.set(&mut cx, "statusCode", status_code)?;

                                    // Push the status object to the array
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
                Err(e) => cx.throw_error(e.to_string()),
            }
        });
    });

    Ok(promise)
}

struct ScarletDownloadReporter {
    callback: Arc<Root<JsFunction>>,
    channel: Arc<Channel>,
    last_update: Mutex<std::time::Instant>,
    current_progress: Arc<Mutex<u64>>,
    max_progress: Mutex<Option<u64>>,
    message: Mutex<String>,
}

impl ScarletDownloadReporter {
    fn new(callback: Root<JsFunction>, channel: Channel) -> Self {
        Self {
            callback: Arc::new(callback),
            channel: Arc::new(channel),
            last_update: Mutex::new(std::time::Instant::now()),
            current_progress: Arc::new(Mutex::new(0)),
            max_progress: Mutex::new(None),
            message: Mutex::new(String::new()),
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
        let mut progress = self.current_progress.lock().unwrap();
        *progress = current;
        let mut last_update = self.last_update.lock().unwrap();
        if last_update.elapsed().as_millis() >= 1000 {
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

fn md5_hash_file(path: &Path) -> Result<String, std::io::Error> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Md5::new();
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    hasher.update(&buffer);

    Ok(format!("{:x}", hasher.finalize()))
}

#[neon::main]
fn main(mut cx: ModuleContext) -> NeonResult<()> {
    cx.export_function("md5_hash", md5_hash)?;
    cx.export_function("delete_file", delete_file)?;
    cx.export_function("sync_scarlet_mods", sync_scarlet_mods)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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

        let results = download_files(
            repo_url.to_string(),
            destination_folder.clone(),
            files_to_download,
            reporter
        )
            .expect("Download should succeed");

        assert_eq!(results.len(), 1);
        for summary_result in results {
            match summary_result {
                Ok(summary) => {
                    assert_eq!(summary.verified, Verification::Ok, "File {} should be verified", summary.file_name.display());
                    assert!(!summary.status.is_empty(), "Status should not be empty");
                },
                Err(e) => panic!("Download failed: {:?}", e),
            }
        }
    }

    struct TestReporter;

    impl downloader::progress::Reporter for TestReporter {
        fn setup(&self, _max_progress: Option<u64>, _message: &str) {}
        fn progress(&self, _current: u64) {}
        fn set_message(&self, _message: &str) {}
        fn done(&self) {}
    }
}