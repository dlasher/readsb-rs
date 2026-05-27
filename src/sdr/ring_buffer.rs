use tokio::sync::mpsc;

/// A bounded ring buffer wrapping `tokio::sync::mpsc` for decoupling
/// the RTL_TCP reader from the main loop.
pub struct RingBuffer {
    pub tx: mpsc::Sender<Vec<u8>>,
    pub rx: mpsc::Receiver<Vec<u8>>,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        let (tx, rx) = mpsc::channel(capacity);
        RingBuffer { tx, rx }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ring_buffer_send_recv() {
        let mut rb = RingBuffer::new(4);
        let data = vec![0u8; 100];
        rb.tx.send(data.clone()).await.unwrap();
        let received = rb.rx.recv().await.unwrap();
        assert_eq!(received, data);
    }

    #[tokio::test]
    async fn test_ring_buffer_backpressure() {
        let rb = RingBuffer::new(2);
        let data = vec![0u8; 10];
        
        // Fill the buffer
        rb.tx.send(data.clone()).await.unwrap();
        rb.tx.send(data.clone()).await.unwrap();
        
        // Third push should block until we pop
        let tx = rb.tx.clone();
        let mut rx = rb.rx;
        
        let push_handle = tokio::spawn(async move {
            tx.send(data.clone()).await.unwrap();
        });
        
        // Pop one to make room
        rx.recv().await.unwrap();
        
        // Now the blocked push should succeed
        push_handle.await.unwrap();
        
        // Should have 2 items remaining
        assert!(rx.recv().await.is_some());
        assert!(rx.recv().await.is_some());
    }
}