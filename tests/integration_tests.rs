use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

use rredis::server::Server;

fn start_test_server() -> (u16, thread::JoinHandle<()>) {
    let port = 0; // Let OS assign available port
    let server = Server::new("127.0.0.1", port).unwrap();
    let actual_port = server.listener.local_addr().unwrap().port();

    let handle = thread::spawn(move || {
        server.run();
    });

    // Give server time to start
    thread::sleep(Duration::from_millis(100));

    (actual_port, handle)
}

fn connect_to_server(port: u16) -> TcpStream {
    TcpStream::connect(format!("127.0.0.1:{port}")).unwrap()
}

fn send_command(stream: &mut TcpStream, command: &[u8]) -> Vec<u8> {
    stream.write_all(command).unwrap();

    let mut response = vec![0; 1024];
    let n = stream.read(&mut response).unwrap();
    response.truncate(n);
    response
}

#[test]
fn test_server_starts() {
    let (_port, _handle) = start_test_server();
    // Test passes if server starts without panic
}

#[test]
fn test_ping_command() {
    let (port, _handle) = start_test_server();
    let mut stream = connect_to_server(port);

    let ping_cmd = b"*1\r\n$4\r\nPING\r\n";
    let response = send_command(&mut stream, ping_cmd);

    assert_eq!(response, b"+PONG\r\n");
}
