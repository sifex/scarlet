mod tests;
mod reporter;

use std::{fs, thread};
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use downloader::{Download, Downloader, DownloadSummary, Error, Verification};
use downloader::progress::Reporter;
use md5::{Digest, Md5};
use neon::prelude::*;
use reporter::ScarletDownloadReporter;

struct FileToDownload {
    pub path: String,
    pub md5_hash: String,
}

// Global cancellation flag
static CANCELLATION_FLAG: AtomicBool = AtomicBool::new(false);

fn start_download(mut cx: FunctionContext) -> JsResult<JsPromise> {
    CANCELLATION_FLAG.store(false, Ordering::SeqCst);
    
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

    thread::spawn(move || {
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

    Ok(promise)
}

fn stop_download(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    CANCELLATION_FLAG.store(true, Ordering::SeqCst);

    Ok(cx.undefined())
}

fn download_files(
    repo_url: String,
    destination_folder: String,
    files: Vec<FileToDownload>,
    reporter: Arc<dyn downloader::progress::Reporter + Send + Sync>
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
        .collect();

    let mut downloader = Downloader::builder()
        .download_folder(Path::new(&destination_folder))
        .parallel_requests(1)
        .build()?;

    reporter.set_message("Downloading files");
        
    let results = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        downloader.download(&*downloads)
    }));

    let results: Result<Vec<Result<DownloadSummary, Error>>, Error> = results.unwrap_or_else(|_e| Err(Error::Setup("Download cancelled".to_string())));

    reporter.set_message("Finalising Results");
    
    // If we find an item that matches downloader::Error::Download(DownloadSummary), we need to verify that the file was downloaded
    // and the hash is correct. We can map the error to a Result<DownloadSummary, Error> and provide the verification status back to the client.
    // For errors that aren't downloader::Error::Download(DownloadSummary), we can just return the error as is
    
    let results = results.map(|results| {
        results.into_iter().map(|result| {
            match result {
                Ok(summary) => Ok(summary),
                Err(Error::Download(mut summary)) => {
                    // Here we can get the hash from the downloads array and verify the file
                    let file = downloads.iter().find(|d| d.file_name == summary.file_name).unwrap();
                    let verify_callback = file.verify_callback.clone();
                    
                    let verification = verify_callback(summary.file_name.clone(), &|_| {});
                    
                    match verification {
                        Verification::Ok => {
                            summary.verified = Verification::Ok;
                        },
                        Verification::Failed => {
                            summary.verified = Verification::Failed;
                        },
                        _ => {
                            summary.verified = Verification::NotVerified;
                        }
                    }
                    
                    summary.status.push((summary.file_name.to_str().unwrap_or("").to_string(), 200));
                    
                    Ok(summary)
                },
                Err(e) => Err(e),
            }
        }).collect()
    });

    reporter.set_message("Finished Downloading");
    reporter.done();
    
    return results;
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


