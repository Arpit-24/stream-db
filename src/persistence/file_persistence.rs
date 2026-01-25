use crate::persistence::item_persistence::{ItemStreamReader, ItemStreamWriter};

use async_trait::async_trait;
use fs2::FileExt;
use quick_xml::Reader;
use quick_xml::events::Event;
use std::fs::{File, OpenOptions};
use std::io::prelude::*;
use tokio::fs::File as TokioFile;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

static OUTPUT_FOLDER_PATH: &'static str = "tmp_outputs";
macro_rules! metadata_format {
    () => {
        r#"<metadata>
    <version>{item_version}</version>
</metadata>"#
    };
}

pub fn init() -> Result<(), String> {
    println!("Initializing file persistence");
    std::fs::create_dir_all(OUTPUT_FOLDER_PATH)
        .map_err(|error| format!("Failed to create output directory: {error}"))?;
    Ok(())
}

pub struct FileWriter {
    data_file: TokioFile,
    metadata_file: File,
    item_version: u64,
}

impl FileWriter {
    pub fn new(item_id: &String, item_version: &u64) -> Result<Self, String> {
        let metadata_path = format!("{OUTPUT_FOLDER_PATH}/{item_id}_metadata.xml");
        let versioned_path = format!("{OUTPUT_FOLDER_PATH}/{item_id}_{item_version}.xml");

        // 1. Open & Lock Metadata File
        let mut metadata_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&metadata_path)
            .map_err(|error| format!("Metadata open error: {error}"))?;

        metadata_file
            .try_lock_exclusive()
            .map_err(|_| "Metadata file is locked by another request.")?;

        // 2. Validate Version from Metadata
        let mut meta_bytes = Vec::new();
        metadata_file.read_to_end(&mut meta_bytes).ok(); // ok() handles empty new files

        if !meta_bytes.is_empty() {
            let mut reader = Reader::from_reader(&meta_bytes[..]);
            let mut buffer = Vec::new();
            while let Ok(event) = reader.read_event_into(&mut buffer) {
                match event {
                    Event::Start(ref event) if event.name().as_ref() == b"version" => {
                        let v_str = reader
                            .read_text(event.name())
                            .map_err(|error| error.to_string())?;
                        let current_version: u64 = v_str.parse().unwrap_or(0);
                        if *item_version <= current_version {
                            return Err(format!(
                                "Conflict: Version {item_version} is not newer than {current_version}"
                            ));
                        }
                        break;
                    }
                    Event::Eof => break,
                    _ => (),
                }
            }
        }

        // 3. Open, Lock & Truncate the Data File
        let mut data_file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&versioned_path)
            .map_err(|error| format!("Data file open error: {error}"))?;
        data_file
            .try_lock_exclusive()
            .map_err(|_| "Data file is locked.")?;
        data_file.set_len(0).map_err(|error| error.to_string())?;
        data_file.rewind().map_err(|error| error.to_string())?;

        let data_file = TokioFile::from_std(data_file);

        Ok(Self {
            data_file,
            metadata_file,
            item_version: *item_version,
        })
    }
}

#[async_trait]
impl ItemStreamWriter for FileWriter {
    async fn write_chunk(&mut self, chunk: Vec<u8>) -> Result<(), String> {
        println!("Writing {} bytes", chunk.len());
        self.data_file
            .write_all(&chunk)
            .await
            .map_err(|error| format!("Write failed for chunk to file: {error}"))?;
        self.data_file
            .sync_data()
            .await
            .map_err(|error| format!("Failed to sync data to disk: {error}"))?;
        Ok(())
    }

    fn commit(&mut self) -> Result<(), String> {
        let new_metadata = format!(metadata_format!(), item_version = self.item_version);
        self.metadata_file
            .set_len(0)
            .map_err(|error| error.to_string())?;
        self.metadata_file
            .rewind()
            .map_err(|error| error.to_string())?;
        self.metadata_file
            .write_all(new_metadata.as_bytes())
            .map_err(|error| error.to_string())?;
        self.metadata_file
            .sync_all()
            .map_err(|error| error.to_string())?;
        Ok(())
    }
}

pub struct FileReader {
    data_file: TokioFile,
    metadata_path: String,
    target_version: u64,
}

impl FileReader {
    pub fn new(item_id: String, item_version: u64) -> Result<Self, String> {
        let metadata_path = format!("{}/{}_metadata.xml", OUTPUT_FOLDER_PATH, item_id);
        let data_path = format!("{}/{}_{}.xml", OUTPUT_FOLDER_PATH, item_id, item_version);

        let file_handle = std::fs::File::open(&data_path).map_err(|err| err.to_string())?;
        file_handle.lock_shared().map_err(|err| err.to_string())?;

        Ok(Self {
            data_file: TokioFile::from_std(file_handle),
            metadata_path,
            target_version: item_version,
        })
    }

    fn is_finished(&self) -> bool {
        let mut metadata_handle = match std::fs::File::open(&self.metadata_path) {
            Ok(handle) => handle,
            Err(_) => return false,
        };
        let mut metadata_bytes = Vec::new();
        metadata_handle.read_to_end(&mut metadata_bytes).ok();
        let mut xml_reader = Reader::from_reader(&metadata_bytes[..]);
        let mut event_buffer = Vec::new();
        while let Ok(xml_event) = xml_reader.read_event_into(&mut event_buffer) {
            if let Event::Start(ref element) = xml_event {
                if element.name().as_ref() == b"version" {
                    let version_string = xml_reader
                        .read_text(element.name())
                        .map_err(|_| "")
                        .unwrap_or_default();
                    let latest_version = version_string.parse::<u64>().unwrap_or(0);
                    return latest_version >= self.target_version;
                }
            }
        }
        false
    }
}

#[async_trait]
impl ItemStreamReader for FileReader {
    async fn read_chunk(&mut self) -> Result<Option<Vec<u8>>, String> {
        let mut read_buffer = Vec::with_capacity(1000);
        let mut temp_buffer = [0u8; 1];
        let mut bytes_read = 0;

        loop {
            // Read exactly one byte at a time to check for available data
            match self.data_file.read_exact(&mut temp_buffer).await {
                Ok(_) => {
                    read_buffer.push(temp_buffer[0]);
                    bytes_read += 1;

                    // If we have a full chunk, return it
                    if bytes_read == 1000 {
                        println!("Read chunk: {} bytes", bytes_read);
                        return Ok(Some(read_buffer));
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    // 2. EOF reached: Check if writer is actually done
                    if self.is_finished() {
                        println!("Reader detected finish. Terminating stream.");
                        if bytes_read > 0 {
                            println!("Read chunk: {} bytes", bytes_read);
                            return Ok(Some(read_buffer));
                        }
                        return Ok(None);
                    }
                    // 3. Not finished: Sleep and retry (Tail -f behavior)
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                    continue;
                }
                Err(read_error) => return Err(read_error.to_string()),
            }
        }
    }
}
