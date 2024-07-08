use std::cmp::min;
use std::fmt::{format};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use downloader::{Download, Downloader};
use neon::prelude::*;
use md5::{Md5, Digest};
use neon::object::PropertyKey;
use neon::prelude::*;
use serde::Deserialize;
use tokio::runtime::Runtime;

// Return a global tokio runtime or create one if it doesn't exist.
// Throws a JavaScript exception if the `Runtime` fails to create.
fn runtime<'a, C: Context<'a>>(cx: &mut C) -> NeonResult<&'static Runtime> {
    static RUNTIME: once_cell::sync::OnceCell<Runtime> = once_cell::sync::OnceCell::new();

    RUNTIME.get_or_try_init(|| Runtime::new().or_else(|err| cx.throw_error(err.to_string())))
}

#[neon::main]
fn main(mut cx: ModuleContext) -> NeonResult<()> {
    // cx.export_function("md5_hash", md5_hash)?;
    cx.export_function("delete_file", delete_file)?;
    cx.export_function("download_file", download_file)?;
    Ok(())
}

// fn md5_hash(mut cx: FunctionContext) -> JsResult<JsString> {
//     let file_hash_path_attribute = cx.argument::<JsString>(0)?.value(&mut cx);
//
//     let f = File::open(file_hash_path_attribute).unwrap();
//     let mut hasher = Md5::new();
//     let hash = hasher.update(f.bytes()). ;
//
//     return Ok(cx.string(format!("{}", hash)));
// }

fn delete_file(mut cx: FunctionContext) -> JsResult<JsBoolean> {
    let destination_path_attribute = cx.argument::<JsString>(0)?.value(&mut cx);

    match fs::remove_file(destination_path_attribute) {
        Ok(_) => Ok(cx.boolean(true)),
        Err(_) => Ok(cx.boolean(false))
    }
}

fn download_file(mut cx: FunctionContext) -> JsResult<JsPromise> {
    // Arguments
    let download_urls = cx.argument::<JsArray>(0)?.value(&mut cx);

    let destination_path_attribute = cx.argument::<JsString>(1)?.value(&mut cx);
    let expected_md5_hash = cx.argument::<JsString>(2)?.value(&mut cx);
    let progress_callback = cx.argument::<JsFunction>(3)?;

    // Create dir if it doesn't exist
    let destination_path = PathBuf::from(destination_path_attribute.to_string());
    let file_directory = destination_path.parent().unwrap();
    let _ = fs::create_dir_all(file_directory.to_str().unwrap());

    // Javascript Promise
    let (deferred, promise) = cx.promise();
    let channel = cx.channel();
    let rt = runtime(&mut cx)?;

    // Downloader
    let mut downloader = Downloader::builder()
        .parallel_requests(1)
        .build()
        .unwrap();

    let dl = Download::new(&*download_url)
        .file_name(destination_path_attribute.as_ref());

    let dl = dl.progress(ScarletDownloadReporter::create());

    #[cfg(feature = "verify")]
        let dl = {
        use downloader::verify;
        fn decode_hex(s: String) -> Result<Vec<u8>, std::num::ParseIntError> {
            (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
                .collect()
        }
        dl.verify(verify::with_digest::<md5::Md5>(
            decode_hex(expected_md5_hash).unwrap(),
        ))
    };

    let _ = rt.spawn(async move {
        deferred.settle_with(&channel, move |mut cx| {
            #[cfg(not(feature = "tui"))]

            let result = downloader.download(&[dl]).unwrap();

            match result.first().unwrap() {
                Err(e) => cx.throw_error(e.to_string()),
                Ok(s) => Ok(cx.string(s.status.first().unwrap().0.clone())),
            }
        });
    });

    Ok(promise)
}

// Define a custom progress reporter:
struct SimpleReporterPrivate {
    last_update: std::time::Instant,
    max_progress: Option<u64>,
    message: String,
}

struct ScarletDownloadReporter {
    private: std::sync::Mutex<Option<SimpleReporterPrivate>>
}

impl<T> ScarletDownloadReporter {
    #[cfg(not(feature = "tui"))]
    fn create() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            private: std::sync::Mutex::new(None)
        })
    }
}

impl downloader::progress::Reporter for ScarletDownloadReporter {
    fn setup(&self, max_progress: Option<u64>, message: &str) {
        let private = SimpleReporterPrivate {
            last_update: std::time::Instant::now(),
            max_progress,
            message: message.to_owned(),
        };

        let mut guard = self.private.lock().unwrap();
        *guard = Some(private);
    }

    fn progress(&self, current: u64) {
        if let Some(p) = self.private.lock().unwrap().as_mut() {
            let max_bytes = match p.max_progress {
                Some(bytes) => format!("{:?}", bytes),
                None => "{unknown}".to_owned(),
            };
            if p.last_update.elapsed().as_millis() >= 1000 {
                println!(
                    "test file: {} of {} bytes. [{}]",
                    current, max_bytes, p.message
                );
                p.last_update = std::time::Instant::now();
            }
        }
    }

    fn set_message(&self, message: &str) {
        println!("test file: Message changed to: {}", message);
    }

    fn done(&self) {
        let mut guard = self.private.lock().unwrap();
        *guard = None;
        println!("test file: [DONE]");
    }
}
