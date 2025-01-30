use std::env::args;

use server_app::*;

fn main() -> AppResult<()> {
    let password = args().nth(1).unwrap_or_else(|| String::from("supersecret123"));    

    Server::new(password).run()    
}
