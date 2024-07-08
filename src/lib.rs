use std::fs;
use std::path::{Path, PathBuf};
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

#[neon::main]
fn main(mut cx: ModuleContext) -> NeonResult<()> {
    cx.export_function("md5_hash", md5_hash)?;
    cx.export_function("delete_file", delete_file)?;
    cx.export_function("download_file", download_file)?;
    Ok(())
}

fn md5_hash(mut cx: FunctionContext) -> JsResult<JsString> {
    let file_path = cx.argument::<JsString>(0)?.value(&mut cx);
    let mut file = fs::File::open(file_path).or_else(|e| cx.throw_error(e.to_string()))?;
    let mut hasher = Md5::new();
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).or_else(|e| cx.throw_error(e.to_string()))?;
    hasher.update(&buffer);
    let result = format!("{:x}", hasher.finalize());
    Ok(cx.string(result))
}

fn delete_file(mut cx: FunctionContext) -> JsResult<JsBoolean> {
    let file_path = cx.argument::<JsString>(0)?.value(&mut cx);
    match fs::remove_file(file_path) {
        Ok(_) => Ok(cx.boolean(true)),
        Err(_) => Ok(cx.boolean(false))
    }
}

fn download_file(mut cx: FunctionContext) -> JsResult<JsPromise> {
    let download_url = cx.argument::<JsString>(0)?.value(&mut cx);
    let destination_path = cx.argument::<JsString>(1)?.value(&mut cx);
    let expected_md5_hash = cx.argument::<JsString>(2)?.value(&mut cx);
    let progress_callback = cx.argument::<JsFunction>(3)?;

    let destination_path = PathBuf::from(destination_path);
    if let Some(parent) = destination_path.parent() {
        fs::create_dir_all(parent).or_else(|e| cx.throw_error(e.to_string()))?;
    }

    let (deferred, promise) = cx.promise();
    let channel = cx.channel();
    let rt = runtime(&mut cx)?;

    let mut downloader = Downloader::builder()
        .parallel_requests(1)
        .build()
        .unwrap();

    let dl = Download::new(&download_url)
        .file_name(destination_path.as_path())
        .progress(Arc::new(ScarletDownloadReporter::new(progress_callback.root(&mut cx))));

    rt.spawn(async move {
        deferred.settle_with(&channel, move |mut cx| {
            let result = downloader.download(&[dl]).unwrap();
            match result.first().unwrap() {
                Err(e) => cx.throw_error(e.to_string()),
                Ok(s) => {
                    print!("Download completed");
                    let downloaded_hash = md5_hash_file(&destination_path).unwrap();

                    // if downloaded_hash == expected_md5_hash {
                    //     Ok(cx.string("Download completed and verified"))
                    // } else {
                    //     fs::remove_file(destination_path).unwrap();
                    //     cx.throw_error("MD5 hash mismatch")
                    // }
                    Ok(cx.string(downloaded_hash))
                },
            }
        });
    });

    Ok(promise)
}

struct ScarletDownloadReporter {
    callback: Arc<Root<JsFunction>>,
    last_update: Mutex<std::time::Instant>,
    max_progress: Mutex<Option<u64>>,
    message: Mutex<String>,
}

impl ScarletDownloadReporter {
    fn new(callback: Root<JsFunction>) -> Self {
        Self {
            callback: Arc::new(callback),
            last_update: Mutex::new(std::time::Instant::now()),
            max_progress: Mutex::new(None),
            message: Mutex::new(String::new()),
        }
    }
}

impl downloader::progress::Reporter for ScarletDownloadReporter {
    fn setup(&self, max_progress: Option<u64>, message: &str) {
        *self.max_progress.lock().unwrap() = max_progress;
        *self.message.lock().unwrap() = message.to_owned();
    }

    fn progress(&self, current: u64) {
        // Update the progress bar only every 100ms
    }

    fn set_message(&self, message: &str) {
        *self.message.lock().unwrap() = message.to_owned();
    }

    fn done(&self) {
        // You can add any cleanup logic here if needed
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