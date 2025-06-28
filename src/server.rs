use log;

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

#[derive(Debug)]
pub struct Server {
    pub addr: String,
    pub listener: TcpListener,
}

impl Server {
    pub fn new(host: &str, port: u16) -> Result<Self, std::io::Error> {
        let addr = format!("{}:{}", host, port);
        let listener = TcpListener::bind(&addr)?;

        return Ok(Self {
            addr: addr,
            listener: listener,
        });
    }

    pub fn run(&self) {
        log::info!("Starting server {}", self.addr);
        for stream in self.listener.incoming() {
            match stream {
                Ok(stream) => self.handle_stream(stream),
                Err(e) => {
                    log::error!("error connecting {}", e)
                }
            }
        }
    }

    fn handle_stream(&self, stream: TcpStream) {
        let peer_addr = stream.peer_addr().unwrap();
        log::info!("incoming connection {}", peer_addr);

        let mut reader = BufReader::new(&stream);
        let mut writer = &stream;

        let mut line = String::new();
        while let Ok(bytes_read) = reader.read_line(&mut line) {
            if bytes_read == 0 {
                break; // connection closed
            }

            print!("Received: {}", line); // print on server
            if let Err(e) = writer.write(line.as_bytes()) {
                eprintln!("Failed to write: {}", e);
                break;
            }

            line.clear(); // reset buffer for next line
        }
    }
}
