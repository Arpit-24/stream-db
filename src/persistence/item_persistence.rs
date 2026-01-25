use async_trait::async_trait;

#[async_trait]
pub trait ItemStreamWriter: Send + Sync {
    async fn write_chunk(&mut self, chunk: Vec<u8>) -> Result<(), String>;
    fn commit(&mut self) -> Result<(), String>;
}

#[async_trait]
pub trait ItemStreamReader: Send + Sync {
    async fn read_chunk(&mut self) -> Result<Option<Vec<u8>>, String>;
}
