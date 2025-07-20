use anyhow::{Result, anyhow};
use bytes::{Buf, Bytes, BytesMut};
use std::io::{BufReader, Read};

pub const CRLF: &[u8] = b"\r\n";

pub struct Parser<R> {
    reader: BufReader<R>,
    buf: BytesMut,
}

impl<R: Read> Parser<R> {
    pub fn new(reader: BufReader<R>) -> Self {
        let buf = BytesMut::with_capacity(1024);
        return Self {
            reader: reader,
            buf: buf, // reusable byte buffer
        };
    }

    pub fn parse(&mut self) -> Result<Option<Arg>> {
        self.read()?; // fill our buffer until EOF

        let args = parse_array(&mut self.buf);

        return Err(anyhow::anyhow!("can't parse empty arg"));
    }

    fn read(&mut self) -> Result<()> {
        self.buf.clear();
        loop {
            // Ensure space to read into
            self.buf.reserve(1024);

            // Reserve space for reading
            let buf_len = self.buf.len();
            self.buf.resize(buf_len + 1024, 0);

            // Read into the buffer
            let n = self.reader.read(&mut self.buf[buf_len..])?;
            if n == 0 {
                // No data read, remove the reserved space
                self.buf.truncate(buf_len);
                break; // EOF reached
            }

            // Adjust buffer size to actual data read
            self.buf.truncate(buf_len + n);
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum CommandName {
    Fake,
    Ping,
}

#[derive(Debug)]
pub struct Command {
    pub name: CommandName,
    pub args: Vec<Arg>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Arg {
    String(String),
    Bytes(Bytes),
    Int(i64),
    Array(Vec<Arg>),
}

pub fn parse_arg(buf: &mut BytesMut) -> Result<Arg> {
    if buf.is_empty() {
        return Err(anyhow::anyhow!("can't parse empty arg"));
    }

    let peek = buf[0];

    match peek {
        b':' => {
            let i = parse_integer(buf)?;
            return Ok(Arg::Int(i));
        }
        b'+' => {
            let s = parse_simple_string(buf)?;
            return Ok(Arg::String(s));
        }
        b'$' => {
            let b = parse_bulk_string(buf)?;
            return Ok(Arg::Bytes(b));
        }
        b'*' => return Ok(parse_array(buf)?),
        _ => return Err(anyhow!("Invalid arg prefix")),
    }
}

// Parses the next redis simple string (e.g. +FOOBAR\r\n) in the buffer
// and advance buf's cursor to the next valid position.
pub fn parse_simple_string(buf: &mut BytesMut) -> Result<String> {
    if buf.is_empty() || buf[0] != b'+' {
        return Err(anyhow!("Invalid simple string format"));
    }

    for i in 1..buf.len() - 1 {
        if buf[i..].starts_with(CRLF) {
            let simple_string = std::str::from_utf8(&buf[1..i])
                .map_err(|_| anyhow!("Invalid UTF-8 in simple string"))?
                .to_string();
            buf.advance(i + 2);
            return Ok(simple_string);
        }
    }

    Err(anyhow!("Incomplete simple string"))
}

// Parses a redis integer (e.g. ":-123\r\n") from buf and advance
// buf's cursor to the next valid position.
pub fn parse_integer(buf: &mut BytesMut) -> Result<i64> {
    if buf.len() < 4 || buf[0] != b':' {
        return Err(anyhow!("Invalid integer prefix"));
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

    let bulk_str = buf.split_to(len);
    if buf.len() < 2 || !buf.starts_with(CRLF) {
        return Err(anyhow::anyhow!("missing bulk string CRLF"));
    }
    buf.advance(2);
    return Ok(bulk_str.freeze());
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

fn parse_array(buf: &mut BytesMut) -> Result<Arg> {
    if buf.is_empty() || buf[0] != b'*' {
        return Err(anyhow::anyhow!("No array prefix found"));
    }

    buf.advance(1);
    let len_buf = parse_to_next_crlf(buf)?;
    let len_str = std::str::from_utf8(len_buf.chunk())?;
    let len = len_str.parse::<usize>()?;

    let mut args: Vec<Arg> = Vec::with_capacity(len);

    for _ in 0..len {
        let arg = parse_arg(buf)?;
        args.push(arg);
    }

    return Ok(Arg::Array(args));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_array() {
        let set = Arg::Bytes(Bytes::from("SET"));
        let get = Arg::Bytes(Bytes::from("GET"));
        let ping = Arg::Bytes(Bytes::from("PING"));
        let key = Arg::Bytes(Bytes::from("mykey"));
        let val = Arg::Bytes(Bytes::from("myvalue"));
        let cases = [
            ("*1\r\n$3\r\nSET\r\n", vec![set.clone()]),
            ("*1\r\n$4\r\nPING\r\n", vec![ping]),
            (
                "*2\r\n$3\r\nGET\r\n$5\r\nmykey\r\n",
                vec![get.clone(), key.clone()],
            ),
            (
                "*3\r\n$3\r\nSET\r\n$5\r\nmykey\r\n$7\r\nmyvalue\r\n",
                vec![set.clone(), key.clone(), val],
            ),
        ];
        for (input, expected) in cases {
            let arr = Arg::Array(expected);
            let out = parse_array(&mut BytesMut::from(input));
            assert_eq!(out.unwrap(), arr);
        }
    }

    #[test]
    fn test_parse_arg() {
        let cases = [
            ("$2\r\nOK\r\n", Arg::Bytes(Bytes::from("OK"))),
            (":+12\r\n", Arg::Int(12)),
            ("+OK\r\n", Arg::String("OK".into())),
            (
                "*1\r\n$3\r\nSET\r\n",
                Arg::Array(vec![Arg::Bytes(Bytes::from("SET"))]),
            ),
        ];
        for (input, expected) in cases {
            let mut buf = BytesMut::from(input);
            let arg = parse_arg(&mut buf).unwrap();
            assert_eq!(arg, expected);
            assert!(buf.is_empty());
        }
    }

    #[test]
    fn test_parse_arg_errors() {
        let cases = ["", "$", "A", "!"];
        for input in cases {
            let mut buf = BytesMut::from(input);
            assert!(parse_arg(&mut buf).is_err());
        }
    }

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
