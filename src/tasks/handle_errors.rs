use std::env;

use poise::serenity_prelude as serenity;
use ::serenity::all::{ChannelId, UserId};
use serenity::all::{Http, Message};

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