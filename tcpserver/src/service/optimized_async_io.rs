//! 최적화된 비동기 I/O 처리
//! 
//! Zero-copy, vectored I/O, 그리고 링 버퍼를 활용한 고성능 I/O

use anyhow::Result;
use bytes::{Bytes, BytesMut, BufMut};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadBuf};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use crossbeam::channel::{unbounded, Sender, Receiver};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::future::Future;
use tokio::sync::mpsc;
use tracing::{debug, trace};

/// 링 버퍼 구현 (Zero-copy를 위한)
pub struct RingBuffer {
    buffer: Vec<u8>,
    capacity: usize,
    read_pos: usize,
    write_pos: usize,
    size: usize,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![0u8; capacity],
            capacity,
            read_pos: 0,
            write_pos: 0,
            size: 0,
        }
    }
    
    pub fn write(&mut self, data: &[u8]) -> usize {
        let available = self.capacity - self.size;
        let to_write = data.len().min(available);
        
        if to_write == 0 {
            return 0;
        }
        
        // 두 부분으로 나누어 쓰기 (링 버퍼 경계 처리)
        let first_part = (self.capacity - self.write_pos).min(to_write);
        self.buffer[self.write_pos..self.write_pos + first_part]
            .copy_from_slice(&data[..first_part]);
        
        if to_write > first_part {
            let second_part = to_write - first_part;
            self.buffer[..second_part]
                .copy_from_slice(&data[first_part..to_write]);
        }
        
        self.write_pos = (self.write_pos + to_write) % self.capacity;
        self.size += to_write;
        
        to_write
    }
    
    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        let to_read = buf.len().min(self.size);
        
        if to_read == 0 {
            return 0;
        }
        
        // 두 부분으로 나누어 읽기
        let first_part = (self.capacity - self.read_pos).min(to_read);
        buf[..first_part]
            .copy_from_slice(&self.buffer[self.read_pos..self.read_pos + first_part]);
        
        if to_read > first_part {
            let second_part = to_read - first_part;
            buf[first_part..to_read]
                .copy_from_slice(&self.buffer[..second_part]);
        }
        
        self.read_pos = (self.read_pos + to_read) % self.capacity;
        self.size -= to_read;
        
        to_read
    }
    
    pub fn available(&self) -> usize {
        self.size
    }
    
    pub fn capacity(&self) -> usize {
        self.capacity - self.size
    }
}

/// Vectored I/O를 위한 버퍼 매니저
pub struct VectoredIOManager {
    write_queue: Arc<Mutex<VecDeque<Bytes>>>,
    read_buffer: Arc<Mutex<RingBuffer>>,
    flush_threshold: usize,
}

impl VectoredIOManager {
    pub fn new(buffer_size: usize, flush_threshold: usize) -> Self {
        Self {
            write_queue: Arc::new(Mutex::new(VecDeque::with_capacity(100))),
            read_buffer: Arc::new(Mutex::new(RingBuffer::new(buffer_size))),
            flush_threshold,
        }
    }
    
    /// 벡터화된 쓰기 작업
    pub async fn vectored_write(
        &self,
        writer: &mut OwnedWriteHalf,
        data: Bytes,
    ) -> Result<()> {
        let should_flush = {
            let mut queue = self.write_queue.lock();
            queue.push_back(data);
            queue.len() >= self.flush_threshold
        };
        
        if should_flush {
            self.flush_writes(writer).await?;
        }
        
        Ok(())
    }
    
