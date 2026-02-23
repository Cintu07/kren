//! Simple benchmarks for KREN latency and throughput
//!
//! Run with: cargo test --release --test bench -- --nocapture

use kren_core::{KrenWriter, KrenReader};
use std::time::Instant;

#[test]
fn bench_latency_64b() {
    let name = "bench_lat_64";
    let mut writer = KrenWriter::create(name, 1024 * 64).expect("create");
    let mut reader = KrenReader::connect(name).expect("connect");

    let data = [0xABu8; 64];
    let iterations = 100_000;

    // Warmup
    for _ in 0..1000 {
        writer.write(&data).expect("write");
        reader.read().expect("read");
    }

    // Measure
    let start = Instant::now();
    for _ in 0..iterations {
        writer.write(&data).expect("write");
        reader.read().expect("read");
    }
    let elapsed = start.elapsed();

    let latency_ns = elapsed.as_nanos() / iterations as u128;
    let ops_per_sec = (iterations as f64 / elapsed.as_secs_f64()) as u64;

    println!("\n=== 64B Message Latency ===");
    println!("  Latency: {} ns/op", latency_ns);
    println!("  Throughput: {} ops/sec", ops_per_sec);
    println!("  Total: {:.2?} for {} ops", elapsed, iterations);
}

#[test]
fn bench_latency_1kb() {
    let name = "bench_lat_1k";
    let mut writer = KrenWriter::create(name, 1024 * 128).expect("create");
    let mut reader = KrenReader::connect(name).expect("connect");

    let data = [0xCDu8; 1024];
    let iterations = 50_000;

    // Warmup
    for _ in 0..500 {
        writer.write(&data).expect("write");
        reader.read().expect("read");
    }

    let start = Instant::now();
    for _ in 0..iterations {
        writer.write(&data).expect("write");
        reader.read().expect("read");
    }
    let elapsed = start.elapsed();

    let latency_ns = elapsed.as_nanos() / iterations as u128;
    let throughput_mbps = (iterations as f64 * 1024.0 / elapsed.as_secs_f64()) / (1024.0 * 1024.0);

    println!("\n=== 1KB Message Latency ===");
    println!("  Latency: {} ns/op", latency_ns);
    println!("  Throughput: {:.1} MB/s", throughput_mbps);
    println!("  Total: {:.2?} for {} ops", elapsed, iterations);
}

#[test]
fn bench_latency_64kb() {
    let name = "bench_lat_64k";
    let mut writer = KrenWriter::create(name, 1024 * 256).expect("create");
    let mut reader = KrenReader::connect(name).expect("connect");

    let data = [0xEFu8; 65536];
    let iterations = 1_000;

    // Warmup
    for _ in 0..100 {
        writer.write(&data).expect("write");
        reader.read().expect("read");
    }

    let start = Instant::now();
    for _ in 0..iterations {
        writer.write(&data).expect("write");
        reader.read().expect("read");
    }
    let elapsed = start.elapsed();

    let latency_ns = elapsed.as_nanos() / iterations as u128;
    let throughput_mbps = (iterations as f64 * 65536.0 / elapsed.as_secs_f64()) / (1024.0 * 1024.0);

    println!("\n=== 64KB Message Latency ===");
    println!("  Latency: {} ns/op", latency_ns);
    println!("  Throughput: {:.1} MB/s", throughput_mbps);
    println!("  Total: {:.2?} for {} ops", elapsed, iterations);
}

#[test]
fn bench_throughput_burst() {
    let name = "bench_burst";
    let capacity = 1024 * 1024; // 1MB buffer
    let mut writer = KrenWriter::create(name, capacity).expect("create");
    let mut reader = KrenReader::connect(name).expect("connect");

    let data = [0x42u8; 64];
    let batch = 10_000;

    // Write a batch, then read a batch
    let start = Instant::now();
    let mut total_written = 0;
    let mut total_read = 0;

    for _ in 0..10 {
        // Write burst
        for _ in 0..batch {
            writer.write(&data).expect("write");
            total_written += 1;
        }
        // Read burst
        for _ in 0..batch {
            reader.read().expect("read");
            total_read += 1;
        }
    }
    let elapsed = start.elapsed();

    let ops = total_written + total_read;
    let ops_per_sec = (ops as f64 / elapsed.as_secs_f64()) as u64;

    println!("\n=== Burst Throughput ===");
    println!("  {} write + {} read = {} ops", total_written, total_read, ops);
    println!("  Throughput: {} ops/sec", ops_per_sec);
    println!("  Total: {:.2?}", elapsed);
}
