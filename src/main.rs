use std::{env, path::{PathBuf, Path}, thread::sleep, time::Duration, fs::{File, self}, io::copy};

use serenity::{
    async_trait,
    model::{channel::Message, gateway::Ready},
    prelude::*, client::Cache, http::{CacheHttp, Http, Typing}, builder::CreateMessage,
};

use async_openai::{
    types::{Role, ChatCompletionRequestMessage, CreateChatCompletionRequestArgs, CreateImageRequestArgs, ImageSize, ResponseFormat, ImageModel, ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestFunctionMessageArgs, ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestToolMessageArgs, ChatCompletionRequestUserMessageArgs}, Client,
};
use tokio::task;
use reqwest::header::HeaderValue;
use uuid::Uuid;

#[derive(serde::Deserialize)]
struct FunctionData {
    function_command: String,
    function_type: String,
    function_api_key: String
}

#[derive(serde::Deserialize)]
struct JsonObject{
    function_data: Vec<FunctionData>
}

#[derive(serde::Deserialize)]
struct RunResponseObject {
    id: String,
    status: String
}

#[derive(serde::Deserialize)]
struct ImageGenOutput {
    image: String,
    seed: u32
}

#[derive(serde::Deserialize)]
struct OutputResponseObject {
    delayTime: Option<u64>,
    executionTime: Option<u64>,
    id: Option<String>,
    output: Option<Vec<ImageGenOutput>>,
    status: Option<String>
}

#[derive(serde::Serialize)]
struct ImageGenRunInput {
    prompt: String,
    negative_prompt: String,
    width: u32,
    height: u32,
    init_image: String,
    mask: String,
    guidance_scale: f32,
    num_inference_steps: u32,
    num_outputs: u32,
    prompt_strength: f32,
    scheduler: String,
    seed: u32
}

#[derive(serde::Serialize)]
struct ImageGenRequest {
    input: ImageGenRunInput,
    webhook: String
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        task::spawn(async move {
            let author_id: String = env::var("USER_ID").expect("Can't get USER_ID system variable");
            let debug_enabled: String = env::var("DEBUG").expect("Can't get DEBUG system variable");

            if debug_enabled != "1".to_owned() ||
            author_id == msg.author.id.to_string() {
                let message_prefix: String;
                if debug_enabled == "1".to_owned() {
                    message_prefix = "DEBUG: ".to_string();
                } else {
                    message_prefix = "".to_string();
                }
                if msg.author.id != ctx.cache.current_user().id {
                    if msg.content.starts_with("!delta") {
                        let typing = Typing::start(ctx.clone().http, msg.channel_id.into())
                            .expect("Unable to start typing");
                        let msg_safe = msg.content_safe(ctx.clone().cache);
                        let mut full_command = msg_safe.splitn(2, " ");
                        let command_string = full_command.next().unwrap();
                        let command_data = full_command.next().unwrap();
                        let function_json_string = tokio::fs::read_to_string("assets/functions.json").await.expect("Unable to get function data");
                        let function_object: JsonObject = serde_json::from_str(&function_json_string).unwrap();
                        let current_function: FunctionData = function_object.function_data.into_iter().filter(|function| function.function_command == command_string).next().unwrap();
                        let command_function_str = current_function.function_type.as_str();
                        let command_api_str = current_function.function_api_key;
                        let image_path: Option<PathBuf>;

                        match command_function_str {
                            "openai_dalle"=>{
                                image_path = Some(generate_dalle(command_data.to_owned(), msg.clone()).await);
                            }
                            "runpod_image"=>{
                                image_path = Some(generate_runpod_image(command_data.to_owned(), command_api_str, msg.clone()).await);
                            }
                            _=>{
                                msg.reply(ctx.http, format!("{}Your command has not been recognised, sorry I couldn't help!", message_prefix)).await.expect("Unable to send default command reply");
                                return;
                            }
                        }

                        let image_file = [(&tokio::fs::File::open(image_path.clone().unwrap()).await.expect("Unable to open DALL-E image"), "image.png")];

                        msg.channel_id.send_message(ctx.http, |message: &mut CreateMessage<'_>| {
                            message.reference_message(&msg);
                            message.allowed_mentions(|am| {
                                am.replied_user(true);
                                am
                            });
                            message.files(image_file);
                            message.content(format!("{}Hello, thank you for the request! Here is the image you've requested!", message_prefix));
                            message
                        }).await.expect("DALL-E message failed to send");
                        typing.stop().expect("Unable to stop typing");
                        fs::remove_file(image_path.unwrap()).expect("Unable to delete imagegen file");
                    } else if msg.mentions_user_id(ctx.cache.current_user().id) {
                        let typing = Typing::start(ctx.clone().http, msg.channel_id.into())
                            .expect("Unable to start typing");
                        let mut reply_msg = msg.clone();
                        let response_vec = text_reply(msg, &ctx, ctx.cache.current_user().id.into()).await;
                        for response in response_vec {
                            reply_msg = reply_msg.reply(&ctx.http, format!("{}{}", message_prefix, response)).await.expect("Error sending message");
                        }
                        typing.stop().expect("Unable to stop typing");

                    }
                }
            }
        });
    }

    async fn ready(&self, _: Context, ready: Ready) {
            println!("{} is connected!", ready.user.name);
    }
}
#[tokio::main]
async fn main() {
    let token = env::var("DISCORD_TOKEN")
    .expect("Expected a token in the environment");

    let intents = GatewayIntents::GUILD_MESSAGES
    | GatewayIntents::DIRECT_MESSAGES
    | GatewayIntents::MESSAGE_CONTENT;

    let mut client = serenity::Client::builder(&token, intents).event_handler(Handler).await.expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}