    /// 버퍼된 데이터 플러시
    pub async fn flush_writes(&self, writer: &mut OwnedWriteHalf) -> Result<()> {
        let chunks: Vec<Bytes> = {
            let mut queue = self.write_queue.lock();
            queue.drain(..).collect()
        };
        
        if chunks.is_empty() {
            return Ok(());
        }
        
        // Vectored write를 위한 IoSlice 준비
        let io_slices: Vec<std::io::IoSlice> = chunks
            .iter()
            .map(|chunk| std::io::IoSlice::new(chunk))
            .collect();
        
        // 한 번의 시스템 콜로 모든 데이터 전송
        let mut total_written = 0;
        while total_written < io_slices.len() {
            match writer.try_write_vectored(&io_slices[total_written..]) {
                Ok(n) => {
                    total_written += n;
                    trace!("Vectored write: {} bytes", n);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // 비동기 대기
                    writer.writable().await?;
                }
                Err(e) => return Err(e.into()),
            }
        }
        
        writer.flush().await?;
        Ok(())
    }
    
    /// Zero-copy 읽기
    pub async fn zero_copy_read(
        &self,
        reader: &mut OwnedReadHalf,
    ) -> Result<Option<Bytes>> {
        let mut buf = BytesMut::with_capacity(4096);
        
        // ReadBuf를 사용한 zero-copy 읽기
        let mut read_buf = ReadBuf::new(&mut buf);
        
        match reader.poll_read(
            &mut Context::from_waker(futures::task::noop_waker_ref()),
            &mut read_buf,
        ) {
            Poll::Ready(Ok(())) => {
                let filled = read_buf.filled().len();
                if filled == 0 {
                    return Ok(None);
                }
                buf.advance(filled);
                Ok(Some(buf.freeze()))
            }
            Poll::Ready(Err(e)) => Err(e.into()),
            Poll::Pending => Ok(None),
        }
    }
}

/// 파이프라인 방식의 메시지 프로세서
pub struct PipelinedMessageProcessor {
    // 읽기 파이프라인
    read_stage1: mpsc::UnboundedSender<BytesMut>,
    read_stage2: mpsc::UnboundedSender<serde_json::Value>,
    read_stage3: mpsc::UnboundedSender<crate::protocol::GameMessage>,
    
    // 쓰기 파이프라인
    write_stage1: mpsc::UnboundedSender<crate::protocol::GameMessage>,
    write_stage2: mpsc::UnboundedSender<BytesMut>,
    write_stage3: mpsc::UnboundedSender<Bytes>,
}

impl PipelinedMessageProcessor {
    pub fn new() -> Self {
        let (read1_tx, mut read1_rx) = mpsc::unbounded_channel();
        let (read2_tx, mut read2_rx) = mpsc::unbounded_channel();
        let (read3_tx, _read3_rx) = mpsc::unbounded_channel();
        
        let (write1_tx, mut write1_rx) = mpsc::unbounded_channel();
        let (write2_tx, mut write2_rx) = mpsc::unbounded_channel();
        let (write3_tx, _write3_rx) = mpsc::unbounded_channel();
        
        // 읽기 파이프라인 스테이지 1: 바이트 -> JSON
        let read2_tx_clone = read2_tx.clone();
        tokio::spawn(async move {
            while let Some(bytes) = read1_rx.recv().await {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    let _ = read2_tx_clone.send(json);
                }
            }
        });
        
        // 읽기 파이프라인 스테이지 2: JSON -> GameMessage
        let read3_tx_clone = read3_tx.clone();
        tokio::spawn(async move {
            while let Some(json) = read2_rx.recv().await {
                if let Ok(msg) = serde_json::from_value(json) {
                    let _ = read3_tx_clone.send(msg);
                }
            }
        });
        
        // 쓰기 파이프라인 스테이지 1: GameMessage -> 직렬화
        let write2_tx_clone = write2_tx.clone();
        tokio::spawn(async move {
            while let Some(msg) = write1_rx.recv().await {
                if let Ok(json) = serde_json::to_vec(&msg) {
                    let mut buf = BytesMut::with_capacity(json.len() + 4);
                    buf.put_u32(json.len() as u32);
                    buf.extend_from_slice(&json);
                    let _ = write2_tx_clone.send(buf);
                }
            }
        });
        
