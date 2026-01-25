use crate::component::item_stream_component::{self, ItemStreamComponent};

use async_stream::stream;
use axum::{
    body::Body,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};

pub fn init() -> Result<(), String> {
    println!("Initializing read item stream api");
    item_stream_component::init()?;

    Ok(())
}

pub async fn read_item_stream(item_id: String, item_version: u64) -> impl IntoResponse {
    let mut component = match ItemStreamComponent::new_reader(item_id, item_version) {
        Ok(component) => component,
        Err(_) => return (StatusCode::NOT_FOUND, "Item not found").into_response(),
    };

    // Use async-stream to yield chunks back to Axum
    let response_stream = stream! {
        loop {
            match component.read_chunk().await {
                Ok(Some(chunk)) => {
                    println!("Reading {} bytes", chunk.len());
                    // Yielding chunk as-is - ensure it's sent immediately
                    yield Ok::<axum::body::Bytes, std::io::Error>(axum::body::Bytes::from(chunk));
                }
                Ok(None) => {
                    println!("Read complete");
                    break;
                }
                Err(e) => {
                    println!("Read error: {}", e);
                    yield Err(std::io::Error::new(std::io::ErrorKind::Other, e));
                    break;
                }
            }
        }
    };

    let mut headers = HeaderMap::new();
    // Explicitly set chunked transfer encoding to ensure streaming behavior
    headers.insert("Transfer-Encoding", "chunked".parse().unwrap());
    // Disable buffering on both server and proxy
    headers.insert("X-Accel-Buffering", "no".parse().unwrap());
    headers.insert("Cache-Control", "no-cache".parse().unwrap());
    headers.insert("Pragma", "no-cache".parse().unwrap());

    (headers, Body::from_stream(response_stream)).into_response()
}
