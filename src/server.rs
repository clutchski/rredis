use anyhow::Result;
use log;

use std::io::BufReader;
use std::net::{Shutdown, TcpListener, TcpStream};

use crate::parser::Parser;

pub struct Server {
    pub addr: String,
    pub listener: TcpListener,
}

impl Server {
    pub fn new(host: &str, port: u16) -> Result<Self, std::io::Error> {
        let addr = format!("{host}:{port}");
        let listener = TcpListener::bind(&addr)?;

        log::info!("rredis running on {addr}");

        Ok(Self { addr, listener })
    }

    pub fn run(&self) {
        log::info!("Starting server {}", self.addr);
        for stream in self.listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    let result = self.handle_stream(&mut stream);
                    if result.is_err() {
                        log::error!("err running command {result:?}");
                        let _ = stream.shutdown(Shutdown::Both);
                    }
                    break;
                }
                Err(e) => {
                    log::error!("error connecting {e}");
                }
            }
        }
        log::debug!("finished run loop");
    }

    fn handle_stream(&self, stream: &mut TcpStream) -> Result<()> {
        let peer_addr = stream.peer_addr().unwrap();
        log::info!("incoming connection {peer_addr}");

        let reader = BufReader::new(stream);

        let mut parser = Parser::new(reader);
        loop {
            let cmd = parser.parse()?;
            match cmd {
                Some(cmd) => {
                    log::info!("received command: {cmd:?}");
                }
                None => {
                    break;
                }
            }
        }
        Ok(())
    }
}
