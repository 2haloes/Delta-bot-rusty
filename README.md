# Delta Bot Rusty

## What is this?
This is a Discord bot that uses generative AI to reply to users and requests.

It is a continuation of the [Delta bot project](https://github.com/2haloes/Delta-Discord-Bot), while the other project worked well enough, I didn't feel like Python and the libraries allowed me to bring out what I wanted from the bot so I changed gears to Rust.

The current state of Delta bot rusty is still in a pre-release state with complete feature parity with the orginal Delta Bot project, features are actively being developed at this time.

## Features

- Text replies using AI
  - Using GPT4-turbo
  - Text can split between messages, this attempts to account for formatting but may fail
- Generate images using AI
  - Using DALL-E 3
  - Using Runpod serverless with a modified version of the Stable Diffusion XL spec
    - Details of the serverless setup can be [found here](https://github.com/2haloes/worker-sdxl-pony-v8), note that the refiner code has been removed
- Use vision to look at images
  - Using GPT4V
- Load text based file attachments
  - Works with (hopefully) every plain text format including source code, scripts, plain text and markdown
  - Attachments are labeled as such and sent in plain text as part of the message to GPT4
  - This can be used to get around Discord's character limit which is an intended use case due to trying to paste too much text into Discord renders it as a text attachment
- Multi threaded
  - Each reply is run on a different thread, allowing the bot to be in multple places at once
- Typing indicator support
  - The bot will start typing when processing and stop when done
- Error surfacing
  - Lets the end user know if an exception has occured with a basic description of the exception
  - This includes when content has been blocked by OpenAI filters
- Customisable functionality
  - The `assets/functions.json` file allows implimenting different models. Currently it's only used for image generation
  - Image models can be linked to different endpoints or the same endpoint with modified prompts
    - This can be used with my [modified SDXL endpoint](https://github.com/2haloes/worker-sdxl-pony-v8) as the model used in the repo supports 4 different output styles

## Roadmap

- [x] Setup working serverless solutions for generating images outside of DALL-E
  - [x] Currently working on the following image generation models for generation
    - [x] [Juggernaut XL](<https://civitai.com/models/133005/juggernaut-xl>) for more realistic general generations
    - [x] [Pony Diffusion XL](<https://civitai.com/models/257749/pony-diffusion-v6-xl>) for more stylised generations
- [x] Rewrite image handling
  - Currently the image handling sucks, it saves and then deletes the image file so it can be copied into the message
  - [x] This new way will be entirely in memory, nothing is saved to disk
- [ ] Public release of Runpod serverless templates
  - Will allow anyone to quickly setup Runpod services using the same models that I have been using
  - Not currently possible but the docker containers to use on Runpod are avaliable
    - [Juggernaut XL Container](https://hub.docker.com/repository/docker/2haloes/runpod-sdxl-juggernaut)
    - [Pony Diffusion XL Container](https://hub.docker.com/repository/docker/2haloes/runpod-sdxl-pony)
    - [Github repo these both are based on](https://github.com/2haloes/worker-sdxl-pony-v8)
- [x] Proper function integration
  - This is a big unknown for me but I'm looking at it, basically when you type ! in discord, it should then show Delta's commands
    - I got it close enough using slash commands, I had no idea what was going to happen and didn't know what was possible
- [ ] Voice support (both ways)
  - [x] For tts, this would use OpenAI's built in TTS support (if it works in the libraries I'm using of course), you'd probably start a message with something like !delta-tts and then Delta will reply with both text and a audio file
    - [x] Will need to convert to a video, Discord sucks horribly for audio formats
  - [ ] For voice recognition, this would use Whisper, it's just the best, only speech to text will work but I believe that's all that's currently around
    - [ ] Delta would skip it's own messages as it always provides a transcription of it's own message anyway
- [ ] Possibly reimpliment OpenAI dependent functionality to allow use of Runpod Serverless
  - Will require menually calling endpoints as opposed to OpenAI which is using a library
  - [ ] Text generation (Will need to investigate what model to use, do not want to use one too big due to the cost)
  - [ ] Vision (Currently LLaVA 1.6 34B is looking like the best option)
  - [ ] TTS (There's a lot around, will look into what I want to do with this)
  - [ ] STT (Whisper, it's been freely released so there's no reason not to use it)

## Building and Running

### Prerequisites

- Open AI API key (needed for everything outside of RunPod image generation currently)
- Runpod API key (needed for Runpod image generation)
- A Discord bot token (required for interating with Discord)
- (Optional) Your own Discord user ID (Only used if you are using this in debug mode)
- The latest stable version of rust installed
- Git installed

### Building

- Clone the repo into whatever folder you like `git clone https://github.com/2haloes/Delta-bot-rusty.git`
- Enter the repo folder `cd Delta-bot-rusty`
- Run a build `cargo build --release` (remove `--release` if you want to create a debug build)

### Running
- After building above, enter the build folder `cd target/release` (use `cd target/debug` if you created a debug build)
- Run the program with environment varaibles, fill out the values in the commands
  - Environment variable names
    - (Optional) DEBUG - If set to 1 then only replies to the user specified in USER_ID and will prepend all messages with "Debug: "
    - (Optional) USER_ID - Only used if DEBUG is set to 1, is the ID of the testing user
    - DISCORD_TOKEN - The Discord token used for the bot
    - OPENAI_API_KEY - The OpenAI API key used to call OpenAI services
    - RUNPOD_API_KEY - The RunPod API Key used to call serverless services
    - SYSTEM_DETAILS - The system message used for text generation, this details the personality and style that you would like the bot to have
  - Windows: `cmd.exe /c "set DISCORD_TOKEN= && set OPENAI_API_KEY= && set RUNPOD_API_KEY= && set SYSTEM_DETAILS= && ./delta-bot-rusty.exe"`
  - Linux/WSL: `DISCORD_TOKEN="" OPENAI_API_KEY="" RUNPOD_API_KEY="" SYSTEM_DETAILS="" ./delta-bot-rusty`

## functions.json

The functions.json file needs to be located in assets folder which is in the same folder as delta-bot-rusty(.exe)

It has the following JSON layout

```
{
    "function_data": [
        {
            "function_command": "!delta-dalle",
            "function_type": "openai_dalle",
            "function_api_key": "",
            "function_friendly_name": "DALL-E 3",
            "prompt_prefix": "",
            "prompt_suffix": ""
        },
        {
            "function_command": "!delta-imagegen",
            "function_type": "runpod_image",
            "function_api_key": "sd-openjourney",
            "function_friendly_name": "OpenJourney SD 1.5"
            "prompt_prefix": "",
            "prompt_suffix": ""
        }
    ]
}
```

Breaking down each of the fields, the options are simple
- function_command - Must start with "!delta", this is the command that the user must start the message with to run the command
- function_type - What functionality is being used with this command, the avaliable types are as following
  - openai_dalle - Uses OpenAI's DALL-E for image generation
  - runpod_image - Uses Runpod serverless for image generation
- function_api_key - This is currenly used to point to Runpod serverless endpoints, put the serverless endpoint ID here, unused with openai_dalle functions
- prompt_prefix - This is put before the prompt, I use this for the putting in the score part of a Pony Diffusion prompt
- prompt_suffix - This is put after the prompt, I use this for putting the style for a Pony Diffusion prompt
