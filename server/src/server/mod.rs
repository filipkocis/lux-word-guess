mod clients;
mod listeners;

use std::{collections::HashMap, fs, io::{stdin, BufRead, Read}, sync::{Arc, Mutex, RwLock}};

use serde::Serialize;

use crate::{AppError, AppResult, Command, Connection, Packet, ReadBytes, WriteBytes};

type AMPlayer = Arc<RwLock<Player>>;
type ARWServerState = Arc<RwLock<ServerState>>;

pub struct Player {
    id: u32,
    connection: Connection,
    in_game: Option<u32>,
    authenticated: bool,
}

impl Player {
    pub fn new(id: u32, connection: Connection) -> Self {
        Self {
            id,
            connection,
            in_game: None,
            authenticated: false,
        }
    }
}

#[derive(Serialize, Debug)]
pub struct Game {
    id: u32,
    hinter: u32,
    guesser: u32,
    word: Option<String>,
    guesses: Vec<String>,
    hints: Vec<String>,
    winner: Option<u32>,
    finished: bool,
    timestamp: u64,
}

impl Game {
    fn new(id: u32, hinter: u32, guesser: u32) -> Self {
        Self {
            id,
            hinter,
            guesser,
            word: None,
            guesses: Vec::new(),
            hints: Vec::new(),
            winner: None,
            finished: false,
            timestamp: 0,
        }
    }
}

pub struct ServerState {
    players: HashMap<u32, Arc<RwLock<Player>>>,
    subscribers: Arc<Mutex<HashMap<u32, Option<Connection>>>>,
    games: HashMap<u32, Arc<RwLock<Game>>>,
    next_player_id: u32,
    next_game_id: u32,
}

impl ServerState {
    fn new() -> Self {
        Self {
            players: HashMap::new(),
            subscribers: Arc::new(Mutex::new(HashMap::new())),
            games: HashMap::new(),
            next_player_id: 1,
            next_game_id: 1,
        }
    }

    fn next_player_id(&mut self) -> u32 {
        let id = self.next_player_id;
        self.next_player_id += 1;
        id
    }

    fn next_game_id(&mut self) -> u32 {
        let id = self.next_game_id;
        self.next_game_id += 1;
        id
    }

    fn create_player(&mut self, connection: Connection) -> Arc<RwLock<Player>> {
        let id = self.next_player_id();
        let player = Player::new(id, connection);
        let player = Arc::new(RwLock::new(player));
        self.players.insert(id, player.clone());
        player
    }

    fn create_game(&mut self, hinter: u32, guesser: u32) -> Arc<RwLock<Game>> {
        let id = self.next_game_id();
        let game = Game::new(id, hinter, guesser);
        let game = Arc::new(RwLock::new(game));
        self.games.insert(id, game.clone());
        game
    }
}

pub struct ServerConfig {
    password: String,
}

impl ServerConfig {
    fn new(password: String) -> Self {
        Self {
            password,
        }
    }
}

pub struct Server {
    state: Arc<RwLock<ServerState>>,
    config: Arc<ServerConfig>,
}

impl Server {
    const SOCKET_PATH: &'static str = "/tmp/game-guess-a-word-socket";

    pub fn new(password: String) -> Self {
        Self {
            state: Arc::new(RwLock::new(ServerState::new())),
            config: Arc::new(ServerConfig::new(password)),
        }
    }

    pub fn run(&mut self) -> AppResult<()> {
        println!("Server started with password: {:?}", self.config.password);
        self.start_unix_listener()?; 
        self.start_tcp_listener()?; 

        for line in stdin().lock().lines() {
            let line = line?;

            if line.trim() == "exit" {
                break;
            } else {
                println!("Unknown command: {:?}", line);
                println!("Type 'exit' to quit");
            }
        }

        Ok(())
    }

    /// Unlink the socket file if it exists.
    pub fn cleanup_socket() {
        if fs::remove_file(Self::SOCKET_PATH).is_ok() {
            println!("Removed existing socket file");
        }
    }

    pub fn receive(stream: &mut dyn Read) -> AppResult<Command> {
        let mut buf = [0; 2];
        stream.read_exact(&mut buf)?;

        let size = u16::read(&mut buf.iter()).ok_or(AppError::InvalidCommand)? as usize;

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

    pub fn send(connection: &mut Connection, command: Command) -> AppResult<()> {
        let stream = connection.writer();

        let packet = Packet::new(command);
        packet.write(stream)?;
        Ok(())
    }
}