async fn text_reply(msg: Message, cache: impl CacheHttp, user_id: u64) -> Vec<String> {
    let client = Client::new();
    let current_cache = Cache::new();
    let current_http = Http::new(&env::var("DISCORD_TOKEN")
    .expect("Expected a token in the environment"));
    let current_channel = msg.channel(cache)
        .await
        .expect("Failed to get current channel to get the ChatGPT context");
    let mut current_message = msg;
    let context_messages: &mut Vec<ChatCompletionRequestMessage> = &mut Vec::new();
    // Default to user role as the bot needs to be called to reply
    let mut current_role = Role::User;
    let chatgpt_system_details = env::var("SYSTEM_DETAILS").expect("");
    let max_tokens: u16 = 4096;

    if current_message.message_reference.is_none() {
        context_messages.push(
            generate_chat_messages(current_role, current_message.author.id.0.to_string() + "|" + &current_message.author.name + ": " + &current_message.content_safe(current_cache.as_ref()))
        );
    } else {
        loop{
            context_messages.push(
                generate_chat_messages(current_role, current_message.author.id.0.to_string() + "|" + &current_message.author.name + ": " + &current_message.content_safe(current_cache.as_ref()))
            );

            current_message = current_channel
            .id()
            .message(&current_http, current_message.message_reference.expect("Unable to get previous message in chain (1)").message_id.expect("Unable to get ID of previous message in chain (2)"))
            .await
            .expect("Unable to reieve previous message");

            if current_message.author.id == user_id {
                current_role = Role::Assistant;
            } else {
                current_role = Role::User
            }

            if current_message.message_reference.is_none()  {
                break;
            }
        }
    }

    //context_messages.push(
    //    generate_chat_messages(Role::User, "Do not start any reply with 'Delta:', 'Delta Bot:' or anything similar. If you wish to mention someone, you can use <@[USER ID]>, user messages start with [USER ID]|[USER NAME]: and the ID for the mention can be pulled from there".to_owned())
    //);

    context_messages.push(
        generate_chat_messages(Role::System, format!("{chatgpt_system_details} You are a cheerful android that responds to the name Delta, you care very much for your creator and do a lot of errands around your local town for them. She is also fond of using emotes in her replies. If someone asks you a question then you do your best to reply! Do not start any reply with 'Delta:', 'Delta Bot:' or anything similar. If you wish to mention someone, you can use <@[USER ID]>, user messages start with [USER ID]|[USER NAME]: and the ID for the mention can be pulled from there. Please don't put an @ in front of usernames when you reply, that is only needed when using the user ID. Make all of your responses longform").to_owned())
    );

    context_messages.reverse();

    let chatgpt_request = CreateChatCompletionRequestArgs::default()
        .model("gpt-4-1106-preview")
        .temperature(1.0)
        .messages(&**context_messages)
        .max_tokens(max_tokens)
        .build()
            .expect("Unable to construct message reply");

    let response_choices = client.chat().create(chatgpt_request).await.expect("Unable to generate reply");
    let response = &response_choices.choices[0].message.content;
    let response_text = response.as_ref().expect("Unable to convert the GPT response to text");
    let mut return_vec: Vec<String> = Vec::new();

    if response_text.len() > 2000 {
        let chars: Vec<char> = response_text.chars().collect();
        let total_chunks = (response_text.len()/1980) + 1;
        let chunk_size = 1980;
        let mut split = chars.chunks(chunk_size)
            .map(|chunk| chunk.iter().collect::<String>())
            .collect::<Vec<_>>();
        let split_clone = split.clone();

        for (index, _element) in split_clone.iter().enumerate() {
            let code_vec: Vec<_> = split[index]
                .match_indices("```")
                .collect();
            if (code_vec.len() % 2) == 1 {
                split[index] = split[index].to_owned() + "```";
                if index != split.len() - 1 {
                    split[index + 1] = "```".to_owned() + &split[index + 1];
                }
            }
            split[index] = format!("{}/{total_chunks}: {}", index + 1, split[index])
        }
        return_vec = split;
    } else {
        return_vec.push(response_text.to_owned())
    }
    return return_vec;
}

