use std::{io::{stdin, stdout, BufRead, Read, Write}, net::TcpStream, os::unix::net::UnixStream, sync::{Arc, Mutex}, thread};

use server_app::{AppError, AppResult, Command, Packet, ReadBytes, WriteBytes, Connection};

fn main() -> AppResult<()> {
    Client::new().run()
}

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
enum GameState {
    Guessing,
    Hinting,
    Menu,
    #[default]
    Auth,
}

struct ClientConfig {
    state: GameState,
    id: Option<String>,
    opponent_id: Option<String>,
}

impl ClientConfig {
    fn new() -> Self {
        Self {
            state: Default::default(),
            id: None,
            opponent_id: None,
        }
    }
}

struct Client {
    connection: Arc<Mutex<Option<Connection>>>,
    config: Arc<Mutex<ClientConfig>>,
}

impl Client {
    const SOCKET_PATH: &'static str = "/tmp/game-guess-a-word-socket";

    fn new() -> Self {
        Self {
            connection: Arc::new(Mutex::new(None)),
            config: Arc::new(Mutex::new(ClientConfig::new())),
        }
    }

    /// Read a line from stdin with a prompt.
    fn prompt(prompt: &str) -> AppResult<String> {
        let mut line = String::new();

        let mut stdin = stdin().lock();
        print!("{}: ", prompt);
        stdout().flush()?;
        stdin.read_line(&mut line)?;

        Ok(line.trim().to_string())
    }

    fn connect_tcp(&mut self) -> AppResult<()> {
        let addr = Self::prompt("Enter server address")?;

        let stream = match TcpStream::connect(&addr) {
            Ok(stream) => stream,
            Err(err) => {
                eprintln!("Failed to connect to server {:?}", addr);
                return Err(err)?;
            }
        };

        *self.connection.lock().unwrap() = Some(Connection::tcp(stream)?);
        Ok(())
    }

    fn connect_unix(&mut self) -> AppResult<()> {
        let stream = match UnixStream::connect(Self::SOCKET_PATH) {
            Ok(stream) => stream,
            Err(err) => {
                eprintln!("Failed to connect to unix socket {:?}", Self::SOCKET_PATH);
                return Err(err)?;
            }
        };

        *self.connection.lock().unwrap() = Some(Connection::unix(stream)?);
        Ok(())
    }

    fn connect_to_server(&mut self) -> AppResult<()> {
        loop {
            let input = Self::prompt("Enter connection type (unix/tcp)")?;
            
            match input.as_str() {
                "unix" => self.connect_unix()?, 
                "tcp" => self.connect_tcp()?,
                _ => {
                    println!("Invalid connection type");
                    continue;
                }
            }

            return Ok(());
        };
    }

    fn run(mut self) -> AppResult<()> {
        self.connect_to_server()?;
        
        self.start_game()?;

        Ok(())
    }

    fn start_game(&mut self) -> AppResult<()> {
        let connection = self.connection.clone();
        let config = self.config.clone();
        thread::spawn(move || if let Err(err) = Self::handle_input(connection, config) {
            eprintln!("Input handling thread failed: {:?}", err);
        });

        let stream = self.connection.lock().unwrap().as_ref().expect("No connection").reader.clone();
        let mut lock = stream.lock().unwrap();
        let reader = lock.reader();

        loop {
            let command = Self::receive(reader)?;
            self.handle_command(command)?;
        }
    }

    fn handle_input(connection: Arc<Mutex<Option<Connection>>>, config: Arc<Mutex<ClientConfig>>) -> AppResult<()> {
        loop {
            thread::sleep(std::time::Duration::from_millis(10));

            let state = {
                config.lock().unwrap().state
            };

            if state == GameState::Guessing {
                let guess = Self::prompt("Enter guess")?;
                let mut lock = connection.lock().unwrap();
                let stream = lock.as_mut().expect("No connection").writer();
                if !guess.is_empty() {
                    Self::send(stream, Command::Guess(guess))?;
                }
                continue;
            }

            if state == GameState::Menu || state == GameState::Hinting {
                let in_menu = state == GameState::Menu;
                let command = Self::prompt("")?;

                let command = match command.as_str() {
                    "opponents" => Some(Command::OpponentsRequest),
                    "help" => {
                        let commands = vec![
                            ("opponents", "list opponents"),
                            ("hint [new_hint]", "send a hint"),
                            ("exit", "exit the match"),
                            ("surrender", "surrender the match"),
                            ("help", "show all commands"),
                            ("match [opponent_id]", "start a match"),
                        ];

                        let commands = commands.iter().map(|(cmd, desc)| format!("\n    {cmd} - {desc}")).collect::<String>();
                        println!("Commands: {}", commands);
                        None
                    }
                    s if s.starts_with("match ") => {
                        if in_menu {
                            match s.splitn(2, " ").nth(1) {
                                Some(id) => {
                                    let mut config = config.lock().unwrap();
                                    config.opponent_id = Some(id.to_string());
                                    config.state = GameState::Hinting;
                                    Some(Command::RequestMatch(id.to_string()))
                                },
                                None => {
                                    println!("Invalid player id");
                                    None
                                }
                            }
                        } else {
                            println!("cannot start a match while playing");
                            None
                        }
                    }
                    s if s.starts_with("hint ") => {
                        if in_menu {
                            println!("you are not in a game");
                            None
                        } else {
                            match s.splitn(2, " ").nth(1) {
                                Some(hint) => Some(Command::Hint(hint.to_string())),
                                None => {
                                    println!("Invalid hint");
                                    None
                                }
                            }
                        }
                    }
                    "exit" => {
                        if in_menu {
                            println!("you are not in a game");
                        } else {
                            println!("Exiting...");
                            config.lock().unwrap().state = GameState::Menu;
                        }
                        None
                    },
                    "surrender" => {
                        if in_menu {
                            println!("you are not in a game");
                            None
                        } else {
                            println!("Exiting...");
                            config.lock().unwrap().state = GameState::Menu;
                            Some(Command::Surrender)
                        }
                    },
                    s => {
                        if s.len() > 0 {
                            println!("Invalid command, type 'help' to see available commands");
                        }
                        None
                    },
                };

                if let Some(command) = command {
                    let mut lock = connection.lock().unwrap();
                    let stream = lock.as_mut().expect("No connection").writer();
                    Self::send(stream, command)?;
                };
            }
        }
    }

