//! JSON-RPC transport layer for the LSP server.
//!
//! Handles reading Content-Length framed messages from stdin
//! and writing JSON-RPC responses to stdout.

use std::io::{self, BufRead, Write as IoWrite};

use serde_json::Value;

/// Read the Content-Length header from the input stream
pub(crate) fn read_content_length(reader: &mut impl BufRead) -> io::Result<usize> {
    let mut header = String::new();
    loop {
        header.clear();
        let bytes_read = reader.read_line(&mut header)?;
        if bytes_read == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "EOF"));
        }
        let header = header.trim();
        if header.is_empty() {
            continue;
        }
        if let Some(len_str) = header.strip_prefix("Content-Length: ") {
            let len: usize = len_str.parse().map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidData, format!("Invalid length: {}", e))
            })?;
            // Read the empty line after headers
            let mut empty = String::new();
            reader.read_line(&mut empty)?;
            return Ok(len);
        }
    }
}

/// Send a JSON-RPC message to stdout
pub(crate) fn send_message(msg: &Value) -> io::Result<()> {
    let body = serde_json::to_string(msg)?;
    let stdout = io::stdout();
    let mut out = stdout.lock();
    write!(out, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
    out.flush()
}
