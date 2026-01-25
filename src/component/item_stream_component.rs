use crate::logic::item_stream_logic::{self, ItemStreamLogic};

pub fn init() -> Result<(), String> {
    println!("Initializing item stream component");
    item_stream_logic::init()?;
    Ok(())
}

pub struct ItemStreamComponent {
    logic: ItemStreamLogic,
}

impl ItemStreamComponent {
    pub fn new_reader(item_id: String, item_version: u64) -> Result<Self, String> {
        Ok(Self {
            logic: ItemStreamLogic::new_reader(item_id, item_version)?,
        })
    }

    pub fn new_writer(item_id: String, item_version: u64) -> Result<Self, String> {
        Ok(Self {
            logic: ItemStreamLogic::new_writer(item_id, item_version)?,
        })
    }

    pub async fn write_chunk(&mut self, input_bytes: Vec<u8>) -> Result<(), String> {
        self.logic.write_chunk(input_bytes).await
    }

    pub async fn read_chunk(&mut self) -> Result<Option<Vec<u8>>, String> {
        self.logic.read_chunk().await
    }

    pub fn finalize(&mut self) -> Result<(), String> {
        self.logic.finalize()
    }
}
