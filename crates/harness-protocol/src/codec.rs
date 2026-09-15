//! Newline-delimited framing: exactly one JSON-RPC message per line.

use crate::rpc::RpcMessage;
use std::io::{self, BufRead, Write};

#[derive(Debug)]
pub enum ReadError {
    /// The peer closed its end.
    Eof,
    /// A non-empty line that is not a JSON-RPC message. `line` is kept verbatim for the report.
    Malformed {
        line: String,
        error: String,
    },
    Io(io::Error),
}

/// Write `msg` as one line and flush, so the peer sees it immediately.
pub fn write_message<W: Write>(w: &mut W, msg: &RpcMessage) -> io::Result<()> {
    let line = serde_json::to_string(msg).map_err(io::Error::other)?;
    w.write_all(line.as_bytes())?;
    w.write_all(b"\n")?;
    w.flush()
}

/// Read the next message, skipping blank lines.
pub fn read_message<R: BufRead>(r: &mut R) -> Result<RpcMessage, ReadError> {
    loop {
        let mut line = String::new();
        let n = r.read_line(&mut line).map_err(ReadError::Io)?;
        if n == 0 {
            return Err(ReadError::Eof);
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        return serde_json::from_str(trimmed).map_err(|e| ReadError::Malformed {
            line: trimmed.to_string(),
            error: e.to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{method, Empty, RpcMessage};
    use std::io::Cursor;

    #[test]
    fn write_then_read_round_trips_and_skips_blank_lines() {
        let mut buf = Vec::new();
        write_message(
            &mut buf,
            &RpcMessage::notification(method::UNIT_EVENT, &Empty {}),
        )
        .unwrap();
        buf.extend_from_slice(b"\n\n");
        write_message(&mut buf, &RpcMessage::response(3, &Empty {})).unwrap();
        assert_eq!(buf.iter().filter(|b| **b == b'\n').count(), 4);

        let mut r = Cursor::new(buf);
        assert_eq!(
            read_message(&mut r).unwrap().method.as_deref(),
            Some("unit/event")
        );
        assert_eq!(read_message(&mut r).unwrap().id, Some(3));
        assert!(matches!(read_message(&mut r), Err(ReadError::Eof)));
    }

    #[test]
    fn a_non_json_line_is_malformed_and_keeps_the_raw_line() {
        let mut r = Cursor::new(b"this is not json\n".to_vec());
        match read_message(&mut r) {
            Err(ReadError::Malformed { line, .. }) => assert_eq!(line, "this is not json"),
            other => panic!("expected Malformed, got {other:?}"),
        }
    }
}
