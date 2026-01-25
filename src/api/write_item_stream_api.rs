use crate::component::item_stream_component::{self, ItemStreamComponent};

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::IntoResponse,
};
use futures::StreamExt;

pub fn init() -> Result<(), String> {
    println!("Initializing write item stream api");
    item_stream_component::init()?;

    Ok(())
}

pub async fn write_item_stream(
    item_id: String,
    item_version: u64,
    input: Request<Body>,
) -> impl IntoResponse {
    let mut input_stream = input.into_body().into_data_stream();

    let mut component = match ItemStreamComponent::new_writer(item_id, item_version) {
        Ok(component) => component,
        Err(error) => return (StatusCode::CONFLICT, error).into_response(),
    };

    while let Some(chunk) = input_stream.next().await {
        match chunk {
            Ok(bytes) => {
                // Pass ownership of bytes down the chain
                if let Err(error) = component.write_chunk(bytes.to_vec()).await {
                    return (StatusCode::INTERNAL_SERVER_ERROR, error).into_response();
                }
            }
            Err(error) => return (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
        }
    }

    match component.finalize() {
        Ok(_) => (StatusCode::OK, "Stream processed successfully").into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Write error: {error}"),
        )
            .into_response(),
    }
}