async fn generate_dalle (prompt_text: String, msg: Message) -> PathBuf {
    let client = Client::new();
    let request = CreateImageRequestArgs::default()
        .prompt(prompt_text)
        .n(1)
        .response_format(ResponseFormat::Url)
        .size(ImageSize::S1024x1024)
        .user("Delta-Bot")
        .model(ImageModel::DallE3)
        .build()
        .expect("Exception when building DALL-E image request");

    let response = client.images().create(request).await.expect("Exception when getting DALL-E image response");

    let mut image_path = response.save("./images").await.expect("Unable to save returned image");

    return image_path.remove(0);
}

async fn generate_runpod_image (prompt_text: String, model_ref: String, msg: Message) -> PathBuf {
    let runpod_auth_key = env::var("RUNPOD_API_KEY").expect("");
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("accept", HeaderValue::from_static("application/json"));
    headers.insert("authorization", HeaderValue::from_str(&runpod_auth_key).unwrap());
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    let client = reqwest::Client::new();
    let run_response: RunResponseObject = client.post(format!("https://api.runpod.ai/v2/{}/run", model_ref))
       .headers(headers.clone())
       .body(format!("{{
           \"input\": {{
              \"prompt\": \"{}\",
               \"height\": 768,
               \"width\": 768,
               \"scheduler\": \"DDIM\",
               \"num_inference_steps\": 40
           }}
       }}", prompt_text))
       .send()
       .await
       .expect("Failed to start runpod job")
       .json()
       .await
       .expect("Unable to convert JSON to object");

    let mut status_response: OutputResponseObject;

    loop {
        status_response = client.post(format!("https://api.runpod.ai/v2/{}/status/{}", model_ref, run_response.id))
        .headers(headers.clone())
        .send()
        .await
        .expect("Unable to send request to RunPod endpoint")
        .json()
        .await
        .expect("Unable to convert JSON to object");

        if !(status_response.status.clone().unwrap() == "IN_QUEUE" || status_response.status.clone().unwrap() == "IN_PROGRESS") {
            break;
        }

        sleep(Duration::from_secs(2));
    }
    
    let image_path: PathBuf = Path::new(&format!("./images/{}.png", Uuid::from_u128(rand::random::<u128>()))).to_path_buf();
    let mut image_data_response: &[u8] = &reqwest::get(status_response.output.unwrap()[0]
        .image.to_owned())
        .await
        .expect("Unable to download the generated RunPod image")
        .bytes()
        .await
        .expect("Unable to convert image link to bytes");
    let mut image_file = File::create(image_path.clone()).expect("Unable to create image file");
    copy(&mut image_data_response, &mut image_file).expect("Error writing Runpod image file");
    return image_path;
}

async fn return_error (msg: Message, error_msg : String) {
    let current_http = Http::new(&env::var("DISCORD_TOKEN")
    .expect("Expected a token in the environment"));    
    msg.reply(current_http, format!("Apologies, your request cannot be completed, the error is as follows:\n```{}```", error_msg))
    .await
    .expect("Error showing an error");
}

fn generate_chat_messages (current_role: Role, content: String) -> ChatCompletionRequestMessage {
    if current_role == Role::Assistant {
        return async_openai::types::ChatCompletionRequestMessage::Assistant(ChatCompletionRequestAssistantMessageArgs::default()
        .content(content)
        .build()
        .expect("Unable to generate message to send to ChatGPT"))
    } else if current_role == Role::Function {
        return async_openai::types::ChatCompletionRequestMessage::Function(ChatCompletionRequestFunctionMessageArgs::default()
        .content(content)
        .build()
        .expect("Unable to generate message to send to ChatGPT"))
    } else if current_role == Role::System {
        return async_openai::types::ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessageArgs::default()
        .content(content)
        .build()
        .expect("Unable to generate message to send to ChatGPT"))
    } else if current_role == Role::Tool {
        return async_openai::types::ChatCompletionRequestMessage::Tool(ChatCompletionRequestToolMessageArgs::default()
        .content(content)
        .build()
        .expect("Unable to generate message to send to ChatGPT"))
    } else {
        return async_openai::types::ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessageArgs::default()
        .content(content)
        .build()
        .expect("Unable to generate message to send to ChatGPT"))
    }
}