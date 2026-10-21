//! Streaming JSON limits: reject during encoding, before growing an unbounded Rust buffer.
use super::api::{PluginError, PluginResult};
use serde::Serialize;
use std::io::{self, Write};

struct Writer {
    len: usize,
    limit: usize,
    bytes: Option<Vec<u8>>,
    exceeded: bool,
}
impl Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit - self.len {
            self.exceeded = true;
            return Err(io::Error::other("JSON size limit"));
        }
        if let Some(buffer) = &mut self.bytes {
            buffer.extend_from_slice(bytes);
        }
        self.len += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn encode<T: Serialize + ?Sized>(value: &T, limit: usize, retain: bool) -> PluginResult<Writer> {
    let mut writer = Writer {
        len: 0,
        limit,
        bytes: retain.then(Vec::new),
        exceeded: false,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| {
        if writer.exceeded {
            PluginError::LimitExceeded
        } else {
            PluginError::Unavailable
        }
    })?;
    Ok(writer)
}
pub(crate) fn encoded_len<T: Serialize + ?Sized>(value: &T, limit: usize) -> PluginResult<usize> {
    Ok(encode(value, limit, false)?.len)
}
pub(crate) fn to_string<T: Serialize + ?Sized>(value: &T, limit: usize) -> PluginResult<String> {
    let bytes = encode(value, limit, true)?
        .bytes
        .expect("retained JSON buffer");
    Ok(String::from_utf8(bytes).expect("serde_json writes UTF-8"))
}

#[cfg(test)]
#[path = "tests/bounded_json_tests.rs"]
mod tests;
