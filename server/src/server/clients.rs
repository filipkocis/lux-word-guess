use std::{sync::{Arc, RwLock}, time::{SystemTime, UNIX_EPOCH}};

use crate::{AppError, AppResult, Command, Connection};

use super::{AMPlayer, ARWServerState, Game, Player, Server, ServerConfig, ServerState};

fn get_timestamp() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("time travel").as_secs()
}

impl Server {
    fn broadcast_games(state: &ServerState, game: &Game) -> AppResult<()> {
        if !game.finished || game.word.is_none() {
            return Ok(())
        }

        let mut subs = state.subscribers.lock().unwrap();
        
        for (_, conns) in subs.iter_mut() {
            if let Some(conn) = conns {
                let json = serde_json::to_string(game).map_err(|e| AppError::Serde(e))?;
                if let Err(_) = Self::send(conn, Command::SubscribeToGames(json)) {
                    *conns = None;
                };
            }
        }

        Ok(())
    }

    fn broadcast_games_single(connection: &mut Connection, game: &Game) -> AppResult<()> {
        if !game.finished || game.word.is_none() {
            return Ok(())
        }
        let json = serde_json::to_string(game).map_err(|e| AppError::Serde(e))?;
        Self::send(connection, Command::SubscribeToGames(json))
    }

    fn disconnect(player: AMPlayer, state: ARWServerState) -> AppResult<()> {
        let in_game = player.read().unwrap().in_game;

        let Some(game_id) = in_game else {
            return Ok(())
        };

        let state = state.read().unwrap();
        let Some(game) = state.games.get(&game_id) else {
            return Ok(())
        };

        let player_id = {
            let mut game = game.write().unwrap();
            game.finished = true;
            game.timestamp = get_timestamp();

            let guesser = game.guesser != player.read().unwrap().id;

            let player_id = if guesser {
                game.winner = Some(game.guesser);
                game.guesser
            } else {
                game.winner = Some(game.hinter);
                game.hinter
            };

            Self::broadcast_games(&state, &game)?;

            player_id
        };

        let Some(player) = state.players.get(&player_id) else {
            return Ok(())
        };

        let mut player = player.write().unwrap();
        player.in_game = None;
        Self::send(&mut player.connection, Command::PlayerLeft)?;
        Ok(())
    }

    fn handle_client_auth(player: &AMPlayer) -> AppResult<()> {
        let mut player = player.write().unwrap();
        let connection = &mut player.connection;

        Self::send(connection, Command::PasswordRequest)
    }

    pub fn handle_client(player: AMPlayer, state: ARWServerState, config: Arc<ServerConfig>) -> AppResult<()> { 
        // request password
        Self::handle_client_auth(&player)?;

        // only one reader is ever needed for a connection
        let (id, connection) = {
            let player = player.read().unwrap();
            let conn = player.connection.reader.clone();
            (player.id, conn)
        };
        let mut lock = connection.lock().unwrap();
        let reader = lock.reader();

        // main blocking loop per connection
        loop {
            let command = match Self::receive(reader) {
                Ok(command) => command,
                Err(_) => {
                    Self::disconnect(player, state)?;
                    return Ok(())
                }
            };
            Self::handle_command(command, &player, &state, &config)?;

            if !player.read().unwrap().authenticated {
                break
            }
        };

        drop(lock);
        drop(connection);
        drop(player);

        if state.read().unwrap().subscribers.lock().unwrap().contains_key(&id) {
            let mut state = state.write().unwrap();  
            let Some(player) = state.players.remove(&id) else { return Ok(()) };
            let Ok(player) = Arc::try_unwrap(player) else { return Ok(()) };
            let Ok(mut player) = player.into_inner() else { return Ok(()) };

            println!("Added new subscriber");
            for (_, game) in &state.games {
                Self::broadcast_games_single(&mut player.connection, &game.read().unwrap())?;
            }

            state.subscribers.lock().unwrap().get_mut(&id).unwrap().replace(player.connection);
        }

        Ok(())
    }

