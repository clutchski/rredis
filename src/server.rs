use anyhow::Result;
use log;
use thiserror::Error;

use std::io::{BufRead, BufReader, Read};
use std::net::{TcpListener, TcpStream};

use crate::parser::Parser;

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
                Ok(stream) => {
                    if let Err(e) = self.handle_stream(stream) {
                        log::error!("stream handling failed: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    log::error!("error connecting {}", e);
                }
            }
        }
        log::debug!("finished run loop");
    }

    fn handle_stream(&self, stream: TcpStream) -> Result<()> {
        let peer_addr = stream.peer_addr().unwrap();
        log::info!("incoming connection {}", peer_addr);

        let reader = BufReader::new(&stream);
        //let mut writer = &stream;

        let mut parser = Parser::new(reader);
        loop {
            let cmd = parser.parse()?;
            match cmd {
                Some(cmd) => {
                    log::info!("received command: {:?}", cmd);
                }
                None => {
                    break;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("network error: {0}")]
    Network(#[from] std::io::Error),

    #[error("protocol error: {0}")]
    Protocol(String),
}