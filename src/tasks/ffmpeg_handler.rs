use std::{env, io::Write, path::PathBuf, process::{Command, Stdio}, u8};
use serenity::all::{ChannelId, UserId};
use which::which;
use shell_words::split;

use super::handle_errors::return_error;

pub async fn run_ffmpeg(file_input: Option<Vec<u8>>, url_input: Option<String>, command: String, user_id: UserId, message_channel_id: ChannelId) -> Vec<u8> {

    if file_input.is_none() && url_input.is_none() {
        let _: u8 = return_error(user_id, message_channel_id, "No file or URL has been provided for FFmpeg to ".to_owned()).await.unwrap();
    }

    let ffmpeg_location = which("ffmpeg").unwrap_or(PathBuf::default());
    let ffmpeg_input_args: Vec<String> = split(&command).expect("Woopsie");
    let mut ffmpeg_full_args: Vec<String> = Vec::new();
    let debug_enabled: String = env::var("DEBUG").unwrap_or("0".to_owned());

    // This adds in the default args, leaving only the FFmpeg args to be passed to the function
    if debug_enabled != "1" {
        ffmpeg_full_args.push("-hide_banner".to_owned());
        ffmpeg_full_args.push("-loglevel".to_owned());
        ffmpeg_full_args.push("panic".to_owned());
    }
    ffmpeg_full_args.push("-i".to_owned());
    if url_input.is_none() {
        ffmpeg_full_args.push("pipe:0".to_owned());
    } else {
        // Using unwrap as the value cannot be None
        ffmpeg_full_args.push(format!(r#"{}"#, url_input.clone().unwrap()));
    }
    
    ffmpeg_full_args.extend(ffmpeg_input_args);
    ffmpeg_full_args.push("pipe:1".to_owned());

    if ffmpeg_location != PathBuf::default() {
        let mut ffmpeg_run = match Command::new("ffmpeg")
            .args(ffmpeg_full_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
                {
                    Ok(t) => t,
                    Err(e) => return_error(user_id, message_channel_id, e.to_string()).await.unwrap(),
                };
        if url_input.is_none() {
            let mut ffmpeg_stdin = match ffmpeg_run.stdin.take()
                {
                    Some(t) => t,
                    None => return_error(user_id, message_channel_id, "Unable to take control of the FFmpeg stdin".to_owned()).await.unwrap(),
                };
            // Using unwrap as the value cannot be None
            match ffmpeg_stdin.write_all(&file_input.unwrap())
                {
                    Ok(t) => t,
                    Err(e) => return_error(user_id, message_channel_id, e.to_string()).await.unwrap(),
                };
            drop(ffmpeg_stdin);
        }

        let ffmpeg_output = match ffmpeg_run.wait_with_output()
            {
                Ok(t) => t,
                Err(e) => return_error(user_id, message_channel_id, e.to_string()).await.unwrap(),
            };
        return ffmpeg_output.stdout;
    }

   return Vec::new(); 
}