    pub fn handle_command(command: Command, player: &AMPlayer, state: &ARWServerState, config: &Arc<ServerConfig>) -> AppResult<()> {
        match command {
            Command::SubscribeToGames(password) => {
                println!("Received password: {:?}", password);

                if password == config.password {
                    let id = player.read().unwrap().id;
                    let state = state.read().unwrap();
                    state.subscribers.lock().unwrap().insert(id, None);
                    return Ok(())
                } else {
                    return Err(AppError::InvalidAuth);
                }
            },
            _ => (),
        };

        if !player.read().unwrap().authenticated {
            if let Command::Password(pass) = command {
                println!("Received password: {:?}", pass);

                if pass == config.password {
                    let mut player = player.write().unwrap();
                    player.authenticated = true;
                    let id = player.id.to_string();
                    return Self::send(&mut player.connection, Command::SetId(id));
                } else {
                    return Err(AppError::InvalidAuth);
                }
            } else {
                return Err(AppError::Unauthorized);
            }
        }

        match command {
            Command::OpponentsRequest => {
                let self_id = player.read().unwrap().id;

                let state = state.read().unwrap();
                let players = state.players.iter().filter_map(|(id, player)| {
                    if *id == self_id {
                        return None
                    }

                    let player = player.read().unwrap();
                    if player.in_game.is_none() && player.authenticated {
                        Some(player.id.to_string())
                    } else {
                        None
                    }
                }).collect::<Vec<_>>();

                let mut player_self = player.write().unwrap();
                Self::send(&mut player_self.connection, Command::Opponents(players))?;
            },
            Command::RequestMatch(id) => {
                let self_id = player.read().unwrap().id;
                let Ok(player_id) = id.parse() else {
                    Self::send(&mut player.write().unwrap().connection, Command::Error("Invalid player id".to_string()))?;
                    return Ok(())
                };

                if self_id == player_id {
                    Self::send(&mut player.write().unwrap().connection, Command::Error("Cannot start a match with yourself".to_string()))?;
                    return Ok(())
                }

                let player_other = {
                    let state = state.read().unwrap();
                    let Some(player_other) = state.players.get(&player_id) else {
                        Self::send(&mut player.write().unwrap().connection, Command::Error("Player id not found".to_string()))?;
                        return Ok(())
                    };
                    player_other.clone()
                };

                // deadlock if multiple players are trying to match with the same player
                let mut player_self = player.write().unwrap();
                let mut player_other = player_other.write().unwrap();

                if player_self.in_game.is_some() || player_other.in_game.is_some() || !player_other.authenticated {
                    Self::send(&mut player_self.connection, Command::Error("Invalid player id".to_string()))?;
                    return Ok(())
                }

                let game = state.write().unwrap().create_game(self_id, player_id);
                let game_id = game.read().unwrap().id;

                player_self.in_game = Some(game_id);
                player_other.in_game = Some(game_id);

                Self::send(&mut player_self.connection, Command::RequestMatchGuess)?;
                Self::send(&mut player_other.connection, Command::PlayerJoined(self_id.to_string()))?;
            },
            Command::SetGuess(guess) => {
                let Some((game, other_player)) = Self::get_game_other_player(&player, &state, true)? else {
                    return Ok(())
                };

                if game.read().unwrap().word.is_some() {
                    Self::send(&mut player.write().unwrap().connection, Command::Error("Word already set".to_string()))?;
                    return Ok(())
                }

                let blank_guess = "_".repeat(guess.len());
                game.write().unwrap().word = Some(guess.clone());

                // deadlock if multiple players are trying to match with the same player
                let mut player_self = player.write().unwrap();
                let mut player_other = other_player.write().unwrap();

                Self::send(&mut player_self.connection, Command::PlayerJoined(player_other.id.to_string()))?;
                Self::send(&mut player_other.connection, Command::SetGuess(blank_guess))?;
            },
            Command::Hint(hint) => {
                let Some((game, other_player)) = Self::get_game_other_player(&player, &state, true)? else {
                    return Ok(())
                };

                {
                    let mut game = game.write().unwrap();
                    if game.word.is_none() {
                        Self::send(&mut player.write().unwrap().connection, Command::Error("Not in a game".to_string()))?;
                        return Ok(())
                    }
                    game.hints.push(hint.clone());
                }

                let mut other_player = other_player.write().unwrap();
                Self::send(&mut other_player.connection, Command::Hint(hint))?;
            },
            Command::Guess(guess) => {
                let Some((game, other_player)) = Self::get_game_other_player(&player, &state, false)? else {
                    return Ok(())
                };

                {
                    let mut game = game.write().unwrap();
                    let Some(word) = game.word.clone() else {
                        Self::send(&mut player.write().unwrap().connection, Command::Error("Not in a game".to_string()))?;
                        return Ok(())
                    };
                    game.guesses.push(guess.clone());

                    if word == guess {
                        game.finished = true;     
                        game.timestamp = get_timestamp();
                        game.winner = Some(player.read().unwrap().id);
                        { 
                            let mut player = player.write().unwrap();
                            player.in_game = None;
                            Self::send(&mut player.connection, Command::Win)?; 
                        }
                        Self::broadcast_games(&state.read().unwrap(), &game)?;
                        let mut other_player = other_player.write().unwrap();
                        other_player.in_game = None;
                        Self::send(&mut other_player.connection, Command::Win)?;
                        return Ok(())
                    } 
                }

                let mut other_player = other_player.write().unwrap();
                Self::send(&mut other_player.connection, Command::Guess(guess))?;
            }

            _ => println!("Received unhandled command: {:?}", command),
        };

        Ok(())
    }

    fn get_game_other_player(player: &AMPlayer, state: &ARWServerState, guesser: bool) -> AppResult<Option<(Arc<RwLock<Game>>, Arc<RwLock<Player>>)>> {
        let in_game = player.read().unwrap().in_game;

        let Some(game_id) = in_game else {
            Self::send(&mut player.write().unwrap().connection, Command::Error("Not in a game".to_string()))?;
            return Ok(None)
        };

        let state = state.read().unwrap();
        let Some(game) = state.games.get(&game_id) else {
            let err = Command::Error("Game not found".to_string());
            Self::send(&mut player.write().unwrap().connection, err)?;
            return Ok(None)
        };

        let player_id = if guesser {
            game.read().unwrap().guesser
        } else {
            game.read().unwrap().hinter
        };
        let Some(player) = state.players.get(&player_id) else {
            let err = Command::Error("Player in game not found".to_string());
            Self::send(&mut player.write().unwrap().connection, err)?;
            return Ok(None)
        };

        Ok(Some((game.clone(), player.clone())))
    }
}
