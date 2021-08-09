use futures_lite::prelude::*;
use rtrb::{Consumer, Producer};
use std::{
    io::Read,
    io::Write,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::Context,
    task::Poll,
};

/// Create a "bipe". Use async_dup's methods if you want something cloneable/shareable
pub fn bipe(capacity: usize) -> (PipeWriter, PipeReader) {
    let buffer = rtrb::RingBuffer::new(capacity);
    let (send_buf, recv_buf) = buffer.split();
    let write_ready = Arc::new(event_listener::Event::new());
    let read_ready = Arc::new(event_listener::Event::new());
    let closed = Arc::new(AtomicBool::new(false));
    (
        PipeWriter {
            queue: send_buf,
            signal: write_ready.clone(),
            signal_reader: read_ready.clone(),
            listener: write_ready.listen(),
            closed: closed.clone(),
        },
        PipeReader {
            queue: recv_buf,
            signal: read_ready.clone(),
            signal_writer: write_ready.clone(),
            listener: read_ready.listen(),
            closed,
        },
    )
}

/// Writing end of a byte pipe.
pub struct PipeWriter {
    queue: Producer<u8>,
    signal: Arc<event_listener::Event>,
    signal_reader: Arc<event_listener::Event>,
    listener: event_listener::EventListener,
    closed: Arc<AtomicBool>,
}

impl Drop for PipeWriter {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::SeqCst);
        self.signal_reader.notify(1);
    }
}

fn broken_pipe() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::ConnectionReset, "broken pipe")
}

impl AsyncWrite for PipeWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        loop {
            if self.closed.load(Ordering::SeqCst) {
                return Poll::Ready(Err(broken_pipe()));
            }
            // if there's room in the buffer then it's fine
            {
                if let Ok(n) = self.queue.write(buf) {
                    // if n > 0 {
                    self.signal_reader.notify(1);
                    return Poll::Ready(Ok(n));
                    // }
                }
            }
            let listen_capacity = &mut self.listener;
            futures_lite::pin!(listen_capacity);
            // there's no room, so we try again later
            futures_lite::ready!(listen_capacity.poll(cx));
            self.listener = self.signal.listen()
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.closed.store(true, Ordering::Relaxed);
        self.signal_reader.notify(1);
        Poll::Ready(Ok(()))
    }
}

/// Read end of a byte pipe.
pub struct PipeReader {
    queue: Consumer<u8>,
    signal: Arc<event_listener::Event>,
    signal_writer: Arc<event_listener::Event>,
    listener: event_listener::EventListener,
    closed: Arc<AtomicBool>,
}

impl AsyncRead for PipeReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        loop {
            if let Ok(n) = self.queue.read(buf) {
                if n > 0 {
                    self.signal_writer.notify(1);
                    return Poll::Ready(Ok(n));
                }
            }
            if self.closed.load(Ordering::Relaxed) {
                return Poll::Ready(Err(broken_pipe()));
            }
            let listen_new_data = &mut self.listener;
            futures_lite::pin!(listen_new_data);
            futures_lite::ready!(listen_new_data.poll(cx));
            self.listener = self.signal.listen();
        }
    }
}
