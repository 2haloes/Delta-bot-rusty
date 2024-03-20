use std::{env, io::Write, path::PathBuf, process::{Command, Stdio}, u8};
use serenity::all::{ChannelId, UserId};
use which::which;
use shell_words::split;

use super::handle_errors::return_error;

pub async fn run_ffmpeg(file_input: Vec<u8>, command: String, user_id: UserId, message_channel_id: ChannelId) -> Vec<u8> {

    let ffmpeg_location = which("ffmpeg").unwrap_or(PathBuf::default());
    let ffmpeg_input_args: Vec<String> = split(&command).expect("Woopsie");
    let mut ffmpeg_full_args: Vec<String> = Vec::new();
    let ffmpeg_location_as_str = match ffmpeg_location.to_str()
        {
            Some(t) => t,
            None => return_error(user_id, message_channel_id, "Unable to convert FFmpeg location to string in execution".to_owned()).await.unwrap(),
        };
    let debug_enabled: String = env::var("DEBUG").unwrap_or("0".to_owned());

    // This adds in the default args, leaving only the FFmpeg args to be passed to the function
    ffmpeg_full_args.push("/C".to_owned());
    ffmpeg_full_args.push(ffmpeg_location_as_str.to_string());
    ffmpeg_full_args.push("-y".to_owned());
    if debug_enabled == "1" {
        ffmpeg_full_args.push("-hide_banner".to_owned());
        ffmpeg_full_args.push("-loglevel".to_owned());
        ffmpeg_full_args.push("panic".to_owned());
    }
    ffmpeg_full_args.push("-i".to_owned());
    ffmpeg_full_args.push("pipe:0".to_owned());
    ffmpeg_full_args.extend(ffmpeg_input_args);
    ffmpeg_full_args.push("pipe:1".to_owned());

    if ffmpeg_location != PathBuf::default() {
        let mut ffmpeg_run = match Command::new("cmd")
            .args(ffmpeg_full_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
                {
                    Ok(t) => t,
                    Err(e) => return_error(user_id, message_channel_id, e.to_string()).await.unwrap(),
                };

        let mut ffmpeg_stdin = match ffmpeg_run.stdin.take()
            {
                Some(t) => t,
                None => return_error(user_id, message_channel_id, "Unable to take control of the FFmpeg stdin".to_owned()).await.unwrap(),
            };
        match ffmpeg_stdin.write_all(&file_input)
            {
                Ok(t) => t,
                Err(e) => return_error(user_id, message_channel_id, e.to_string()).await.unwrap(),
            };
        drop(ffmpeg_stdin);

        let ffmpeg_output = match ffmpeg_run.wait_with_output()
            {
                Ok(t) => t,
                Err(e) => return_error(user_id, message_channel_id, e.to_string()).await.unwrap(),
            };
        return ffmpeg_output.stdout;
    }

   return Vec::new(); 
}