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
    // Validate content type is XML
    let content_type = input
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !content_type.contains("xml") {
        return (
            StatusCode::BAD_REQUEST,
            "Content-Type must be application/xml or text/xml",
        )
            .into_response();
    }

    let mut input_stream = input.into_body().into_data_stream();

    let mut component = match ItemStreamComponent::new_writer(item_id.clone(), item_version) {
        Ok(component) => component,
        Err(error) => return (StatusCode::CONFLICT, error).into_response(),
    };

    // Buffer to accumulate partial XML chunks
    let mut xml_buffer = String::new();
    let mut property_count = 0;

    while let Some(chunk) = input_stream.next().await {
        match chunk {
            Ok(bytes) => {
                // Convert bytes to string, handling UTF-8
                if let Ok(chunk_str) = std::str::from_utf8(&bytes) {
                    xml_buffer.push_str(chunk_str);

                    // Try to parse complete property elements from the buffer
                    // We look for complete <property>...</property> elements
                    while let Some(end_tag_pos) = xml_buffer.find("</property>") {
                        // Extract complete property element
                        let property_element = &xml_buffer[..=end_tag_pos];

                        // Write the property to the file without validation
                        // This ensures all XML is written as-is
                        if let Err(error) = component
                            .write_chunk(property_element.as_bytes().to_vec())
                            .await
                        {
                            return (StatusCode::INTERNAL_SERVER_ERROR, error).into_response();
                        }

                        property_count += 1;
                        // Remove the processed element from buffer
                        xml_buffer.drain(..=end_tag_pos);
                    }
                } else {
                    return (StatusCode::BAD_REQUEST, "Invalid UTF-8 in XML data").into_response();
                }
            }
            Err(error) => return (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
        }
    }

    // Handle any remaining data in buffer (incomplete property at end of stream)
    if !xml_buffer.is_empty() {
        // Write any remaining data as-is
        if let Err(error) = component.write_chunk(xml_buffer.as_bytes().to_vec()).await {
            return (StatusCode::INTERNAL_SERVER_ERROR, error).into_response();
        }
        // Count as a property if it looks like a property element
        if xml_buffer.contains("<property") {
            property_count += 1;
        }
    }

    // Check if we received any valid properties
    if property_count == 0 {
        return (
            StatusCode::BAD_REQUEST,
            "No valid property elements found in XML",
        )
            .into_response();
    }

    match component.finalize() {
        Ok(_) => (
            StatusCode::OK,
            format!(
                "Stream processed successfully. {} properties written.",
                property_count
            ),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Write error: {error}"),
        )
            .into_response(),
    }
}
