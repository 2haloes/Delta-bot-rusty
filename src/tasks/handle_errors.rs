use std::env;

use serenity::all::{Http, Message};

pub async fn return_error<T> (msg: Message, error_msg : String) -> Option<T> {
    let current_http = Http::new(&env::var("DISCORD_TOKEN")
    .expect("Expected a token in the environment - ERROR HANDLER"));    
    // Not using the return_error function as it leads here and if there's an issue here, it'll just loop
    msg.reply(current_http, format!("Apologies, your request cannot be completed, the error is as follows:\n```{}```", error_msg))
    .await
    .expect("Error showing an error - ERROR HANDLER");

    panic!("{}", format!("An error has occured: {}", error_msg))
}