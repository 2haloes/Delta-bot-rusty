use std::{io::Write, path::PathBuf, process::{Command, Stdio}, u8};
use which::which;
use shell_words::split;

pub async fn run_ffmpeg(file_input: Vec<u8>, command: String) -> Vec<u8> {

    let ffmpeg_location = which("ffmpeg").unwrap_or(PathBuf::default());

    let ffmpeg_input_args: Vec<String> = split(&command).expect("Woopsie");

    let mut ffmpeg_full_args: Vec<String> = Vec::new();

    // This adds in the default args, leaving only the FFmpeg args to be passed to the function
    ffmpeg_full_args.push("/C".to_owned());
    ffmpeg_full_args.push(ffmpeg_location.to_str().expect("Test").to_string());
    ffmpeg_full_args.push("-y".to_owned());
    ffmpeg_full_args.push("-i".to_owned());
    ffmpeg_full_args.push("pipe:0".to_owned());
    ffmpeg_full_args.extend(ffmpeg_input_args);
    ffmpeg_full_args.push("pipe:1".to_owned());

    if ffmpeg_location != PathBuf::default() {
        let mut ffmpeg_run = Command::new("cmd")
            .args(ffmpeg_full_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("FFmpeg couldn't start!");

        let mut ffmpeg_stdin = ffmpeg_run.stdin.take().expect("Unable to take FFmpeg stdin!");
        ffmpeg_stdin.write_all(&file_input).expect("Unable to write file to FFmpeg stdin!");
        drop(ffmpeg_stdin);

        let ffmpeg_output = ffmpeg_run.wait_with_output().expect("Unable to get data from FFmpeg out");
        return ffmpeg_output.stdout;
    }

   return Vec::new(); 
}