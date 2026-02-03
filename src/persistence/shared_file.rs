use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::fs::File as TokioFile;
use tokio::io::AsyncReadExt;
use tokio::sync::Notify;

/// Shared file state that can be accessed by multiple concurrent readers
/// and a single writer.
pub struct SharedFile {
    /// The underlying file handle (shared for reads)
    pub file_handle: Arc<tokio::sync::RwLock<TokioFile>>,
    /// Current file size in bytes (updated by writer)
    pub file_size: AtomicU64,
    /// Whether the file has been finalized (writer finished)
    pub is_finished: AtomicBool,
    /// Notify readers when new data is available
    pub write_notify: Notify,
    /// Path to the data file
    #[allow(dead_code)]
    pub data_path: String,
    /// Path to the metadata file
    #[allow(dead_code)]
    pub metadata_path: String,
}

impl SharedFile {
    pub fn new(file_handle: TokioFile, data_path: String, metadata_path: String) -> Arc<Self> {
        Arc::new(Self {
            file_handle: Arc::new(tokio::sync::RwLock::new(file_handle)),
            file_size: AtomicU64::new(0),
            is_finished: AtomicBool::new(false),
            write_notify: Notify::new(),
            data_path,
            metadata_path,
        })
    }

    /// Update file size after a write and notify waiting readers
    pub fn update_size(&self, new_size: u64) {
        self.file_size.store(new_size, Ordering::Release);
        // Notify all waiting readers that new data is available
        self.write_notify.notify_waiters();
    }

    /// Mark the file as finished and notify all readers
    pub fn mark_finished(&self) {
        self.is_finished.store(true, Ordering::Release);
        self.write_notify.notify_waiters();
    }

    /// Get the current file size
    pub fn get_size(&self) -> u64 {
        self.file_size.load(Ordering::Acquire)
    }

    /// Check if the file is finished
    pub fn is_finished(&self) -> bool {
        self.is_finished.load(Ordering::Acquire)
    }

    /// Read data from a specific offset
    pub async fn read_at(&self, offset: u64, buffer: &mut [u8]) -> Result<usize, std::io::Error> {
        let mut file = self.file_handle.write().await;
        // Use seek to position at offset
        tokio::io::AsyncSeekExt::seek(&mut *file, std::io::SeekFrom::Start(offset)).await?;
        let bytes_read = file.read(buffer).await?;
        Ok(bytes_read)
    }
}

/// Registry to track shared files by (item_id, version)
pub struct SharedFileRegistry {
    files: Mutex<HashMap<(String, u64), Arc<SharedFile>>>,
}

impl SharedFileRegistry {
    pub fn new() -> Self {
        Self {
            files: Mutex::new(HashMap::new()),
        }
    }

    /// Get or create a shared file entry
    pub fn get_or_create(
        &self,
        item_id: String,
        version: u64,
        create_fn: impl FnOnce() -> Result<Arc<SharedFile>, String>,
    ) -> Result<Arc<SharedFile>, String> {
        let mut files = self.files.lock().unwrap();
        let key = (item_id, version);

        if let Some(shared_file) = files.get(&key) {
            return Ok(shared_file.clone());
        }

        let shared_file = create_fn()?;
        files.insert(key, shared_file.clone());
        Ok(shared_file)
    }

    /// Get an existing shared file
    pub fn get(&self, item_id: &str, version: u64) -> Option<Arc<SharedFile>> {
        let files = self.files.lock().unwrap();
        files.get(&(item_id.to_string(), version)).cloned()
    }

    /// Remove a shared file from the registry
    #[allow(dead_code)]
    pub fn remove(&self, item_id: &str, version: u64) {
        let mut files = self.files.lock().unwrap();
        files.remove(&(item_id.to_string(), version));
    }
}

/// Global registry instance
use std::sync::OnceLock;
static SHARED_FILE_REGISTRY: OnceLock<SharedFileRegistry> = OnceLock::new();

pub fn get_shared_file_registry() -> &'static SharedFileRegistry {
    SHARED_FILE_REGISTRY.get_or_init(SharedFileRegistry::new)
}
