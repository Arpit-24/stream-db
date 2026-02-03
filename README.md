# stream-db

A high-performance streaming database written in Rust that enables real-time write and read operations with concurrent access support.

## What is stream-db?

stream-db is a streaming database that allows you to:
- **Write structured property data** in real-time using HTTP streams
- **Read data** as it's being written by multiple concurrent readers
- **Support multiple readers** consuming the same stream simultaneously
- **Store typed property data** with explicit type information

Unlike traditional databases that require complete transactions before reading, stream-db enables real-time data access scenarios where readers can start consuming data before the writer finishes.

## Quick Start

```bash
# Build and run the server
cargo build
cargo run

# In another terminal - write data
curl -X POST http://localhost:3000/write-item-stream/myitem/1 \
  -H "Content-Type: application/xml" \
  -d '<property for="name"><string>Test</string></property>'

# In another terminal - read data
curl -N http://localhost:3000/read-item-stream/myitem/1
```

## Architecture

```
┌─────────────┐         ┌─────────────┐         ┌─────────────┐
│   Writer    │ ──────► │   Server    │ ──────► │   Readers   │
│  (HTTP)     │  XML    │  (Axum)     │  Stream │  (Multiple) │
└─────────────┘         └─────────────┘         └─────────────┘
                                │
                                ▼
                       ┌─────────────┐
                       │ File Storage│
                       │   (XML)     │
                       └─────────────┘
```

## API Endpoints

### Write API

**Endpoint**: `POST /write-item-stream/{item_id}/{version}`

**Content-Type**: `application/xml`

**Description**: Stream property data to an item. Properties can be sent in chunks.

**Example**:
```bash
curl -X POST http://localhost:3000/write-item-stream/test_item/1 \
  -H "Content-Type: application/xml" \
  -d '<property for="name"><string>John Doe</string></property>'
```

**Response Codes**:
- `200 OK`: Stream processed successfully
- `400 Bad Request`: Invalid XML or property format
- `409 Conflict`: Version conflict
- `500 Internal Server Error`: Write error

### Read API

**Endpoint**: `GET /read-item-stream/{item_id}/{version}`

**Description**: Stream item data as it's being written. Multiple readers can consume data simultaneously.

**Example**:
```bash
curl -N http://localhost:3000/read-item-stream/user123/1
```

## Data Format

### Property XML Format

Properties use a typed XML format with explicit type information:

```xml
<item>
  <property for="name"><string>John Doe</string></property>
  <property for="age"><number>25</number></property>
  <property for="active"><boolean>true</boolean></property>
  <property for="created"><datetime>2024-01-15T10:30:00Z</datetime></property>
  <property for="avatar"><binary>base64encodeddata</binary></property>
</item>
```

### Supported Types

- `string`: Text values
- `number`: Numeric values (floating point)
- `boolean`: True/false values
- `datetime`: ISO 8601 formatted timestamps
- `binary`: Base64-encoded binary data

## Streaming Examples

### Rate-Limited Streaming Writer

Test streaming with a slow writer using rate limiting:

```bash
# Start rate-limited writer (5000 bytes/sec)
curl -X POST http://localhost:3000/write-item-stream/test/1 \
  -H "Content-Type: application/xml" \
  --limit-rate 5000 \
  --data-binary @tmp_inputs/large_stream.xml
```

### Concurrent Readers

Test multiple readers consuming the same stream:

```bash
# Terminal 1: Start 6 readers
for i in {1..6}; do
  curl -N "http://localhost:3000/read-item-stream/test_item/1" > "/tmp/reader_${i}.log" &
done

# Terminal 2: Start writer
for i in {1..20}; do
  echo '<property for="item_'${i}'"><string>Value '${i}'</string></property>'
  sleep 0.1
done | curl -X POST http://localhost:3000/write-item-stream/test_item/1 \
  -H "Content-Type: application/xml" \
  --data-binary @-
```

### Full Concurrent Test

Run the included test script that demonstrates true streaming:

```bash
bash test_concurrent.sh
```

This test:
1. Starts a rate-limited writer (5000 bytes/sec)
2. Waits 2 seconds
3. Starts 6 readers while writer is still streaming
4. All readers receive complete data concurrently

Sample output:
```
[23:02:39.3N] Writer started
[23:02:41.3N] Readers 1-6 started (while writer still streaming)
[23:03:02.3N] Writer finished (~20 seconds later)
[23:03:05.3N] All 6 readers received 1000 lines each ✓
```

## Storage Structure

Each item is stored in two files in `tmp_outputs/`:

1. **Data File** (`{item_id}_{version}.xml`)
   - Contains the actual property data in XML format
   - Append-only structure for streaming writes

2. **Metadata File** (`{item_id}_metadata.xml`)
   - Contains version information
   - Used to track completion status

## Features

- **Concurrent Access**: Multiple readers can consume data while it's being written
- **Type Safety**: Explicit type information for all properties
- **Streaming**: Support for large datasets through chunked transfers
- **Atomic Operations**: Thread-safe file operations using `fs2` for file locking
- **Async I/O**: Built on Tokio for efficient asynchronous operations
- **Real-time**: Readers receive data as it arrives, not after completion

## Project Structure

```
stream-db/
├── Cargo.toml              # Project dependencies
├── README.md               # This file
├── LICENSE                 # License information
├── src/
│   ├── main.rs            # Application entry point
│   ├── api/
│   │   ├── mod.rs
│   │   ├── write_item_stream_api.rs
│   │   └── read_item_stream_api.rs
│   ├── component/
│   │   ├── mod.rs
│   │   └── item_stream_component.rs
│   ├── logic/
│   │   ├── mod.rs
│   │   └── item_stream_logic.rs
│   ├── persistence/
│   │   ├── mod.rs
│   │   ├── file_persistence.rs
│   │   ├── item_persistence.rs
│   │   ├── property_serialization.rs
│   │   └── shared_file.rs
│   └── types.rs           # Property and data types
```

## Development

### Building

```bash
cargo build
```

### Running Tests

```bash
# Run the concurrent streaming test
bash test_concurrent.sh

# Standard cargo tests
cargo test
```

### Linting

```bash
cargo clippy -- -D warnings
```

### Formatting

```bash
cargo fmt
```

## Future Enhancements

- **Schema Validation**: Define and enforce schemas per item type
- **Indexing and Search**: Fast property lookup and type-aware queries
- **Compression**: Compressed storage for large binary values
- **Authentication**: Secure access control for API endpoints
- **Query API**: Search and filter items by property values

## License

See [LICENSE](LICENSE) file for details.
