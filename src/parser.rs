use anyhow::{Result, anyhow};
use bytes::{Buf, BufMut, Bytes, BytesMut, buf};
use log;
use std::io::{BufRead, BufReader, Read};

pub const CRLF: &[u8] = b"\r\n";

pub struct Parser<R> {
    reader: BufReader<R>,
    buf: Vec<u8>,
}

impl<R: Read> Parser<R> {
    pub fn new(reader: BufReader<R>) -> Self {
        let buf: Vec<u8> = Vec::with_capacity(1024); // reusable byte buffer
        return Self {
            reader: reader,
            buf: buf, // reusable byte buffer
        };
    }

    pub fn parse(&mut self) -> Result<Option<Command>> {
        self.buf.clear();

        // first is to read how many commands we expect to read in the form of
        // *X\r\n where x is an arbitrary number of digits
        let bytes_read = self.reader.read_until(b'\n', &mut self.buf)?;
        log::info!("raw command {}", String::from_utf8_lossy(&mut self.buf));

        // EOF
        if bytes_read == 0 {
            return Ok(None); // EOF
        } else if bytes_read < 4 {
            return Err(anyhow!("command too short"));
        } else if bytes_read != self.buf.len() {
            return Err(anyhow!(
                "expected {} bytes got {}",
                self.buf.len(),
                bytes_read
            ));
        }

        if &self.buf[bytes_read - 2..] != b"\r\n" {
            return Err(anyhow!(
                "expected last two bytes to be '\r\n' got {:?}",
                &self.buf[bytes_read - 2..]
            ));
        } else if self.buf[0] != b'*' {
            return Err(anyhow!(
                "expected '*' got byte 0x{:02X}('{}')",
                self.buf[0],
                self.buf[0] as char
            ));
        }

        let num_str = std::str::from_utf8(&self.buf[1..bytes_read - 2])?;
        let num: u32 = num_str.parse()?;
        if num == 0 {
            return Err(anyhow!("recevied command array of 0"));
        }

        for _ in 0..num {
            self.buf.clear();
            let _bytes_read = self.reader.read_until(b'\n', &mut self.buf)?;
            log::info!("sub command {}", String::from_utf8_lossy(&mut self.buf));
        }

        let cmd = Command {
            name: CommandNames::Fake,
            args: vec![],
        };

        return Ok(Some(cmd));
    }
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    InvalidInputError,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CommandNames {
    Fake,
    Ping,
}

#[derive(Debug)]
pub struct Command {
    pub name: CommandNames,
    pub args: Vec<String>,
}

// Parses the next redis simple string (e.g. +FOOBAR\r\n) in the buffer
// and advance buf's cursor to the next valid position.
pub fn parse_simple_string(buf: &mut BytesMut) -> Result<String> {
    if buf.is_empty() || buf[0] != b'+' {
        return Err(anyhow::anyhow!("Invalid simple string format"));
    }

    for i in 1..buf.len() - 1 {
        if buf[i..].starts_with(CRLF) {
            let simple_string = std::str::from_utf8(&buf[1..i])
                .map_err(|_| anyhow::anyhow!("Invalid UTF-8 in simple string"))?
                .to_string();
            buf.advance(i + 2);
            return Ok(simple_string);
        }
    }

    Err(anyhow::anyhow!("Incomplete simple string"))
}

// Parses a redis integer (e.g. ":-123\r\n") from buf and advance
// buf's cursor to the next valid position.
pub fn parse_integer(buf: &mut BytesMut) -> Result<i64> {
    if buf.len() < 4 || buf[0] != b':' {
        return Err(anyhow::anyhow!("Invalid integer prefix"));
    }
    for i in 1..buf.len() - 1 {
        if buf[i..].starts_with(CRLF) {
            let int_bytes = &buf[1..i];
            let int_str = std::str::from_utf8(int_bytes)?;
            let val = int_str.parse::<i64>()?;
            buf.advance(i + 2);
            return Ok(val);
        }
    }

    return Err(anyhow::anyhow!("Invalid integer input"));
}

pub fn parse_bulk_string(buf: &mut BytesMut) -> Result<Bytes> {
    // $<length>\r\n<content>\r\n
    if buf.is_empty() || buf[0] != b'$' {
        return Err(anyhow::anyhow!("No bulk string code"));
    }
    buf.advance(1);
    let len_buf = parse_to_next_crlf(buf)?;
    let len_str = std::str::from_utf8(len_buf.chunk())?;
    let len = len_str.parse::<usize>()?;

    if len > buf.len() - 2 {
        return Err(anyhow::anyhow!("bulk string lenth longer than buf"));
    }

    let data = buf.split_to(len);
    if buf.len() < 2 || !buf[0] == b'\r' || buf[1] != b'\n' {
        return Err(anyhow::anyhow!("missing bulk string CRLF"));
    }
    buf.advance(2);
    return Ok(data.freeze());
}

fn parse_to_next_crlf(buf: &mut BytesMut) -> Result<Bytes> {
    if buf.len() < 2 {
        return Err(anyhow::anyhow!("buf too short for CRLF"));
    }
    for i in 0..buf.len() - 1 {
        if buf[i..].starts_with(CRLF) {
            let slice = buf.split_to(i);
            buf.advance(2); // Skip the CRLF
            return Ok(slice.freeze());
        }
    }
    return Err(anyhow::anyhow!("No CRLF found"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bulk_string() {
        // $<length>\r\n<content>\r\n
        let cases = [
            ("$0\r\n\r\n", ""),
            ("$1\r\nM\r\n", "M"),
            ("$2\r\nOK\r\n", "OK"),
            ("$4\r\nPING\r\n", "PING"),
            ("$2\r\nñ\r\n", "ñ"), // <== 2-byte UTF-8 character
        ];
        for (input, expected) in cases {
            let mut buf = BytesMut::from(input);
            assert_eq!(parse_bulk_string(&mut buf).unwrap(), expected);
            assert!(buf.is_empty());
        }

        let multi = "$0\r\n\r\n$1\r\n1\r\n$2\r\n10\r\n";
        let mut buf = BytesMut::from(multi);
        assert_eq!(parse_bulk_string(&mut buf).unwrap(), "");
        assert_eq!(parse_bulk_string(&mut buf).unwrap(), "1");
        assert_eq!(parse_bulk_string(&mut buf).unwrap(), "10");
        assert!(buf.is_empty());
    }

    #[test]
    fn test_parse_bulk_string_errs() {
        // $<length>\r\n<content>\r\n
        let cases = [
            "0",
            "$",
            "$\r\n",
            "$\r\n\r",
            "$\r\n\r\n",
            "$0\r\na\r\n",
            "$1\r\nab\r\n",
            "$2\r\na\r\n",
            "$1\r\n\r\n",
            "$1\r\nñ\r\n", // unicode
            "$3\r\nñ\r\n",
        ];
        for input in cases {
            let mut buf = BytesMut::from(input);
            assert!(parse_bulk_string(&mut buf).is_err());
        }
    }

    #[test]
    fn test_parse_integer_ok() {
        let cases = [
            (":1234\r\n", 1234),
            (":1\r\n", 1),
            (":+12\r\n", 12),
            (":-1\r\n", -1),
            (":-123\r\n", -123),
            (":-6666666\r\n", -6666666),
        ];

        for (input, expected) in &cases {
            let mut buf = BytesMut::from(*input);
            let result = parse_integer(&mut buf).unwrap();
            assert_eq!(result, *expected, "input: {:?}", input);
            assert!(buf.is_empty());
        }
    }

    #[test]
    fn test_parse_multiple_integers() {
        let mut double = BytesMut::from(":1\r\n:10\r\n:100\r\n");
        assert_eq!(parse_integer(&mut double).unwrap(), 1);
        assert_eq!(parse_integer(&mut double).unwrap(), 10);
        assert_eq!(parse_integer(&mut double).unwrap(), 100);
        assert!(double.is_empty());
    }

    #[test]
    fn test_parse_integer_errs() {
        let cases = ["1234\r\n", "", ":-1\r", ":-6666666\n", ":abc\r\n"];

        for input in &cases {
            let mut buf = BytesMut::from(*input);
            let result = parse_integer(&mut buf);
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_parse_simple_string_ok() {
        let cases = [("+OK\r\n", "OK"), ("+abcd\r\n", "abcd"), ("+\r\n", "")];

        for (input, expected) in &cases {
            let mut buf = BytesMut::from(*input);
            assert_eq!(parse_simple_string(&mut buf).unwrap(), *expected);
            assert!(buf.is_empty());
        }

        let mut double_buf = BytesMut::from("+OK1\r\n+OK2\r\n");
        assert_eq!(parse_simple_string(&mut double_buf).unwrap(), "OK1");
        assert_eq!(parse_simple_string(&mut double_buf).unwrap(), "OK2");
        assert!(double_buf.is_empty());
    }

    #[test]
    fn test_parse_simple_string_errs() {
        let cases = ["+OK", "+OK\r", "+OK\n", "+", "", "OK", ":123\r\n"];

        for input in &cases {
            let mut b = BytesMut::from(*input);
            assert!(parse_simple_string(&mut b).is_err());
        }
    }
}
