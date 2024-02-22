use std::env;

use serenity::all::{Http, Message};

pub trait UnwrapOrReturnError<T, E> {
    async fn unwrap_or_return_error(self, msg: Message, override_error: Option<String>) -> T;
}

impl<T> UnwrapOrReturnError<T, String> for Option<T> {
    async fn unwrap_or_return_error(self, msg: Message, override_error: Option<String>) -> T {
        match self {
            Some(t) => t,
            None => return_error(msg.clone(), override_error.unwrap_or("An error has occured unwrapping an Option".to_owned())).await.unwrap(),
        }
    }
}

impl<T, E: std::fmt::Display> UnwrapOrReturnError<T, E> for Result<T, E> {
    async fn unwrap_or_return_error(self, msg: Message, override_error: Option<String>) -> T {
        match self {
            Ok(t) => t,
            Err(e) => return_error(msg.clone(), override_error.unwrap_or(e.to_string())).await.unwrap(),
        }
    }
}

pub async fn return_error<T> (msg: Message, error_msg : String) -> Option<T> {
    let current_http = Http::new(&env::var("DISCORD_TOKEN")
    .expect("Expected a token in the environment - ERROR HANDLER"));    
    // Not using the return_error function as it leads here and if there's an issue here, it'll just loop
    msg.reply(current_http, format!("Apologies, your request cannot be completed, the error is as follows:\n```{}```", error_msg))
    .await
    .expect("Error showing an error - ERROR HANDLER");

    panic!("{}", format!("An error has occured: {}", error_msg))
}