# stream-db

A high-performance streaming database written in Rust that enables real-time write and read operations with concurrent access support.

## What is stream-db?

stream-db is a streaming database that allows you to:
- **Write data** in real-time using HTTP streams
- **Read data** as it's being written
- **Support multiple concurrent readers** consuming the same stream
- **Store structured property data** with explicit type information

Unlike traditional databases that require complete transactions before reading, stream-db enables real-time data access scenarios where readers can start consuming data before the writer finishes.

## How It Works

### Architecture Overview

```
┌─────────────┐         ┌─────────────┐         ┌─────────────┐
│   Writer    │ ──────► │   Server    │ ──────► │   Readers   │
│  (HTTP)     │         │  (Tokio)    │         │  (Streams)  │
└─────────────┘         └─────────────┘         └─────────────┘
                                │
                                ▼
                       ┌─────────────┐
                       │ File Storage│
                       │   (XML)     │
                       └─────────────┘
```

### Storage Structure

Each item is stored in two files:

1. **Metadata File** (`{item_id}_{version}.meta`)
   - Contains item metadata including item ID, version, creation timestamp
   - Property index for fast lookups
   - File size tracking

2. **Data File** (`{item_id}_{version}.data`)
   - Contains the actual property data in XML format
   - Append-only structure for streaming writes

### Streaming Behavior

1. **Writer** sends property data in chunks via HTTP POST
2. **Server** parses and validates each XML chunk
3. **File Writer** appends valid properties to the data file
4. **Shared File Registry** tracks file state and notifies readers
5. **Readers** are notified when new data is available and can stream it in real-time

### Concurrency Model

- **File Locking**: Uses `fs2` for atomic file operations
- **Async I/O**: Built on Tokio for efficient asynchronous operations
- **Shared Registry**: Coordinates between writers and readers
- **Notification System**: Readers are notified when new data arrives

## API Endpoints

### Write API

**Endpoint**: `POST /write-item-stream/{item_id}/{version}`

**Content-Type**: `application/xml`

**Description**: Stream property data to an item. Properties can be sent in multiple chunks.

**Example**:
```bash
curl -X POST http://localhost:3000/write-item-stream/test_item/1 \
  -H "Content-Type: application/xml" \
  -d '<property for="name"><string>Test User</string></property>'
```

**Response Codes**:
- `200 OK`: Stream processed successfully
- `400 Bad Request`: Invalid XML or property format
- `409 Conflict`: Version conflict
- `500 Internal Server Error`: Write error

### Read API

**Endpoint**: `GET /read-item-stream/{item_id}/{version}`

**Description**: Stream item data as it's being written. Readers receive raw bytes from the data file.

**Example**:
```bash
curl -N http://localhost:3000/read-item-stream/user123/1
```

**Features**:
- Multiple readers can consume data while it's being written
- Readers are notified when new data arrives
- Supports long-lived connections for real-time streaming

## Data Format

### Property XML Format

Properties are stored in typed XML format:

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
- `number`: Numeric values (integers and floats)
- `boolean`: True/false values
- `datetime`: ISO 8601 formatted timestamps
- `binary`: Base64-encoded binary data

## Features

- **Concurrent Access**: Multiple readers can consume data while it's being written
- **Type Safety**: Explicit type information for all properties
- **Streaming**: Support for large datasets through chunked transfers
- **Atomic Operations**: Thread-safe file operations using `fs2` for file locking
- **Async I/O**: Built on Tokio for efficient asynchronous operations
- **Property Indexing**: Metadata includes property index for future search capabilities

## Technology Stack

- **Rust 2024 Edition**: Modern Rust with latest language features
- **Tokio**: Async runtime for efficient I/O operations
- **Axum**: Web framework for HTTP API endpoints
- **quick-xml**: Fast XML parsing and serialization
- **serde/serde_json**: JSON serialization support
- **fs2**: File locking for concurrent access
- **uuid**: Unique identifier generation

## Installation

```bash
# Clone the repository
git clone <repository-url>
cd stream-db

# Build the project
cargo build --release

# Run the server
cargo run
```

## Usage

### Starting the Server

```bash
cargo run
```

The server will start on `http://localhost:3000` by default.

### Writing Data

Write properties in chunks:

```bash
# Chunk 1
curl -X POST http://localhost:3000/write-item-stream/user123/1 \
  -H "Content-Type: application/xml" \
  -d '<property for="name"><string>John Doe</string></property>'

# Chunk 2
curl -X POST http://localhost:3000/write-item-stream/user123/1 \
  -H "Content-Type: application/xml" \
  -d '<property for="email"><string>john@example.com</string></property>'
```

### Reading Data

Stream data as it's being written:

```bash
curl -N http://localhost:3000/read-item-stream/user123/1
```

### Concurrent Write and Read

You can have multiple readers consuming data while a writer is streaming:

```bash
# Terminal 1: Start readers
for i in {1..3}; do
  curl -N "http://localhost:3000/read-item-stream/test_item/1" > "/tmp/reader_${i}.log" &
done

# Terminal 2: Write data in chunks
for i in {1..10}; do
  curl -X POST http://localhost:3000/write-item-stream/test_item/1 \
    -H "Content-Type: application/xml" \
    -d "<property for=\"chunk_${i}\"><string>Data chunk ${i}</string></property>"
  sleep 0.5
done
```

## Project Structure

```
stream-db/
├── Cargo.toml              # Project dependencies
├── README.md               # This file
├── LICENSE                 # License information
├── src/                    # Source code
│   └── ...                 # Source files
```

## Development

### Building

```bash
cargo build
```

### Running Tests

```bash
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

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
