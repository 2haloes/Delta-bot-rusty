use crate::Error;

#[poise::command(prefix_command, slash_command)]
pub async fn help(ctx: crate::Context<'_>) -> Result<(), Error> {
    let channel_id = ctx.channel_id();
    let _ = channel_id.say(ctx, "Hello! The following prefixes can be used to call me:\n- delta\n- !delta\n\nSlash commands are also supported!\n\nAvaliable commands:\n- imagegen - Generate an image using machine learning, OpenAI DALL-E 3 and Stable diffusion supported\n\nSpecial commands:\n- Mention/Message me - I can respond to requests and chat with you, I can even see images if they have been attached to your messages (I cannot see my own)! Currently only supports OpenAI GPT4").await;
    Ok(())
}