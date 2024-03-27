use async_openai::{types::{AudioInput, CreateTranscriptionRequestArgs}, Client};
use poise::CreateReply;
use serenity::all::Attachment;
use tokio::time::timeout;
use std::time::Duration;

use crate::{tasks::{ffmpeg_handler::run_ffmpeg, handle_errors::{return_error, return_error_command}}, Error};

#[poise::command(slash_command)]
pub async fn transcribe_from_attachment(
    ctx: crate::Context<'_>,
    #[description = "Attachment to convert to text (video and audio supported)"] 
    attachment_to_stt: Attachment
) -> Result<(), Error> {
    // NOTE: This command has a timeout of 3 minutes, this is due to OpenAI sometimes taking an extremely long time to process longer text requests and at some point it does have to stop
    let _result = match timeout(Duration::from_secs(180), stt_run(ctx, attachment_to_stt.proxy_url)).await
        {
            Ok(t) => t,
            Err(_) => return_error_command(ctx, "This TTS command has timed out, this may be due to the length of the text".to_owned()).await.unwrap(),
        };

    Ok(())
}

#[poise::command(slash_command)]
pub async fn transcribe_from_message(
    ctx: crate::Context<'_>,
    #[description = "Link to the message with an attachment to convert to text (video and audio supported)"] 
    message_to_stt: serenity::all::Message
) -> Result<(), Error> {
    if message_to_stt.attachments.is_empty() {
        return_error_command(ctx, "The linked message does not have any attachments".to_owned()).await.unwrap()
    }
    // NOTE: This command has a timeout of 3 minutes, this is due to OpenAI sometimes taking an extremely long time to process longer text requests and at some point it does have to stop
    let _result = match timeout(Duration::from_secs(180), stt_run(ctx, message_to_stt.attachments[0].proxy_url.clone())).await
        {
            Ok(t) => t,
            Err(_) => return_error_command(ctx, "This TTS command has timed out, this may be due to the length of the text".to_owned()).await.unwrap(),
        };

    Ok(())
}

pub async fn stt_run (
    ctx: crate::Context<'_>,
    stt_attachment_url: String
)
{
    let client = Client::new();
    let requester_id = ctx.author().id;
    let channel_id = ctx.channel_id();

    match ctx.defer().await
    {
        Ok(t) => t,
        Err(e) => return_error(requester_id, channel_id, e.to_string()).await.unwrap(),
    };

    let attachment_processed: Vec<u8>= run_ffmpeg(None, Some(stt_attachment_url.clone()), "-f mp3".to_string(), requester_id, channel_id).await;

    if attachment_processed.is_empty() {
        let _: Error = return_error(requester_id, channel_id, "File conversion output has returned empty".to_owned()).await.unwrap();
    }

    let request = match CreateTranscriptionRequestArgs::default()
        .file(AudioInput::from_vec_u8("discord_video.mp3".to_owned(), attachment_processed))
        .model("whisper-1")
        .build()
            {
                Ok(t) => t,
                Err(e) => return_error(requester_id, channel_id, e.to_string()).await.unwrap(),
            };
    
    let response = match client.audio().transcribe(request).await
        {
            Ok(t) => t,
            Err(e) => return_error(requester_id, channel_id, e.to_string()).await.unwrap(),
        };

    let response_text = response.text;

    let message_builder = CreateReply 
    { 
        content: format!("<@{}>\nRequested transcription source: [Here](<{}>)\nTransribed text: {}", requester_id, stt_attachment_url, response_text).into(),
        ..Default::default()
    };

    match ctx.send(message_builder).await
        {
            Ok(t) => t,
            Err(e) => return_error(requester_id, channel_id, e.to_string()).await.unwrap(),
        };
}