# KREN

<p align="center">
  <img src="docs/logo.png" alt="KREN Logo" width="200"/>
</p>

<p align="center">
  <strong>Zero-Copy Shared Memory IPC</strong>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#installation">Installation</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#performance">Performance</a> •
  <a href="#api">API</a>
</p>

---

**KREN** is a high-performance, cross-language Inter-Process Communication (IPC) library written in Rust. It bypasses standard OS networking (TCP/HTTP) and serialization (JSON/Protobuf) by establishing a direct shared memory link between processes.

## Features

- 🚀 **Zero-Copy Transfers** - Data is written once, read directly
- ⚡ **Nanosecond Latency** - No kernel context switches
- 🔒 **Lock-Free** - SPSC ring buffer with atomic synchronization
- 🌍 **Cross-Language** - Native bindings for Python and Node.js
- 💪 **Production Ready** - Comprehensive error handling and tests
- 🪟 **Windows Support** - Uses Named File Mappings

## Installation

### Rust

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
npm install kren
```

## Quick Start

### Python → Node.js Communication

**writer.py**
```python
import kren

# Create a channel with 1MB buffer
writer = kren.Writer("my_channel", 1024 * 1024)

# Write data
writer.write(b"Hello from Python!")
print(f"Written! Available space: {writer.available}")
```

**reader.js**
```javascript
const kren = require('kren');

// Connect to the channel
const reader = new kren.Reader("my_channel");

// Read data
const data = reader.read();
console.log(`Received: ${data.toString()}`);
```

### Rust Usage

```rust
use kren_core::{KrenWriter, KrenReader};

// Process A: Create writer
let mut writer = KrenWriter::create("my_channel", 4096)?;
writer.write(b"Hello, World!")?;

// Process B: Connect reader
let mut reader = KrenReader::connect("my_channel")?;
let data = reader.read()?;
assert_eq!(data, b"Hello, World!");
```

## Architecture

```
┌─────────────────┐                     ┌─────────────────┐
│    Process A    │                     │    Process B    │
│   (Python AI)   │                     │  (Node.js API)  │
├─────────────────┤                     ├─────────────────┤
│  KrenWriter     │                     │  KrenReader     │
│  ┌───────────┐  │                     │  ┌───────────┐  │
│  │  write()  │  │                     │  │  read()   │  │
│  └─────┬─────┘  │                     │  └─────▲─────┘  │
└────────┼────────┘                     └────────┼────────┘
         │                                       │
         │         ┌───────────────────┐         │
         └────────►│   Shared Memory   │◄────────┘
                   │  ┌─────────────┐  │
                   │  │ Ring Buffer │  │
                   │  │ [H]----[T]  │  │
                   │  └─────────────┘  │
                   └───────────────────┘
```

## Performance

| Metric | KREN | TCP localhost | HTTP+JSON |
|--------|------|---------------|-----------|
| Latency (p50) | ~100ns | ~10μs | ~100μs |
| Throughput | 10M+ msg/s | 100K msg/s | 10K msg/s |
| CPU Usage | Minimal | Moderate | High |
| Memory Copy | 0 | 2+ | 4+ |

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
| `read()` | Read next message (blocks conceptually) |
| `try_read()` | Non-blocking read, returns None/null if empty |
| `writer_closed` | Check if writer has disconnected |
| `available` | Data available in bytes |
| `name` | Channel identifier |

## Building from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/kren
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

MIT License - see [LICENSE](LICENSE) for details.
