mod server;

use server::Server;

fn main() {
    env_logger::init();

    let s = Server::new("localhost", 6666).unwrap();
    s.run();
}
