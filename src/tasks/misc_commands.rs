use std::{env, fs};

use serenity::all::Message;
use tokio::fs::try_exists;

use crate::{tasks::{handle_errors::return_error, text_generation::text_reply}, Error};

/// Show help message
#[poise::command(prefix_command, slash_command)]
pub async fn help(
    ctx: crate::Context<'_>,
    #[description = "Command to get help for (default shows general help)"]
    command: Option<String>,
) -> Result<(), Error> {
    let requester_id = ctx.author().id;
    let channel_id = ctx.channel_id();
    let current_exe = match env::current_exe()
        {
            Ok(t) => t,
            Err(e) => return_error(requester_id.clone(), channel_id.clone(), e.to_string()).await.unwrap(),
        };
    let current_path = match current_exe.parent() 
        {
            Some(t) => t,
            None => return_error(requester_id.clone(), channel_id.clone(), "Unable to process current function string".to_owned()).await.unwrap(),
        };
    let command_to_help = command.unwrap_or("help".to_owned());
    let help_file_location = current_path.join("assets").join("help").join(format!("{}.md", command_to_help));
    let help_file_exists = match try_exists(help_file_location.clone()).await
        {
            Ok(t) => t,
            Err(e) => return_error(requester_id.clone(), channel_id.clone(), e.to_string()).await.unwrap(),
        };
    if help_file_exists {
        let help_full_markdown = match fs::read_to_string(help_file_location)
        {
            Ok(t) => t,
            Err(e) => return_error(requester_id.clone(), channel_id.clone(), e.to_string()).await.unwrap(),
        };
        let _ = channel_id.say(ctx, format!("{}", help_full_markdown)).await;
    }
    
    Ok(())
}

/// Override the system command
#[poise::command(prefix_command, slash_command)]
pub async fn override_system(
    ctx: crate::Context<'_>,
    #[description = "The system message to use"]
    system_message: String,
    #[description = "The message content"]
    content: Option<String>,
    #[description = "The message URL to reply to"]
    message: Option<Message>,
) -> Result<(), Error> {
    if message.is_some() {
        let message_return: Vec<String> = text_reply(message.unwrap(), ctx.http(), ctx.author().id.into(), Some(system_message)).await;
        
    }
    Ok(())
}