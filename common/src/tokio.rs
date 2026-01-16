use serde::Serialize;
use std::future::Future;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

use crate::utils::errors::{WatsonError, WatsonErrorKind};
use crate::watson_err;

pub struct SizedMessageObj {
    buffer: Vec<u8>,
}

impl SizedMessageObj {
    /// The ONLY way to create a message for the wire.
    /// This guarantees Bincode is used every time.
    pub fn from_struct<T: Serialize>(data: &T) -> Result<Self, WatsonError> {
        let buffer = bincode::serialize(data)
            .map_err(|e| watson_err!(WatsonErrorKind::Serialize, e.to_string()))?;
        Ok(Self { buffer })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.buffer
    }
}

pub trait AsyncSizedMessage {
    fn write_sized<'a>(
        &'a mut self,
        what: SizedMessageObj,
    ) -> impl Future<Output = Result<(), WatsonError>> + Send + 'a;
    fn read_sized<'a>(
        &'a mut self,
    ) -> impl Future<Output = Result<Vec<u8>, WatsonError>> + Send + 'a;
}
impl AsyncSizedMessage for UnixStream {
    fn write_sized<'a>(
        &'a mut self,
        what: SizedMessageObj,
    ) -> impl Future<Output = Result<(), WatsonError>> + Send + 'a {
        async move {
            // Safely convert buf_len from usize to u32
            let buf_len: u32 = what
                .bytes()
                .len()
                .try_into()
                .map_err(|_| watson_err!(WatsonErrorKind::InvalidData, "message too long"))?;

            // Write message size to stream
            let len_bytes = buf_len.to_be_bytes();
            self.write_all(&len_bytes)
                .await
                .map_err(|e| watson_err!(WatsonErrorKind::StreamWrite, e.to_string()))?;

            // Write message to stream
            self.write(what.bytes())
                .await
                .map_err(|e| watson_err!(WatsonErrorKind::StreamWrite, e.to_string()))?;

            Ok(())
        }
    }
    fn read_sized<'a>(
        &'a mut self,
    ) -> impl Future<Output = Result<Vec<u8>, WatsonError>> + Send + 'a {
        async move {
            let mut buf_len = [0u8; 4];

            // Read message length
            self.read_exact(&mut buf_len)
                .await
                .map_err(|e| watson_err!(WatsonErrorKind::StreamRead, e.to_string()))?;
            let msg_len = u32::from_be_bytes(buf_len) as usize;

            let mut buf = vec![0u8; msg_len];
            self.read_exact(&mut buf)
                .await
                .map_err(|e| watson_err!(WatsonErrorKind::StreamRead, e.to_string()))?;

            Ok(buf)
        }
    }
}

impl AsyncSizedMessage for OwnedReadHalf {
    fn write_sized<'a>(
        &'a mut self,
        _what: SizedMessageObj,
    ) -> impl Future<Output = Result<(), WatsonError>> + Send + 'a {
        async move {
            Err(watson_err!(
                WatsonErrorKind::StreamWrite,
                "Cannot write from ReadHalf"
            ))
        }
    }
    fn read_sized<'a>(
        &'a mut self,
    ) -> impl Future<Output = Result<Vec<u8>, WatsonError>> + Send + 'a {
        async move {
            let mut buf_len = [0u8; 4];

            // Read message length
            self.read_exact(&mut buf_len)
                .await
                .map_err(|e| watson_err!(WatsonErrorKind::StreamRead, e.to_string()))?;
            let msg_len = u32::from_be_bytes(buf_len) as usize;

            let mut buf = vec![0u8; msg_len];
            self.read_exact(&mut buf)
                .await
                .map_err(|e| watson_err!(WatsonErrorKind::StreamRead, e.to_string()))?;

            Ok(buf)
        }
    }
}

impl AsyncSizedMessage for OwnedWriteHalf {
    fn write_sized<'a>(
        &'a mut self,
        what: SizedMessageObj,
    ) -> impl Future<Output = Result<(), WatsonError>> + Send + 'a {
        async move {
            // Safely convert buf_len from usize to u32
            let buf_len: u32 = what
                .bytes()
                .len()
                .try_into()
                .map_err(|_| watson_err!(WatsonErrorKind::InvalidData, "message too long"))?;

            // Write message size to stream
            let len_bytes = buf_len.to_be_bytes();
            self.write_all(&len_bytes)
                .await
                .map_err(|e| watson_err!(WatsonErrorKind::StreamWrite, e.to_string()))?;

            // Write message to stream
            self.write(what.bytes())
                .await
                .map_err(|e| watson_err!(WatsonErrorKind::StreamWrite, e.to_string()))?;

            Ok(())
        }
    }
    fn read_sized<'a>(
        &'a mut self,
    ) -> impl Future<Output = Result<Vec<u8>, WatsonError>> + Send + 'a {
        async move {
            Err(watson_err!(
                WatsonErrorKind::StreamRead,
                "Cannot read to WriteHalf"
            ))
        }
    }
}
