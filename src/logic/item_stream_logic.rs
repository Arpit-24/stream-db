use crate::persistence::file_persistence::{self, FileReader, FileWriter};
use crate::persistence::item_persistence::{ItemStreamReader, ItemStreamWriter};

pub fn init() -> Result<(), String> {
    println!("Initializing item stream logic");
    file_persistence::init()?;
    Ok(())
}

pub struct ItemStreamLogic {
    reader: Option<Box<dyn ItemStreamReader>>,
    writer: Option<Box<dyn ItemStreamWriter>>,
}

impl ItemStreamLogic {
    pub fn new_reader(item_id: String, item_version: u64) -> Result<Self, String> {
        let reader = FileReader::new(item_id, item_version)?;
        Ok(ItemStreamLogic {
            reader: Some(Box::new(reader)),
            writer: None,
        })
    }

    pub fn new_writer(item_id: String, item_version: u64) -> Result<Self, String> {
        let writer = FileWriter::new(&item_id, &item_version)?;
        Ok(ItemStreamLogic {
            reader: None,
            writer: Some(Box::new(writer)),
        })
    }

    pub async fn write_chunk(&mut self, chunk: Vec<u8>) -> Result<(), String> {
        if let Some(ref mut writer) = self.writer {
            writer.write_chunk(chunk).await
        } else {
            Err("Writer not initialized".into())
        }
    }

    pub async fn read_chunk(&mut self) -> Result<Option<Vec<u8>>, String> {
        if let Some(ref mut reader) = self.reader {
            reader.read_chunk().await
        } else {
            Err("Reader not initialized".into())
        }
    }

    pub fn finalize(&mut self) -> Result<(), String> {
        if let Some(ref mut writer) = self.writer {
            writer.commit().map_err(|error| {
                format!("Error while persisting the update, item is not written: {error}")
            })
        } else {
            Ok(())
        }
    }
}
