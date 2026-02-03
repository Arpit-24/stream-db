mod api;
mod component;
mod logic;
mod persistence;
mod types;

use api::write_item_stream_api;

use axum::{
    Router,
    body::Body,
    extract::Path,
    http::Request,
    routing::{get, post},
};

use crate::api::read_item_stream_api;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    write_item_stream_api::init()
        .map_err(|error| format!("Could not initialize write item stream api: {:?}", error))?;
    read_item_stream_api::init()
        .map_err(|error| format!("Could not initialize read item stream api: {:?}", error))?;

    let app = Router::new()
        .route(
            "/write-item-stream/{item_id}/{version}",
            post(
                |path: Path<(String, u64)>, request: Request<Body>| async move {
                    write_item_stream_api::write_item_stream(path.0.0, path.0.1, request).await
                },
            ),
        )
        .route(
            "/read-item-stream/{item_id}/{version}",
            get(|path: Path<(String, u64)>| async move {
                read_item_stream_api::read_item_stream(path.0.0, path.0.1).await
            }),
        );

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("Server listening on http://0.0.0.0:3000");

    axum::serve(listener, app).await?;
    Ok(())
}
