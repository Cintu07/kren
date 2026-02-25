# Social Media Posts for KREN

## Reddit Post

**Title:** I built a shared memory IPC library in Rust with Python and Node.js bindings

**Body:**

Hey everyone,

I built a library called KREN that lets processes on the same machine pass data to each other through shared memory instead of using sockets or HTTP.

The idea is pretty straightforward. If two processes are running on the same machine, they can skip the network stack entirely and just read and write to a shared chunk of memory. This avoids copying the data more than once and skips serialization steps like JSON or Protobuf.

What it does:

- Transfers data without making extra copies (zero-copy)
- Works across Python, Node.js, and Rust processes
- Uses a ring buffer with atomic pointers so it does not need locks
- Latency around 102ns for small 64-byte messages on Windows in release mode

You can install it from:

- crates.io for Rust: `kren-core`
- PyPI for Python: `pip install kren`
- npm for Node.js: `npm install @pawanxz/kren`

Source is on GitHub: https://github.com/Cintu07/kren

I am still working on it and would appreciate any feedback, bug reports, or suggestions.

---

## LinkedIn Post

I recently published a small open source library called KREN.

It is a shared memory IPC library written in Rust. The basic idea is that if two programs are running on the same machine and need to pass data between each other, they can use shared memory instead of going through TCP or HTTP.

This cuts out the network stack overhead and avoids serialization. Data goes from one process to the other directly through a shared memory region.

It comes with bindings for Python and Node.js, so a Python script can write data and a Node.js service can read it, or the other way around.

Some benchmark numbers from testing on Windows with Rust 1.93 in release mode:

- 64 byte messages: around 102ns latency
- 1KB messages: around 189ns latency
- 64KB messages: around 5.6 microseconds

It is available on crates.io, PyPI, and npm. The source code is on GitHub if you want to take a look or try it out.

GitHub: https://github.com/Cintu07/kren

Happy to hear any thoughts or feedback.
