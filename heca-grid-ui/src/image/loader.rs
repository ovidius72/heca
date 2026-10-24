//! One background queue and a weak source cache. No timer, window dependency or fixed asset catalog.
use std::{collections::HashMap, sync::{Arc, Mutex, OnceLock, Weak, mpsc}};
use super::*;

type Sources = HashMap<ImageSource, Weak<Pending<ImageData>>>;
static SOURCES: OnceLock<Mutex<Sources>> = OnceLock::new();
static JOBS: OnceLock<Result<mpsc::Sender<Job>, String>> = OnceLock::new();
enum Job {
    Decode(ImageSource, Arc<Pending<ImageData>>),
    Raster(Arc<ImageData>, RasterSpec, Arc<Pending<ImagePixels>>),
}
fn jobs() -> Result<&'static mpsc::Sender<Job>, ImageError> {
    JOBS.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new().name("heca-images".into()).spawn(move || {
            for job in rx {
                match job {
                    Job::Decode(source, pending) => pending.complete(decode::source(&source)),
                    Job::Raster(data, spec, pending) => pending.complete(decode::raster(&data, spec)),
                }
            }
        }).map_err(|error| error.to_string())?;
        Ok(tx)
    }).as_ref().map_err(|error| ImageError(error.clone()))
}
pub(super) fn load(source: ImageSource, wake: FrameWake) -> ImageHandle {
    let mut cache = SOURCES.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    cache.retain(|_, value| value.strong_count() > 0);
    if let Some(pending) = cache.get(&source).and_then(Weak::upgrade) {
        pending.subscribe(wake); return ImageHandle(pending);
    }
    let pending = Arc::new(Pending::new()); pending.subscribe(wake);
    cache.insert(source.clone(), Arc::downgrade(&pending));
    if let Err(error) = jobs().and_then(|jobs| jobs.send(Job::Decode(source, pending.clone())).map_err(|e| ImageError(e.to_string()))) {
        pending.complete(Err(error));
    }
    ImageHandle(pending)
}
pub(super) fn raster(data: Arc<ImageData>, spec: RasterSpec, pending: Arc<Pending<ImagePixels>>) {
    if let Err(error) = jobs().and_then(|jobs| jobs.send(Job::Raster(data, spec, pending.clone())).map_err(|e| ImageError(e.to_string()))) {
        pending.complete(Err(error));
    }
}
