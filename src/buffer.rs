use std::{
    collections::VecDeque,
    io::{Read, Write},
    sync::{Arc, Mutex},
};

struct Inner {
    buf: VecDeque<u8>,
    limit: usize,
    closed: bool,
}

pub fn new(limit: usize) -> (Producer, Consumer) {
    let inner = Arc::new(Mutex::new(Inner {
        buf: VecDeque::new(),
        limit,
        closed: false,
    }));
    (
        Producer {
            inner: inner.clone(),
        },
        Consumer { inner },
    )
}

pub struct Producer {
    inner: Arc<Mutex<Inner>>,
}

impl Write for Producer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut inner = self.inner.lock().unwrap();
        if inner.closed {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "closed",
            ));
        }
        if inner.buf.len() >= inner.limit {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "would block",
            ));
        }
        let len = buf.len().min(inner.limit - inner.buf.len());
        inner.buf.write_all(&buf[..len]).unwrap();
        assert!(inner.buf.len() <= inner.limit);
        Ok(len)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct Consumer {
    inner: Arc<Mutex<Inner>>,
}

impl Read for Consumer {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut inner = self.inner.lock().unwrap();
        if inner.buf.capacity() > (inner.buf.len() * 4 + 16) {
            inner.buf.shrink_to_fit();
        }
        if !inner.closed && inner.buf.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "would block",
            ));
        }
        inner.buf.read(buf)
    }
}
