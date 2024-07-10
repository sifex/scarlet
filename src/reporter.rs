use std::sync::{Arc, Mutex};
use neon::handle::Root;
use neon::prelude::*;
use neon::event::Channel;
use std::sync::atomic::Ordering;
use neon::context::Context;
use crate::CANCELLATION_FLAG;

#[derive(Clone, Copy)]
enum DownloadStatus {
    Downloading,
    Cancelled,
    Done,
}

pub struct ScarletDownloadReporter {
    callback: Arc<Root<JsFunction>>,
    channel: Arc<Channel>,
    last_update: Mutex<std::time::Instant>,
    current_progress: Arc<Mutex<u64>>,
    max_progress: Mutex<Option<u64>>,
    message: Mutex<String>,
    status: Arc<Mutex<DownloadStatus>>,
}

impl ScarletDownloadReporter {
    pub(crate) fn new(callback: Root<JsFunction>, channel: Channel) -> Self {
        Self {
            callback: Arc::new(callback),
            channel: Arc::new(channel),
            last_update: Mutex::new(std::time::Instant::now()),
            current_progress: Arc::new(Mutex::new(0)),
            max_progress: Mutex::new(None),
            message: Mutex::new(String::new()),
            status: Arc::new(Mutex::new(DownloadStatus::Downloading)),
        }
    }

    fn call_js_callback(&self, status: DownloadStatus, current: u64, max: Option<u64>, message: String) {
        let callback = self.callback.clone();
        let channel = self.channel.clone();

        channel.send(move |mut cx| {
            let callback = callback.to_inner(&mut cx);
            let this = cx.undefined();
            let status_str = match status {
                DownloadStatus::Downloading => "downloading",
                DownloadStatus::Cancelled => "cancelled",
                DownloadStatus::Done => "done",
            };
            let args = vec![
                cx.string(status_str).upcast(),
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
        self.call_js_callback(DownloadStatus::Downloading, 0, max_progress, message.to_owned());
    }

    fn progress(&self, current: u64) {
        if CANCELLATION_FLAG.load(Ordering::SeqCst) {
            *self.status.lock().unwrap() = DownloadStatus::Cancelled;
            panic!("Download cancelled");
        }

        let mut progress = self.current_progress.lock().unwrap();
        *progress = current;
        let mut last_update = self.last_update.lock().unwrap();
        if last_update.elapsed().as_millis() >= 100 {
            *last_update = std::time::Instant::now();
            let max_progress = *self.max_progress.lock().unwrap();
            let message = self.message.lock().unwrap().clone();
            self.call_js_callback(DownloadStatus::Downloading, current, max_progress, message);
        }
    }

    fn set_message(&self, message: &str) {
        *self.message.lock().unwrap() = message.to_owned();
        let current = *self.current_progress.lock().unwrap();
        let max_progress = *self.max_progress.lock().unwrap();
        self.call_js_callback(DownloadStatus::Downloading, current, max_progress, message.to_owned());
    }

    fn done(&self) {
        *self.status.lock().unwrap() = DownloadStatus::Done;
        let max_progress = *self.max_progress.lock().unwrap();
        let message = self.message.lock().unwrap().clone();
        let current = *self.current_progress.lock().unwrap();
        self.call_js_callback(DownloadStatus::Done, current, max_progress, message);
    }
}