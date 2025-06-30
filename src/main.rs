mod server;
mod parser;

use log;
use server::Server;

fn main() {
    env_logger::init();

    let s = Server::new("127.0.0.1", 6666).unwrap();
    s.run();
    log::debug!("bye bye");
}
