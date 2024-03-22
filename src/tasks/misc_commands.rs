use std::{env, fs};

use crate::{tasks::handle_errors::return_error, Error};

#[poise::command(prefix_command, slash_command)]
pub async fn help(ctx: crate::Context<'_>) -> Result<(), Error> {
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
    let assets_location = current_path.join("assets").join("help.md");
    let help_full_markdown = match fs::read_to_string(assets_location)
        {
            Ok(t) => t,
            Err(e) => return_error(requester_id.clone(), channel_id.clone(), e.to_string()).await.unwrap(),
        };
    let _ = channel_id.say(ctx, format!("DEBUG: {}", help_full_markdown)).await;
    Ok(())
}