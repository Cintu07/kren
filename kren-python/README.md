# KREN Python Bindings

Zero-copy shared memory IPC for Python.

## Installation

```bash
pip install kren
```

## Usage

```python
import kren

# Create a writer
writer = kren.Writer("my_channel", 4096)
writer.write(b"Hello!")

# Connect a reader
reader = kren.Reader("my_channel")
data = reader.read()
print(data)  # b"Hello!"
```
