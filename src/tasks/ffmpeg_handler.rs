use std::{path::PathBuf, process::{Command, Stdio}, u8};
use which::which;

async fn run_ffmpeg(file_input: Vec<u8>, command: String) -> Vec<u8> {

    let ffmpeg_location = which("ffmpeg").unwrap_or(PathBuf::default());

    if ffmpeg_location != PathBuf::default() {
        let ffmpeg_run = Command::new("cmd")
            .args([
                "/C",
                &(ffmpeg_location.to_str().expect("Test")),
                &command
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
    }

   return Vec::new(); 
}