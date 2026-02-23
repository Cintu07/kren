# KREN

Zero-Copy Shared Memory IPC

KREN is a fast, cross-language Inter-Process Communication (IPC) library written in Rust. It does not use OS networking like TCP or HTTP and avoids serialization like JSON or Protobuf. Instead, it creates a direct shared memory link between processes.

## Features

* Zero-Copy Transfers: Data is written once and read directly.
* Low Latency: 102ns for 64B messages measured.
* Lock-Free: Uses a Single-Producer Single-Consumer ring buffer with atomic synchronization.
* Cross-Language: Native bindings for Python and Node.js.
* Battle-Tested: Passes stress tests and 100K message validation.
* Windows: Uses Named File Mappings (CreateFileMappingW).
* Linux and macOS: Uses POSIX shared memory (shm_open and mmap).

## Installation

### Rust

Add this to your Cargo.toml:

```toml
[dependencies]
kren-core = "0.1"
```

### Python

```bash
pip install kren
```

### Node.js

```bash
npm install @pawanxz/kren
```

## Quick Start

### Python to Node.js Communication

**writer.py**
```python
import kren

writer = kren.Writer("my_channel", 1024 * 1024)
writer.write(b"Hello from Python!")
print(f"Written. Available space: {writer.available}")
```

**reader.js**
```javascript
const kren = require('@pawanxz/kren');

const reader = new kren.Reader("my_channel");
const data = reader.read();
console.log(`Received: ${data.toString()}`);
```

### Rust Usage

```rust
use kren_core::{KrenWriter, KrenReader};

// Process A
let mut writer = KrenWriter::create("my_channel", 4096).unwrap();
writer.write(b"Hello, World!").unwrap();

// Process B
let mut reader = KrenReader::connect("my_channel").unwrap();
let data = reader.read().unwrap();
assert_eq!(data, b"Hello, World!");
```

## Architecture

Process A (like Python) and Process B (like Node.js) talk through a shared memory segment. KrenWriter writes data to the memory. KrenReader reads data from the memory. They use a ring buffer managed by atomic head and tail pointers.

## Performance (Measured)

| Message Size | Latency | Throughput |
|-------------|---------|------------|
| 64 bytes | 102 ns | 9.7M ops/sec |
| 1 KB | 189 ns | 5.2M ops/sec |
| 64 KB | 5.6 μs | 11 GB/s |

Benchmarks were measured on Windows with Rust 1.93 in release mode. Run `cargo test --release --test bench -- --nocapture` to reproduce.

## API Reference

### Writer

| Method | Description |
|--------|-------------|
| `Writer(name, capacity)` | Create a new channel |
| `write(data)` | Write bytes to the channel |
| `available` | Free space in bytes |
| `name` | Channel identifier |

### Reader

| Method | Description |
|--------|-------------|
| `Reader(name)` | Connect to existing channel |
| `read()` | Read next message |
| `try_read()` | Non-blocking read, returns None or null if empty |
| `writer_closed` | Check if writer disconnected |
| `available` | Data available in bytes |
| `name` | Channel identifier |

## Building from Source

```bash
git clone https://github.com/Cintu07/kren
cd kren

# Build and test core library
cargo build --release
cargo test

# Build Python bindings
cd kren-python
maturin develop
pytest

# Build Node.js bindings
cd kren-node
npm run build
npm test
```

## License

MIT License. See LICENSE for details.
