//! Chrome/Firefox native messaging framing: each message is a 4-byte
//! little-endian length prefix followed by that many bytes of UTF-8
//! JSON. This is the wire format browsers use to talk to a native
//! messaging host's stdin/stdout — see
//! <https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging>.
//!
//! Chrome enforces a 1MB message-size limit for host→browser messages;
//! we mirror that here as a sanity bound so a corrupt length prefix
//! can't make us try to allocate an absurd buffer.

use std::io::{self, Read, Write};

use serde::de::DeserializeOwned;
use serde::Serialize;

const MAX_MESSAGE_BYTES: u32 = 1024 * 1024;

/// Reads one framed message, or `Ok(None)` on clean EOF (the browser
/// closed the port).
pub fn read_message<R: Read, T: DeserializeOwned>(r: &mut R) -> io::Result<Option<T>> {
    let mut len_buf = [0u8; 4];
    match r.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_le_bytes(len_buf);
    if len > MAX_MESSAGE_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "native message exceeds size limit"));
    }
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf)?;
    let value = serde_json::from_slice(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(Some(value))
}

/// Writes one framed message and flushes.
pub fn write_message<W: Write, T: Serialize>(w: &mut W, value: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() as u64 > MAX_MESSAGE_BYTES as u64 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "outgoing native message exceeds size limit"));
    }
    w.write_all(&(bytes.len() as u32).to_le_bytes())?;
    w.write_all(&bytes)?;
    w.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::io::Cursor;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Sample {
        value: String,
    }

    #[test]
    fn round_trips_a_message() {
        let mut buf = Vec::new();
        write_message(&mut buf, &Sample { value: "hello".into() }).unwrap();
        let mut cursor = Cursor::new(buf);
        let decoded: Sample = read_message(&mut cursor).unwrap().unwrap();
        assert_eq!(decoded, Sample { value: "hello".into() });
    }

    #[test]
    fn empty_input_is_clean_eof() {
        let mut cursor = Cursor::new(Vec::<u8>::new());
        let decoded: Option<Sample> = read_message(&mut cursor).unwrap();
        assert!(decoded.is_none());
    }

    #[test]
    fn oversized_length_prefix_is_rejected() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&(MAX_MESSAGE_BYTES + 1).to_le_bytes());
        let mut cursor = Cursor::new(buf);
        let result: io::Result<Option<Sample>> = read_message(&mut cursor);
        assert!(result.is_err());
    }

    #[test]
    fn truncated_body_is_an_error_not_a_panic() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&100u32.to_le_bytes());
        buf.extend_from_slice(b"short");
        let mut cursor = Cursor::new(buf);
        let result: io::Result<Option<Sample>> = read_message(&mut cursor);
        assert!(result.is_err());
    }
}
