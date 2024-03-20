use async_openai::{types::{CreateSpeechRequestArgs, SpeechModel, Voice}, Client};
use poise::CreateReply;
use serenity::all::{CreateAllowedMentions, CreateAttachment};

use crate::{tasks::{ffmpeg_handler::run_ffmpeg, handle_errors::return_error}, Error};

#[poise::command(prefix_command, slash_command)]
pub async fn tts(
    ctx: crate::Context<'_>,
    #[description = "Text to convert to speech"] 
    tts_string: String
) -> Result<(), Error> {
    let client = Client::new();
    let requester_id = ctx.author().id;
    let channel_id = ctx.channel_id();

    let request = CreateSpeechRequestArgs::default()
        .input(tts_string.clone())
        .voice(Voice::Nova)
        .model(SpeechModel::Tts1)
        .build()?;

    let response = match client.audio().speech(request).await
        {
            Ok(t) => t,
            Err(e) => return_error(requester_id, channel_id, e.to_string()).await.unwrap(),
        };

    let attachment: Vec<u8> = response.bytes.to_vec();

    let attachment_processed: Vec<u8>= run_ffmpeg(attachment, "-f matroska -filter_complex \"[0:a]showwaves=s=320x240:colors=White:mode=line'\" -c:a mp3".to_string(), requester_id, channel_id).await;

    let message_builder = CreateReply 
    { 
        attachments: vec![CreateAttachment::bytes(attachment_processed, "tts_output.mp4")],
        allowed_mentions: Some(CreateAllowedMentions::new().replied_user(true)),
        content: format!("<@{}>\nRequested text: {}", requester_id, tts_string).into(),
        ..Default::default()
    };

    match ctx.send(message_builder).await
        {
            Ok(t) => t,
            Err(e) => return_error(requester_id, channel_id, e.to_string()).await.unwrap(),
        };

    Ok(())
}