use std::future::Future;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

pub trait AsyncSizedMessage {
    fn write_sized<'a>(
        &'a mut self,
        buf: &'a [u8],
    ) -> impl Future<Output = tokio::io::Result<()>> + Send + 'a;
    fn read_sized<'a>(&'a mut self)
    -> impl Future<Output = tokio::io::Result<Vec<u8>>> + Send + 'a;
}
impl AsyncSizedMessage for UnixStream {
    fn write_sized<'a>(
        &'a mut self,
        buf: &'a [u8],
    ) -> impl Future<Output = tokio::io::Result<()>> + Send + 'a {
        async move {
            // Safely convert buf_len from usize to u32
            let buf_len: u32 = buf.len().try_into().map_err(|_| {
                tokio::io::Error::new(std::io::ErrorKind::InvalidData, "message too large")
            })?;

            // Write message size to stream
            let len_bytes = buf_len.to_be_bytes();
            self.write_all(&len_bytes).await?;

            // Write message to stream
            self.write(buf).await?;

            Ok(())
        }
    }
    fn read_sized<'a>(
        &'a mut self,
    ) -> impl Future<Output = tokio::io::Result<Vec<u8>>> + Send + 'a {
        async move {
            let mut buf_len = [0u8; 4];

            // Read message length
            self.read_exact(&mut buf_len).await?;
            let msg_len = u32::from_be_bytes(buf_len) as usize;

            let mut buf = vec![0u8; msg_len];
            self.read_exact(&mut buf).await?;

            Ok(buf)
        }
    }
}
