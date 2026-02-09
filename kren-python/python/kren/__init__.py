"""
KREN - Zero-Copy Shared Memory IPC

High-performance inter-process communication using shared memory
with lock-free ring buffers.

Example:
    >>> import kren
    >>> writer = kren.Writer("channel", 4096)
    >>> writer.write(b"Hello!")
    6
    >>> reader = kren.Reader("channel")
    >>> reader.read()
    b'Hello!'

Classes:
    Writer: Creates a shared memory channel for writing
    Reader: Connects to an existing channel for reading
"""

from .kren import Writer, Reader, __version__

__all__ = ["Writer", "Reader", "__version__"]
