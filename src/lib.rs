use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use downloader::{Download, Downloader};
use neon::prelude::*;
use md5::{Md5, Digest};
use tokio::runtime::Runtime;
use std::io::Read;

// Return a global tokio runtime or create one if it doesn't exist.
fn runtime<'a, C: Context<'a>>(cx: &mut C) -> NeonResult<&'static Runtime> {
    static RUNTIME: once_cell::sync::OnceCell<Runtime> = once_cell::sync::OnceCell::new();
    RUNTIME.get_or_try_init(|| Runtime::new().or_else(|err| cx.throw_error(err.to_string())))
}

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

fn sync_scarlet_mods(mut cx: FunctionContext) -> JsResult<JsPromise> {
    let repo_url = cx.argument::<JsString>(0)?.value(&mut cx);
    let destination_folder = cx.argument::<JsString>(1)?.value(&mut cx);
    let files_to_download = cx.argument::<JsArray>(2)?;
    let progress_callback = cx.argument::<JsFunction>(3)?;

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
    let rt = runtime(&mut cx)?;

    let downloads: Vec<Download> = files
        .into_iter()
        .map(|file| {
            let full_url = format!("{}{}", repo_url, file.path);
            let file_path = Path::new(&destination_folder).join(&file.path);
            let progress = Arc::new(ScarletDownloadReporter::new(
                progress_callback.root(&mut cx),
                cx.channel(),
            ));

            return Download::new(&full_url)
                .file_name(&file_path)
                .progress(progress);
        })
        .collect();

    let mut downloader = Downloader::builder()
        .download_folder(Path::new(&destination_folder))
        .parallel_requests(5)
        .build()
        .expect("Failed to create downloader");

    rt.spawn(async move {

        let results = tokio::task::spawn_blocking(move || {
            downloader.download(&downloads)
        }).await.unwrap();

        deferred.settle_with(&channel, move |mut cx| {
            match results {
                Ok(summaries) => {
                    let js_results = JsArray::new(&mut cx, summaries.len());
                    for (i, summary) in summaries.iter().enumerate() {
                        match summary {
                            Ok(summary) => {
                                let obj = cx.empty_object();
                                let file_name = cx.string(summary.file_name.to_str().unwrap_or(""));
                                obj.set(&mut cx, "fileName", file_name)?;
                                let verified = cx.boolean(summary.verified == downloader::Verification::Ok);
                                obj.set(&mut cx, "verified", verified)?;
                                js_results.set(&mut cx, i as u32, obj)?;
                            }
                            Err(e) => {
                                let obj = cx.empty_object();
                                let error = cx.string(e.to_string());
                                obj.set(&mut cx, "error", error)?;
                                js_results.set(&mut cx, i as u32, obj)?;
                            }
                        }
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

    // print
    println!("{:x}", hasher.clone().finalize());

    Ok(format!("{:x}", hasher.finalize()))
}

#[neon::main]
fn main(mut cx: ModuleContext) -> NeonResult<()> {
    cx.export_function("md5_hash", md5_hash)?;
    cx.export_function("delete_file", delete_file)?;
    cx.export_function("sync_scarlet_mods", sync_scarlet_mods)?;
    Ok(())
}