        // 쓰기 파이프라인 스테이지 2: 압축 (선택적)
        let write3_tx_clone = write3_tx.clone();
        tokio::spawn(async move {
            while let Some(buf) = write2_rx.recv().await {
                // 여기서 압축 로직 추가 가능
                let _ = write3_tx_clone.send(buf.freeze());
            }
        });
        
        Self {
            read_stage1: read1_tx,
            read_stage2: read2_tx,
            read_stage3: read3_tx,
            write_stage1: write1_tx,
            write_stage2: write2_tx,
            write_stage3: write3_tx,
        }
    }
    
    /// 메시지 처리 파이프라인에 추가
    pub async fn process_incoming(&self, data: BytesMut) -> Result<()> {
        self.read_stage1.send(data)?;
        Ok(())
    }
    
    pub async fn process_outgoing(&self, msg: crate::protocol::GameMessage) -> Result<()> {
        self.write_stage1.send(msg)?;
        Ok(())
    }
}

/// 고성능 I/O 스케줄러
pub struct IOScheduler {
    io_manager: Arc<VectoredIOManager>,
    processor: Arc<PipelinedMessageProcessor>,
    batch_size: usize,
    flush_interval_ms: u64,
}

impl IOScheduler {
    pub fn new(buffer_size: usize, batch_size: usize, flush_interval_ms: u64) -> Self {
        Self {
            io_manager: Arc::new(VectoredIOManager::new(buffer_size, batch_size)),
            processor: Arc::new(PipelinedMessageProcessor::new()),
            batch_size,
            flush_interval_ms,
        }
    }
    
    /// 스케줄러 시작
    pub async fn start(
        &self,
        mut reader: OwnedReadHalf,
        mut writer: OwnedWriteHalf,
    ) -> Result<()> {
        let io_manager = self.io_manager.clone();
        let processor = self.processor.clone();
        let flush_interval = self.flush_interval_ms;
        
        // 읽기 태스크
        let read_task = tokio::spawn(async move {
            loop {
                match io_manager.zero_copy_read(&mut reader).await {
                    Ok(Some(data)) => {
                        let _ = processor.process_incoming(BytesMut::from(&data[..])).await;
                    }
                    Ok(None) => {
                        tokio::time::sleep(tokio::time::Duration::from_micros(10)).await;
                    }
                    Err(e) => {
                        debug!("Read error: {}", e);
                        break;
                    }
                }
            }
        });
        
        // 쓰기 플러시 태스크
        let io_manager_write = self.io_manager.clone();
        let write_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_millis(flush_interval)
            );
            
            loop {
                interval.tick().await;
                if let Err(e) = io_manager_write.flush_writes(&mut writer).await {
                    debug!("Write flush error: {}", e);
                    break;
                }
            }
        });
        
        // 태스크 조인
        tokio::select! {
            _ = read_task => {}
            _ = write_task => {}
        }
        
        Ok(())
    }
}

/// 성능 메트릭
#[derive(Debug, Default)]
pub struct IOMetrics {
    pub bytes_read: std::sync::atomic::AtomicU64,
    pub bytes_written: std::sync::atomic::AtomicU64,
    pub messages_read: std::sync::atomic::AtomicU64,
    pub messages_written: std::sync::atomic::AtomicU64,
    pub read_operations: std::sync::atomic::AtomicU64,
    pub write_operations: std::sync::atomic::AtomicU64,
    pub vectored_writes: std::sync::atomic::AtomicU64,
    pub zero_copy_reads: std::sync::atomic::AtomicU64,
}

impl IOMetrics {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn record_read(&self, bytes: usize) {
        self.bytes_read.fetch_add(bytes as u64, std::sync::atomic::Ordering::Relaxed);
        self.read_operations.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    pub fn record_write(&self, bytes: usize) {
        self.bytes_written.fetch_add(bytes as u64, std::sync::atomic::Ordering::Relaxed);
        self.write_operations.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    pub fn record_vectored_write(&self) {
        self.vectored_writes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    pub fn record_zero_copy_read(&self) {
        self.zero_copy_reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}