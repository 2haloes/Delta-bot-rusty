use std::env;

use poise::serenity_prelude as serenity;
use ::serenity::all::{ChannelId, UserId};
use serenity::all::{Http, Message};

use crate::{Data, Error};

/*
    This function handles errors from Results<> and Nones from Option<>
    Either using the error message or a custom string that is sent instead of the expected reply
*/
pub async fn return_error_reply<T> (msg: Message, error_msg : String) -> Option<T> {
    let current_http = Http::new(&env::var("DISCORD_TOKEN")
    .expect("Expected a token in the environment - ERROR HANDLER"));    
    // Not using the return_error function as it leads here and if there's an issue here, it'll just loop
    msg.reply(current_http, format!("Apologies, your request cannot be completed, the error is as follows:\n```{}```", error_msg))
    .await
    .expect("Error showing an error - ERROR HANDLER");

    panic!("{}", format!("An error has occured: {}", error_msg))
}

/*
    This function handles errors from Results<> and Nones from Option<>
    Either using the error message or a custom string that is sent instead of the expected reply
*/
pub async fn return_error<T> (user_id: UserId, message_channel_id: ChannelId, error_msg : String) -> Option<T> {
    let current_http = Http::new(&env::var("DISCORD_TOKEN")
    .expect("Expected a token in the environment - ERROR HANDLER"));    
    // Not using the return_error function as it leads here and if there's an issue here, it'll just loop
    message_channel_id.say(current_http, format!("Apologies <@{}>, your request cannot be completed, the error is as follows:\n```{}```", user_id, error_msg))
    .await
    .expect("Error showing an error - ERROR HANDLER");

    panic!("{}", format!("An error has occured: {}", error_msg))
}

pub async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    // This is our custom error handler
    // They are many errors that can occur, so we only handle the ones we want to customize
    // and forward the rest to the default handler
    match error {
        poise::FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {:?}", error),
        poise::FrameworkError::Command { error, ctx, .. } => {
            println!("Error in command `{}`: {:?}", ctx.command().name, error,);
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                println!("Error while handling error: {}", e)
            }
        }
    }
}