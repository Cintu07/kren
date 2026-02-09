"""Tests for KREN Python bindings."""

import pytest
import kren


class TestWriter:
    """Tests for the Writer class."""

    def test_create_writer(self):
        """Writer can be created with valid parameters."""
        writer = kren.Writer("py_test_create", 4096)
        assert writer.name == "py_test_create"
        assert writer.available > 0

    def test_write_data(self):
        """Writer can write data to the channel."""
        writer = kren.Writer("py_test_write", 1024)
        written = writer.write(b"Hello, KREN!")
        assert written == 12

    def test_invalid_capacity(self):
        """Writer raises error for invalid capacity."""
        with pytest.raises(ValueError):
            kren.Writer("py_test_invalid", 0)


class TestReader:
    """Tests for the Reader class."""

    def test_connect_reader(self):
        """Reader can connect to an existing channel."""
        writer = kren.Writer("py_test_connect", 1024)
        reader = kren.Reader("py_test_connect")
        assert reader.name == "py_test_connect"

    def test_read_data(self):
        """Reader can read data from the channel."""
        writer = kren.Writer("py_test_read", 1024)
        reader = kren.Reader("py_test_read")
        
        writer.write(b"Test message")
        data = reader.read()
        assert data == b"Test message"

    def test_try_read_empty(self):
        """try_read returns None when buffer is empty."""
        writer = kren.Writer("py_test_try_empty", 1024)
        reader = kren.Reader("py_test_try_empty")
        
        result = reader.try_read()
        assert result is None

    def test_try_read_with_data(self):
        """try_read returns data when available."""
        writer = kren.Writer("py_test_try_data", 1024)
        reader = kren.Reader("py_test_try_data")
        
        writer.write(b"Available data")
        result = reader.try_read()
        assert result == b"Available data"


class TestRoundtrip:
    """End-to-end roundtrip tests."""

    def test_multiple_messages(self):
        """Multiple messages can be sent and received in order."""
        writer = kren.Writer("py_test_multi", 4096)
        reader = kren.Reader("py_test_multi")
        
        messages = [f"Message {i}".encode() for i in range(10)]
        
        for msg in messages:
            writer.write(msg)
        
        for expected in messages:
            received = reader.read()
            assert received == expected

    def test_large_data(self):
        """Large data can be transferred correctly."""
        writer = kren.Writer("py_test_large", 65536)
        reader = kren.Reader("py_test_large")
        
        large_data = b"X" * 10000
        writer.write(large_data)
        received = reader.read()
        assert received == large_data

    def test_writer_closed_detection(self):
        """Reader can detect when writer is closed."""
        writer = kren.Writer("py_test_closed", 1024)
        reader = kren.Reader("py_test_closed")
        
        assert not reader.writer_closed
        del writer  # Close writer
        assert reader.writer_closed
