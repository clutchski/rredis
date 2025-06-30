use std::io::{BufRead, Read, BufReader};
use anyhow::{Result, anyhow};
use log;


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
        }
    }

    pub fn parse(&mut self) -> Result<Option<Command>> {
        self.buf.clear();

        // first is to read how many commands we expect to read in the form of
        // *X\r\n where x is an arbitrary number of digits
        let bytes_read = self.reader.read_until(b'\n', &mut self.buf)?;
        log::info!("raw command {}", String::from_utf8_lossy(&mut self.buf));

        // EOF
        if bytes_read == 0 {
            return Ok(None) // EOF
        } else if bytes_read < 4 {
            return Err(anyhow!("command too short"));
        } else if bytes_read != self.buf.len() {
            return Err(anyhow!("expected {} bytes got {}", self.buf.len(), bytes_read));
        }
        
        if &self.buf[bytes_read-2..] != b"\r\n" {
            return Err(anyhow!("expected last two bytes to be '\r\n' got {:?}", &self.buf[bytes_read-2..]));
        } else if self.buf[0] != b'*' {
            return Err(anyhow!("expected '*' got byte 0x{:02X}('{}')", self.buf[0], self.buf[0] as char));
        }

        let num_str = std::str::from_utf8(&self.buf[1..bytes_read-2])?;
        let num: u32 = num_str.parse()?;
        if num == 0 {
            return Err(anyhow!("recevied command array of 0"));
        }

        for _ in 0..num {
            self.buf.clear();
            let bytes_read = self.reader.read_until(b'\n', &mut self.buf)?;
            log::info!("sub command {}", String::from_utf8_lossy(&mut self.buf));
        }

        let cmd = Command {
            name: CommandNames::Fake,
            args: vec![],
        };

        return Ok(Some(cmd));
    }
}

#[derive(Debug, PartialEq, Eq)]
enum CommandNames {
    Fake,
    Ping,
    Incr,
    Get,
    Set
}

#[derive(Debug)]
pub struct Command {
    pub name: CommandNames,
    pub args: Vec<String>,
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_parser_ping() {
        let input = b"*1\r\n$4\r\nPING\r\n";
        let reader = BufReader::new(Cursor::new(input));
        let mut parser = Parser::new(reader);
        let result = parser.parse();
        assert!(result.is_ok());
        let option = result.unwrap();
        assert!(option.is_some());
        let command = option.unwrap();
        assert_eq!(command.name, CommandNames::Ping);
    }
}