    fn handle_command(&mut self, command: Command) -> AppResult<()> {
        match command {
            Command::SetId(id) => {
                println!("Your id is: {:?}", id);
                let mut config = self.config.lock().unwrap();
                config.id = Some(id);
                config.state = GameState::Menu;
                println!("Type 'help' to see available commands");
            },
            Command::PasswordRequest => {
                let password = Self::prompt("Enter password")?;
                let mut lock = self.connection.lock().unwrap();
                let stream = lock.as_mut().expect("No connection").writer();
                Self::send(stream, Command::Password(password))?;
            },
            Command::Opponents(opponents) => {
                println!("Opponents: {:?}", opponents);
            },
            Command::Hint(hint) => {
                println!("Hint: {}", hint);
            },
            Command::Guess(guess) => {
                let config = self.config.lock().unwrap();
                println!("Player {} guessed: {}", config.opponent_id.as_ref().expect("No opponent"), guess);
            },
            Command::PlayerJoined(id) => {
                println!("Player {} joined", id);
                let mut config = self.config.lock().unwrap();
                config.opponent_id = Some(id);
            },
            Command::SetGuess(guess) => {
                println!("Guess a word: {}\nPress ENTER to start", guess);
                let mut config = self.config.lock().unwrap();
                config.state = GameState::Guessing;
            }
            Command::Win => {
                let mut config = self.config.lock().unwrap();
                config.opponent_id = None;
                if config.state == GameState::Guessing {
                    println!("You win!");
                } else {
                    println!("Player guessed the word!");
                }
                config.state = GameState::Menu;
            },
            Command::Error(message) => {
                let mut config = self.config.lock().unwrap();
                config.opponent_id = None;
                config.state = GameState::Menu;
                eprintln!("Error: {}", message);
            },
            Command::Unknown(message) => {
                eprintln!("Unknown command: {}", message);
            },
            Command::RequestMatchGuess => {
                println!("Starting new match...");
                let mut guess = Self::prompt("Set a word to guess")?;
                while guess.is_empty() {
                    println!("Word cannot be ampty");
                    guess = Self::prompt("Set a word to guess")?;
                }
                let mut lock = self.connection.lock().unwrap();
                let stream = lock.as_mut().expect("No connection").writer();
                Self::send(stream, Command::SetGuess(guess))?;
            },
            Command::PlayerLeft => {
                let mut config = self.config.lock().unwrap();
                config.opponent_id = None;
                if config.state == GameState::Guessing {
                    println!("Player left, match ended");
                }else {
                    println!("Player surrendered, you win!");
                }
                config.state = GameState::Menu;
            }

            _ => println!("Received unhandled command: {:?}", command),
        };

        Ok(())
    }

    /// Receive a command from the server in a blocking manner.
    fn receive(stream: &mut dyn Read) -> AppResult<Command> {
        let mut buf = [0; 2];
        stream.read_exact(&mut buf)?;

        let size = u16::read(&mut buf.iter()).unwrap() as usize;
        
        if size > 4096 {
            return Err(AppError::TooLarge);
        }

        let mut buf = vec![0; size];
        stream.read_exact(&mut buf)?;
        
        let command = match Command::read(&mut buf.iter()) {
            Some(command) => command,
            None => {
                eprintln!("Failed to parse command from {:?}", &buf);
                return Err(AppError::InvalidCommand)
            }
        };

        Ok(command)
    }

    /// Send a command to the server.
    fn send(stream: &mut dyn Write, command: Command) -> AppResult<()> {
        let packet = Packet::new(command);
        packet.write(stream)?;
        Ok(())
    }
}
