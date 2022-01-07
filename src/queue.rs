use std::{
    io::Cursor,
    sync::{atomic::AtomicUsize, Arc},
};

use concurrent_queue::ConcurrentQueue;

/// Create a new queue
pub fn new_queue(capacity: usize) -> (ByteProducer, ByteConsumer) {
    let inner = Arc::new(ConcurrentQueue::bounded(capacity));
    let ctr = Arc::new(AtomicUsize::new(0));
    let send = ByteProducer {
        inner: inner.clone(),
        ctr: ctr.clone(),
    };
    let recv = ByteConsumer {
        inner,
        buffer: Default::default(),
        ctr,
    };
    (send, recv)
}

/// Sending side of a byte queue.
pub struct ByteProducer {
    inner: Arc<ConcurrentQueue<Vec<u8>>>,
    ctr: Arc<AtomicUsize>,
}

/// Reading side of a byte queue.
pub struct ByteConsumer {
    inner: Arc<ConcurrentQueue<Vec<u8>>>,
    buffer: Cursor<Vec<u8>>,
    ctr: Arc<AtomicUsize>,
}

impl std::io::Write for ByteProducer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if !buf.is_empty() {
            self.inner
                .push(buf.to_vec())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::WouldBlock, e))?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl std::io::Read for ByteConsumer {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if let Ok(n) = self.buffer.read(buf) {
            if n > 0 {
                return Ok(n);
            }
        }
        if let Ok(bts) = self.inner.pop() {
            self.buffer = Cursor::new(bts);
            self.buffer.read(buf)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "would block",
            ))
        }
    }
}
