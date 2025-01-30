use std::{net::TcpListener, os::unix::net::UnixListener, thread};

use crate::{AppResult, Connection};

use super::Server;

impl Server {
    /// Start a unix listener on a new thread on [`Self::SOCKET_PATH`].
    pub fn start_unix_listener(&mut self) -> AppResult<()> {
        Self::cleanup_socket();

        let listener = UnixListener::bind(Self::SOCKET_PATH)?;
        println!("Unix socket listening on {:?}", listener.local_addr()?);

        let state = self.state.clone();
        let config = self.config.clone();
        thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        println!("New unix socket connection");

                        let player = {
                            let mut state = state.write().unwrap();
                            let connection = Connection::unix(stream).unwrap(); // TODO handle error
                            state.create_player(connection)
                        };

                        let state = state.clone();
                        let config = config.clone();
                        thread::spawn(move || {
                            let id = player.read().unwrap().id;

                            match Self::handle_client(player, state.clone(), config) {
                                Ok(_) => println!("Client disconnected"),
                                Err(err) => eprintln!("Closing connection -> Client error: {:?}", err),
                            }

                            let mut state = state.write().unwrap();
                            state.players.remove(&id);
                        });
                    }
                    Err(err) => {
                        eprintln!("Unix socket connection failed: {:?}", err);
                    }
                }
            }
        });

        Ok(())
    }

    /// Start a TCP listener on a new thread with a OS chosen port.
    pub fn start_tcp_listener(&mut self) -> AppResult<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        println!("TCP socket listening on {}", listener.local_addr()?);

        let state = self.state.clone();
        let config = self.config.clone();
        thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let peer_addr = match stream.peer_addr() {
                            Ok(addr) => addr,
                            Err(err) => {
                                eprintln!("Failed to get peer address: {:?}", err);
                                continue;
                            }
                        };
                        println!("New TCP connection: {:?}", peer_addr);

                        let player = {
                            let mut state = state.write().unwrap();
                            let connection = Connection::tcp(stream).unwrap(); // TODO handle error
                            state.create_player(connection)
                        };

                        let state = state.clone();
                        let config = config.clone();
                        thread::spawn(move || {
                            match Self::handle_client(player, state.clone(), config) {
                                Ok(_) => println!("Client disconnected: {:?}", peer_addr),
                                Err(err) => eprintln!("Closing connection -> Client error: {:?}", err),
                            }
                        });
                    }
                    Err(err) => {
                        eprintln!("TCP connection failed: {:?}", err);
                    }
                }
            }
        });

        Ok(())
    }
}
