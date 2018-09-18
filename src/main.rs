//! Copyright (C) 2018 Yannick Heinrich
//! This program is free software; you can redistribute it and/or modify it under the terms of the
//! GNU General Public License as published by the Free Software Foundation; version 2.
//!
//! This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY;
//! without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
//! See the GNU General Public License for more details.
//!
//! You should have received a copy of the GNU General Public License along with this program;
//! if not, write to the Free Software Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA 02110-1301, USA.

extern crate base64;
extern crate mio;

use mio::net::TcpStream;
use mio::unix::EventedFd;
use mio::*;
use std::fs::File;
use std::io::{Read, Write};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn usage() {
    println!("corkscrew-rs {} (yannick.heinrich@gmail.com)\n\n", VERSION);
    println!("usage: corkscrew <proxyhost> <proxyport> <desthost> <destport> [authfile]\n");
}

fn connection_string(
    dest_host: &str,
    dest_port: &str,
    auth_file: Option<&String>,
) -> Option<String> {
    dest_port.parse::<u32>().ok()?;

    let prefix = format!("CONNECT {}:{} HTTP/1.0", dest_host, dest_port);
    let suffix = "\r\n\r\n";
    match auth_file {
        None => Some(prefix + suffix),
        Some(auth_file) => {
            let mut file = File::open(auth_file).ok()?;

            let mut buffer = String::new();
            file.read_to_string(&mut buffer).ok()?;

            let encoded = base64::encode(&buffer);
            Some(prefix + &format!("\nProxy-Authorization: Basic {}", encoded) + suffix)
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let host: &String;
    let port: &String;
    let dest_host: &String;
    let dest_port: &String;
    let auth_file: Option<&String>;

    match args.len() {
        5 => {
            host = &args[1];
            port = &args[2];
            dest_host = &args[3];
            dest_port = &args[4];
            auth_file = None;
        }

        6 => {
            host = &args[1];
            port = &args[2];
            dest_host = &args[3];
            dest_port = &args[4];
            auth_file = Some(&args[5])
        }
        _ => {
            usage();
            std::process::exit(-1);
        }
    }

    let string = connection_string(dest_host, dest_port, auth_file)
        .expect("Destination adress seeems invalid.");

    let poll = Poll::new().unwrap();

    // STDIN read
    const IN: Token = Token(0);
    const SOCK: Token = Token(2);

    let mut buff: [u8; 4096] = [0; 4096];

    let fd0_e = EventedFd(&0);
    poll.register(&fd0_e, IN, Ready::readable(), PollOpt::level())
        .unwrap();

    // Socket
    let addr = format!("{}:{}", host, port).parse().unwrap();
    // Setup the client socket
    let mut sock = TcpStream::connect(&addr).unwrap();
    let interest = Ready::writable() | Ready::readable();
    // Register the socket
    poll.register(&sock, SOCK, interest, PollOpt::level())
        .unwrap();

    let mut stdout = std::io::stdout();
    let mut stdin = std::io::stdin();

    let mut setup = false;
    let mut sent = false;
    let mut events = Events::with_capacity(1024);
    'outer: loop {
        poll.poll(&mut events, None).unwrap();

        for event in &events {
            match event.token() {
                SOCK if !setup => {
                    if !setup && event.readiness().is_readable() {
                        let len = sock.read(&mut buff).unwrap_or(0);
                        if len < 1 {
                            break 'outer;
                        } else {
                            let proxy_answer = std::str::from_utf8(&buff[..]);
                            match proxy_answer {
                                Ok(resp) if resp.len() >= 2 && resp.starts_with("HTTP/1") => {
                                    let status_code = resp[9..12].parse::<u16>().unwrap_or(0);
                                    if status_code >= 200 && status_code < 300 {
                                        setup = true
                                    } else {
                                        break 'outer;
                                    }
                                }
                                _ => break 'outer,
                            }
                        }
                    }

                    if event.readiness().is_writable() && !sent {
                        let len = sock.write(string.as_bytes()).unwrap_or(0);
                        if len < 1 {
                            break 'outer;
                        }
                        sent = true
                    }
                }

                SOCK if setup && event.readiness().is_readable() => {
                    let mut len = sock.read(&mut buff).unwrap_or(0);
                    if len < 1 {
                        break 'outer;
                    }
                    len = stdout.write(&buff[..len]).unwrap_or(0);
                    if len < 1 {
                        break 'outer;
                    }

                    if stdout.flush().is_err() {
                        break 'outer;
                    }
                }
                IN if setup && event.readiness().is_readable() => {
                    let mut len = stdin.read(&mut buff).unwrap_or(0);
                    if len < 1 {
                        break 'outer;
                    }
                    len = sock.write(&buff[..len]).unwrap_or(0);
                    if len < 1 {
                        break 'outer;
                    }

                    if sock.flush().is_err() {
                        break 'outer;
                    }
                }

                _ => {}
            }
        }
    }
}
