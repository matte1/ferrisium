//! Shared browser raster transport and image decoding helpers.
//!
//! The primary Earth tile pipeline and fixed-zoom secondary body tiler keep
//! separate cache/request state, but both use this module for the mechanical
//! browser fetch bridge, bounded result draining, image decode, and safe URL
//! logging.

use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Mutex;

use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, Image, ImageSampler, ImageType};
use bevy::prelude::*;

/// Channel bridge between async raster fetch callbacks and ECS systems.
#[derive(Resource)]
pub(crate) struct RasterFetchChannel<T> {
    receiver: Mutex<Receiver<T>>,
    sender: Sender<T>,
}

impl<T> Default for RasterFetchChannel<T> {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            receiver: Mutex::new(receiver),
            sender,
        }
    }
}

impl<T> RasterFetchChannel<T> {
    /// Returns the sender cloned into async fetch callbacks.
    pub(crate) fn sender(&self) -> &Sender<T> {
        &self.sender
    }
}

/// Drains at most `max_results` messages from a fetch channel.
///
/// Callers supply the exact result application logic because each pipeline
/// owns different slot states and retry policies.
pub(crate) fn drain_fetch_results<T>(
    channel: &RasterFetchChannel<T>,
    max_results: usize,
    lock_error_message: &str,
    disconnected_message: &str,
    mut apply_result: impl FnMut(T),
) {
    let Ok(receiver) = channel.receiver.lock() else {
        error!("{lock_error_message}");
        return;
    };

    for _ in 0..max_results {
        match receiver.try_recv() {
            Ok(fetch_result) => apply_result(fetch_result),
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => {
                warn!("{disconnected_message}");
                break;
            }
        }
    }
}

/// Starts an HTTP GET and sends the caller's typed result back through `sender`.
pub(crate) fn queue_raster_fetch<T>(
    url: String,
    sender: &Sender<T>,
    build_result: impl FnOnce(ehttp::Result<ehttp::Response>, String) -> T + Send + 'static,
) where
    T: Send + 'static,
{
    let request = ehttp::Request::get(url.clone());
    let sender = sender.clone();

    ehttp::fetch(request, move |result| {
        let _ = sender.send(build_result(result, url));
    });
}

/// Decodes a successful HTTP response into a Bevy image suitable for rendering.
pub(crate) fn decode_raster_image(response: &ehttp::Response) -> Result<Image, String> {
    let image_type = response_image_type(response)
        .ok_or_else(|| format!("unable to determine image type for {}", response.url))?;

    Image::from_buffer(
        &response.bytes,
        image_type,
        CompressedImageFormats::NONE,
        true,
        ImageSampler::linear(),
        RenderAssetUsages::RENDER_WORLD,
    )
    .map_err(|error| error.to_string())
}

/// Infers the raster image type from HTTP content type or URL extension.
fn response_image_type(response: &ehttp::Response) -> Option<ImageType<'_>> {
    if let Some(content_type) = response
        .content_type()
        .and_then(|content_type| content_type.split(';').next())
        .map(str::trim)
        .filter(|content_type| !content_type.is_empty())
    {
        return Some(ImageType::MimeType(content_type));
    }

    let url_without_query = response
        .url
        .split('?')
        .next()
        .unwrap_or(response.url.as_str());
    let (_, extension) = url_without_query.rsplit_once('.')?;
    Some(ImageType::Extension(extension))
}

/// Returns a URL with query parameters removed before logging.
pub(crate) fn redacted_url(url: &str) -> &str {
    url.split_once('?')
        .map_or(url, |(url_without_query, _)| url_without_query)
}

#[cfg(test)]
mod tests {
    use super::redacted_url;

    #[test]
    fn redacted_url_removes_query_parameters() {
        assert_eq!(
            redacted_url("https://tiles.example/1/2/3.jpg?access_token=pk.secret"),
            "https://tiles.example/1/2/3.jpg"
        );
        assert_eq!(
            redacted_url("https://tiles.example/1/2/3.jpg"),
            "https://tiles.example/1/2/3.jpg"
        );
    }
}
