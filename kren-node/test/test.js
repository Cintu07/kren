/**
 * Tests for KREN Node.js bindings
 */

const assert = require('assert');
const kren = require('..');

console.log('KREN Node.js Tests');
console.log('==================\n');

let passed = 0;
let failed = 0;

function test(name, fn) {
    try {
        fn();
        console.log(`✓ ${name}`);
        passed++;
    } catch (e) {
        console.log(`✗ ${name}`);
        console.log(`  Error: ${e.message}`);
        failed++;
    }
}

// Writer tests
test('Writer can be created', () => {
    const writer = new kren.Writer('node_test_create', 4096);
    assert.strictEqual(writer.name, 'node_test_create');
    assert(writer.available > 0);
});

test('Writer can write data', () => {
    const writer = new kren.Writer('node_test_write', 1024);
    const written = writer.write(Buffer.from('Hello, KREN!'));
    assert.strictEqual(written, 12);
});

// Reader tests
test('Reader can connect to channel', () => {
    const writer = new kren.Writer('node_test_connect', 1024);
    const reader = new kren.Reader('node_test_connect');
    assert.strictEqual(reader.name, 'node_test_connect');
});

test('Reader can read data', () => {
    const writer = new kren.Writer('node_test_read', 1024);
    const reader = new kren.Reader('node_test_read');

    writer.write(Buffer.from('Test message'));
    const data = reader.read();
    assert.strictEqual(data.toString(), 'Test message');
});

test('tryRead returns null when empty', () => {
    const writer = new kren.Writer('node_test_try_empty', 1024);
    const reader = new kren.Reader('node_test_try_empty');

    const result = reader.tryRead();
    assert.strictEqual(result, null);
});

test('tryRead returns data when available', () => {
    const writer = new kren.Writer('node_test_try_data', 1024);
    const reader = new kren.Reader('node_test_try_data');

    writer.write(Buffer.from('Available data'));
    const result = reader.tryRead();
    assert.strictEqual(result.toString(), 'Available data');
});

// Roundtrip tests
test('Multiple messages in order', () => {
    const writer = new kren.Writer('node_test_multi', 4096);
    const reader = new kren.Reader('node_test_multi');

    const messages = [];
    for (let i = 0; i < 10; i++) {
        messages.push(`Message ${i}`);
    }

    for (const msg of messages) {
        writer.write(Buffer.from(msg));
    }

    for (const expected of messages) {
        const received = reader.read();
        assert.strictEqual(received.toString(), expected);
    }
});

test('Large data transfer', () => {
    const writer = new kren.Writer('node_test_large', 65536);
    const reader = new kren.Reader('node_test_large');

    const largeData = Buffer.alloc(10000, 'X');
    writer.write(largeData);
    const received = reader.read();
    assert.strictEqual(received.length, 10000);
    assert(received.equals(largeData));
});

test('Version is available', () => {
    const ver = kren.version();
    assert(ver.match(/^\d+\.\d+\.\d+$/));
});

// Summary
console.log('\n==================');
console.log(`Results: ${passed} passed, ${failed} failed`);
process.exit(failed > 0 ? 1 : 0);
