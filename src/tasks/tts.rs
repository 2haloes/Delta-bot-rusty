use async_openai::{types::{CreateSpeechRequestArgs, SpeechModel, Voice}, Client};
use poise::CreateReply;
use serenity::all::CreateAttachment;

use crate::{tasks::{ffmpeg_handler::run_ffmpeg, handle_errors::return_error}, Error};

#[poise::command(prefix_command)]
pub async fn tts(
    ctx: crate::Context<'_>,
    #[description = "Text to convert to speech"] 
    tts_string: String
) -> Result<(), Error> {
    let client = Client::new();

    let request = CreateSpeechRequestArgs::default()
        .input(tts_string)
        .voice(Voice::Alloy)
        .model(SpeechModel::Tts1)
        .build()?;

    let response = client.audio().speech(request).await?;

    let attachment: Vec<u8> = response.bytes.to_vec();

    let attachment_processed: Vec<u8>= run_ffmpeg(attachment, "-f matroska -filter_complex \"[0:a]showspectrum=s=hd720:mode=combined:color=intensity:slide=1:scale=cbrt[viz];[viz]format=rgba,geq='p(mod((2*W/(2*PI))*(PI+atan2(0.5*H-Y,X-W/2)),W),H-2*hypot(0.5*H-Y,X-W/2))'\" -c:a mp3".to_string()).await;

    let message_builder = CreateReply 
    { 
        attachments: vec![CreateAttachment::bytes(attachment_processed, "tts_output.mp4")],
        ..Default::default()
    };

    ctx.send(message_builder).await?;

    Ok(())
}