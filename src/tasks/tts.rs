use async_openai::{types::{CreateSpeechRequestArgs, SpeechModel, Voice}, Client};
use poise::CreateReply;
use serenity::all::CreateAttachment;
use tokio::time::timeout;
use std::{env, fs::{create_dir_all, remove_file}, time::Duration};

use crate::{tasks::{ffmpeg_handler::run_ffmpeg, handle_errors::{return_error, return_error_command}}, Error};

#[poise::command(slash_command)]
pub async fn tts_from_text(
    ctx: crate::Context<'_>,
    #[description = "Text to convert to speech"] 
    text_to_tts: String
) -> Result<(), Error> {
    // NOTE: This command has a timeout of 3 minutes, this is due to OpenAI sometimes taking an extremely long time to process longer text requests and at some point it does have to stop
    let _result = match timeout(Duration::from_secs(180), tts_run(ctx, text_to_tts)).await
        {
            Ok(t) => t,
            Err(_) => return_error_command(ctx, "This TTS command has timed out, this may be due to the length of the text".to_owned()).await.unwrap(),
        };

    Ok(())
}

#[poise::command(slash_command)]
pub async fn tts_from_message(
    ctx: crate::Context<'_>,
    #[description = "Link to the message to convert to speech"] 
    message_to_tts: serenity::all::Message
) -> Result<(), Error> {
    // NOTE: This command has a timeout of 3 minutes, this is due to OpenAI sometimes taking an extremely long time to process longer text requests and at some point it does have to stop
    let _result = match timeout(Duration::from_secs(180), tts_run(ctx, message_to_tts.content_safe(ctx))).await
        {
            Ok(t) => t,
            Err(_) => return_error_command(ctx, "This TTS command has timed out, this may be due to the length of the text".to_owned()).await.unwrap(),
        };

    Ok(())
}

pub async fn tts_run (
    ctx: crate::Context<'_>,
    tts_string: String
)
{
    let client = Client::new();
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
    let tmp_location = current_path.join("tmp");

    let _ = create_dir_all(tmp_location.clone());

    let tmp_file = tmp_location.join(format!("{}_{}.mp3", requester_id, channel_id));

    match ctx.defer().await
    {
        Ok(t) => t,
        Err(e) => return_error(requester_id, channel_id, e.to_string()).await.unwrap(),
    };

    let request = match CreateSpeechRequestArgs::default()
    .input(tts_string.clone())
    .voice(Voice::Nova)
    .model(SpeechModel::Tts1Hd)
    .build()
        {
            Ok(t) => t,
            Err(e) => return_error(requester_id, channel_id, e.to_string()).await.unwrap(),
        };

    let response = match client.audio().speech(request).await
        {
            Ok(t) => t,
            Err(e) => return_error(requester_id, channel_id, e.to_string()).await.unwrap(),
        };

    // let attachment: Vec<u8> = response.bytes.to_vec();
    let _ = response.save(tmp_file.clone()).await;

    let attachment_processed: Vec<u8>= run_ffmpeg(None, Some(tmp_file.clone().into_os_string().into_string().unwrap()), "-f matroska -filter_complex \"[0:a]showwaves=s=320x240:colors=White:mode=line'\" -c:a mp3".to_string(), requester_id, channel_id).await;

    let _ = remove_file(tmp_file);
    if attachment_processed.is_empty() {
        let _: Error = return_error(requester_id, channel_id, "TTS output has returned empty".to_owned()).await.unwrap();
    }

    let message_builder = CreateReply 
    { 
        attachments: vec![CreateAttachment::bytes(attachment_processed, "tts_output.mp4")],
        content: format!("<@{}>\nRequested text: {}", requester_id, tts_string).into(),
        ..Default::default()
    };

    match ctx.send(message_builder).await
        {
            Ok(t) => t,
            Err(e) => return_error(requester_id, channel_id, e.to_string()).await.unwrap(),
        };
}