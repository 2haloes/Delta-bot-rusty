use async_openai::{types::{CreateSpeechRequestArgs, SpeechModel, Voice}, Client};
use poise::CreateReply;
use serenity::all::CreateAttachment;

use crate::{tasks::handle_errors::return_error, Error};

#[poise::command(prefix_command)]
pub async fn tts(ctx: crate::Context<'_>) -> Result<(), Error> {
    let client = Client::new();

    let request = CreateSpeechRequestArgs::default()
        .input("Today is a wonderful day to build something people love!")
        .voice(Voice::Alloy)
        .model(SpeechModel::Tts1)
        .build()?;

    let response = client.audio().speech(request).await?;

    let attachment: Vec<u8> = response.bytes.to_vec();

    let message_builder = CreateReply 
    { 
        attachments: vec![CreateAttachment::bytes(attachment, "tts_output.ogg")],
        ..Default::default()
    };

    ctx.send(message_builder).await?;

    Ok(())
}