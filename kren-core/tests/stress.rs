//! Stress tests for KREN ring buffer and shared memory

use kren_core::{KrenWriter, KrenReader};

#[test]
fn stress_100k_messages() {
    let name = "stress_100k";
    let mut writer = KrenWriter::create(name, 1024 * 64).expect("create");
    let mut reader = KrenReader::connect(name).expect("connect");

    for i in 0..100_000 {
        let msg = format!("msg_{:06}", i);
        writer.write(msg.as_bytes()).expect("write");
        let data = reader.read().expect("read");
        assert_eq!(data, msg.as_bytes(), "mismatch at message {}", i);
    }
}

#[test]
fn stress_large_message() {
    let name = "stress_large";
    let size = 1024 * 1024; // 1MB buffer
    let mut writer = KrenWriter::create(name, size).expect("create");
    let mut reader = KrenReader::connect(name).expect("connect");

    // Write a large message (max that fits with 4-byte length prefix)
    let large_data: Vec<u8> = (0..size / 2).map(|i| (i % 256) as u8).collect();
    writer.write(&large_data).expect("write large");
    let received = reader.read().expect("read large");
    assert_eq!(received, large_data);
}

#[test]
fn stress_wraparound() {
    let name = "stress_wrap";
    let capacity = 256; // Small buffer to force many wraparounds
    let mut writer = KrenWriter::create(name, capacity).expect("create");
    let mut reader = KrenReader::connect(name).expect("connect");

    // Write messages that will wrap around many times
    for i in 0..10_000 {
        let msg = format!("w{}", i);
        writer.write(msg.as_bytes()).expect("write");
        let data = reader.read().expect("read");
        assert_eq!(data, msg.as_bytes(), "wraparound failed at {}", i);
    }
}

#[test]
fn stress_varying_sizes() {
    let name = "stress_vary";
    let mut writer = KrenWriter::create(name, 1024 * 32).expect("create");
    let mut reader = KrenReader::connect(name).expect("connect");

    let sizes = [1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];

    for (i, &size) in sizes.iter().cycle().take(5000).enumerate() {
        let data: Vec<u8> = (0..size).map(|j| ((i + j) % 256) as u8).collect();
        writer.write(&data).expect("write");
        let received = reader.read().expect("read");
        assert_eq!(received, data, "mismatch at msg {} (size {})", i, size);
    }
}

#[test]
fn stress_buffer_pressure() {
    let name = "stress_pressure";
    let capacity = 512;
    let mut writer = KrenWriter::create(name, capacity).expect("create");
    let mut reader = KrenReader::connect(name).expect("connect");

    // Fill buffer nearly full, drain, repeat
    for cycle in 0..100 {
        // Fill
        let mut count = 0;
        loop {
            let msg = format!("c{}m{}", cycle, count);
            match writer.write(msg.as_bytes()) {
                Ok(_) => count += 1,
                Err(_) => break,
            }
        }
        assert!(count > 0, "should write at least 1 message");

        // Drain
        for j in 0..count {
            let expected = format!("c{}m{}", cycle, j);
            let data = reader.read().expect("read during drain");
            assert_eq!(data, expected.as_bytes());
        }
    }
}

#[test]
fn stress_writer_drop_safety() {
    let name = "stress_drop";

    let reader;
    {
        let mut writer = KrenWriter::create(name, 1024).expect("create");
        reader = KrenReader::connect(name).expect("connect");
        writer.write(b"before_drop").expect("write");
        // writer drops here
    }

    assert!(reader.is_writer_closed());
    // Should still be able to read data written before drop
    let mut reader = reader;
    let data = reader.read().expect("read after drop");
    assert_eq!(data, b"before_drop");
}

#[test]
fn stress_empty_messages() {
    let name = "stress_empty";
    let mut writer = KrenWriter::create(name, 1024).expect("create");
    let mut reader = KrenReader::connect(name).expect("connect");

    // Empty data should work
    writer.write(b"").expect("write empty");
    let data = reader.read().expect("read empty");
    assert_eq!(data, b"");

    // Normal data after empty
    writer.write(b"after_empty").expect("write after");
    let data = reader.read().expect("read after");
    assert_eq!(data, b"after_empty");
}
