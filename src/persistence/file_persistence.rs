use crate::persistence::item_persistence::{ItemStreamReader, ItemStreamWriter};
use crate::persistence::shared_file::{SharedFile, get_shared_file_registry};

use async_trait::async_trait;
use fs2::FileExt;
use quick_xml::Reader;
use quick_xml::events::Event;
use std::fs::{File, OpenOptions};
use std::io::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs::File as TokioFile;
use tokio::io::AsyncWriteExt;

static OUTPUT_FOLDER_PATH: &str = "tmp_outputs";
const CHUNK_SIZE: usize = 8192; // 8KB chunks for reading

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
    shared_file: Arc<SharedFile>,
    current_offset: u64,
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
            .truncate(true)
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
            .read(true) // Need read access for shared file
            .create(true)
            .truncate(true)
            .open(&versioned_path)
            .map_err(|error| format!("Data file open error: {error}"))?;
        data_file
            .try_lock_exclusive()
            .map_err(|_| "Data file is locked.")?;
        data_file.set_len(0).map_err(|error| error.to_string())?;
        data_file.rewind().map_err(|error| error.to_string())?;

        let data_file = TokioFile::from_std(data_file);

        // 4. Create or get shared file for this item/version
        let item_id_clone = item_id.clone();
        let version_clone = *item_version;
        let metadata_path_clone = metadata_path.clone();
        let versioned_path_clone = versioned_path.clone();

        let shared_file =
            get_shared_file_registry().get_or_create(item_id_clone, version_clone, || {
                // Create a new shared file handle
                let file_handle = OpenOptions::new()
                    .read(true)
                    .open(&versioned_path_clone)
                    .map_err(|e| e.to_string())?;
                let tokio_file = TokioFile::from_std(file_handle);

                Ok(SharedFile::new(
                    tokio_file,
                    versioned_path_clone,
                    metadata_path_clone,
                ))
            })?;

        Ok(Self {
            data_file,
            metadata_file,
            item_version: *item_version,
            shared_file,
            current_offset: 0,
        })
    }
}

#[async_trait]
impl ItemStreamWriter for FileWriter {
    async fn write_chunk(&mut self, chunk: Vec<u8>) -> Result<(), String> {
        let chunk_len = chunk.len();
        self.data_file
            .write_all(&chunk)
            .await
            .map_err(|error| format!("Write failed for chunk to file: {error}"))?;
        self.data_file
            .sync_data()
            .await
            .map_err(|error| format!("Failed to sync data to disk: {error}"))?;

        // Update shared file state
        self.current_offset += chunk_len as u64;
        self.shared_file.update_size(self.current_offset);

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

        // Mark shared file as finished
        self.shared_file.mark_finished();

        Ok(())
    }
}

pub struct FileReader {
    shared_file: Arc<SharedFile>,
    current_offset: AtomicU64,
}

impl FileReader {
    pub fn new(item_id: String, item_version: u64) -> Result<Self, String> {
        // Try to get existing shared file from registry (active writer case)
        let shared_file = match get_shared_file_registry().get(&item_id, item_version) {
            Some(sf) => sf,
            None => {
                // File doesn't exist - return 404 Not Found
                return Err("Item not found".to_string());
            }
        };

        Ok(Self {
            shared_file,
            current_offset: AtomicU64::new(0),
        })
    }

    #[allow(dead_code)]
    fn check_is_finished(shared_file: &SharedFile) -> bool {
        let mut metadata_handle = match std::fs::File::open(&shared_file.metadata_path) {
            Ok(handle) => handle,
            Err(_) => return false,
        };
        let mut metadata_bytes = Vec::new();
        metadata_handle.read_to_end(&mut metadata_bytes).ok();
        let mut xml_reader = Reader::from_reader(&metadata_bytes[..]);
        let mut event_buffer = Vec::new();
        while let Ok(xml_event) = xml_reader.read_event_into(&mut event_buffer) {
            if let Event::Start(ref element) = xml_event
                && element.name().as_ref() == b"version"
            {
                let version_string = xml_reader
                    .read_text(element.name())
                    .map_err(|_| "")
                    .unwrap_or_default();
                let _latest_version = version_string.parse::<u64>().unwrap_or(0);
                return true; // If metadata exists with version, it's finished
            }
        }
        false
    }

    fn is_finished(&self) -> bool {
        self.shared_file.is_finished()
    }
}

#[async_trait]
impl ItemStreamReader for FileReader {
    async fn read_chunk(&mut self) -> Result<Option<Vec<u8>>, String> {
        let mut buffer = vec![0u8; CHUNK_SIZE];

        loop {
            let offset = self.current_offset.load(Ordering::Acquire);
            let file_size = self.shared_file.get_size();

            // Check if there's data available to read
            if offset < file_size {
                // Read available data
                let to_read = std::cmp::min(CHUNK_SIZE, (file_size - offset) as usize);

                let bytes_read = self
                    .shared_file
                    .read_at(offset, &mut buffer[..to_read])
                    .await
                    .map_err(|e| e.to_string())?;

                if bytes_read > 0 {
                    self.current_offset
                        .fetch_add(bytes_read as u64, Ordering::Release);
                    buffer.truncate(bytes_read);
                    return Ok(Some(buffer));
                }
            }

            // Check if we're at EOF and file is finished
            if offset >= file_size && self.is_finished() {
                return Ok(None);
            }

            // Wait for new data to be written
            // Use a timeout to prevent indefinite waiting
            let timeout = tokio::time::Duration::from_secs(30);
            let notified =
                tokio::time::timeout(timeout, self.shared_file.write_notify.notified()).await;

            match notified {
                Ok(_) => continue, // Notified of new data, try reading again
                Err(_) => {
                    // Timeout - check if file is finished
                    if self.is_finished() {
                        return Ok(None);
                    }
                    // Otherwise continue waiting
                    continue;
                }
            }
        }
    }
}
