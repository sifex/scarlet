use std::sync::Arc;

use lazy_static::lazy_static;
use neon::prelude::*;
use tokio::runtime::Runtime;

use crate::download::{DownloadManager, FileToDownload};

mod download;
mod test;
// mod test;

lazy_static! {
    static ref DOWNLOAD_MANAGER: Arc<DownloadManager> = Arc::new(DownloadManager::new());
    static ref RUNTIME: Runtime = Runtime::new().unwrap();
}

fn start_download(mut cx: FunctionContext) -> JsResult<JsPromise> {
    let destination = cx.argument::<JsString>(0)?.value(&mut cx);
    let files_array = cx.argument::<JsArray>(1)?;

    let files: Vec<FileToDownload> = files_array
        .to_vec(&mut cx)?
        .into_iter()
        .map(|v| {
            let obj = v.downcast::<JsObject, _>(&mut cx).unwrap();
            FileToDownload {
                url: obj
                    .get::<JsString, _, _>(&mut cx, "url")
                    .unwrap()
                    .value(&mut cx),
                path: obj
                    .get::<JsString, _, _>(&mut cx, "path")
                    .unwrap()
                    .value(&mut cx),
                sha256_hash: obj
                    .get::<JsString, _, _>(&mut cx, "sha256_hash")
                    .unwrap()
                    .value(&mut cx),
            }
        })
        .collect();

    let manager = DOWNLOAD_MANAGER.clone();
    let (deferred, promise) = cx.promise();
    let channel = cx.channel();

    RUNTIME.spawn(async move {
        let result = manager.download(destination, files).await;
        deferred.settle_with(&channel, move |mut cx| match result {
            Ok(()) => Ok(cx.boolean(true)),
            Err(e) => cx.throw_error(e.to_string()),
        });
    });

    Ok(promise)
}

fn stop_download(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    DOWNLOAD_MANAGER.cancel();

    Ok(cx.undefined())
}

fn get_progress(mut cx: FunctionContext) -> JsResult<JsPromise> {
    let manager = DOWNLOAD_MANAGER.clone();

    let (deferred, promise) = cx.promise();
    let channel = cx.channel();

    RUNTIME.spawn(async move {
        let progress = manager.get_progress().await;
        deferred.settle_with(&channel, move |mut cx| {
            let obj = cx.empty_object();
            let status = cx.string(format!("{:?}", progress.status));
            obj.set(&mut cx, "status", status)?;
            let files_total = cx.number(progress.files_total as f64);
            obj.set(&mut cx, "filesTotal", files_total)?;
            let files_total_completed = cx.number(progress.files_total_completed as f64);
            obj.set(&mut cx, "filesTotalCompleted", files_total_completed)?;
            let verification_total_completed =
                cx.number(progress.verification_total_completed as f64);
            obj.set(
                &mut cx,
                "verificationTotalCompleted",
                verification_total_completed,
            )?;
            let current_file_downloaded = cx.number(progress.current_file_downloaded as f64);
            obj.set(&mut cx, "currentFileDownloaded", current_file_downloaded)?;
            let current_file_total_size = cx.number(progress.current_file_total_size as f64);
            obj.set(&mut cx, "currentFileTotalSize", current_file_total_size)?;
            let current_file_path = cx.string(&progress.current_file_path);
            obj.set(&mut cx, "currentFilePath", current_file_path)?;
            Ok(obj)
        });
    });

    Ok(promise)
}

fn ping(mut cx: FunctionContext) -> JsResult<JsString> {
    Ok(cx.string("pong"))
}

#[neon::main]
fn main(mut cx: ModuleContext) -> NeonResult<()> {
    cx.export_function("start_download", start_download)?;
    cx.export_function("stop_download", stop_download)?;
    cx.export_function("get_progress", get_progress)?;
    cx.export_function("ping", ping)?;
    Ok(())
}
