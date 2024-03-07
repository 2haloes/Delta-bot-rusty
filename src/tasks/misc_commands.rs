use std::fs;

use crate::{tasks::handle_errors::return_error, Error};

#[poise::command(prefix_command, slash_command)]
pub async fn help(ctx: crate::Context<'_>) -> Result<(), Error> {
    let requester_id = ctx.author().id;
    let channel_id = ctx.channel_id();
    let help_full_markdown = match fs::read_to_string("assets/help.md")
        {
            Ok(t) => t,
            Err(e) => return_error(requester_id.clone(), channel_id.clone(), e.to_string()).await.unwrap(),
        };
    let _ = channel_id.say(ctx, format!("DEBUG: {}", help_full_markdown)).await;
    Ok(())
}