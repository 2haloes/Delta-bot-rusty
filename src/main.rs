use std::{borrow::Borrow, env, fs::{File, self}, io::copy, path::{PathBuf, Path}, result, thread::sleep, time::Duration};

use serenity::{
    async_trait,
    model::{channel::Message, gateway::Ready},
    prelude::*, client::Cache, http::{CacheHttp, Http, Typing}, builder::CreateMessage,
};

use async_openai::{
    types::{ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestFunctionMessageArgs, ChatCompletionRequestMessage, ChatCompletionRequestMessageContentPart, ChatCompletionRequestMessageContentPartImageArgs, ChatCompletionRequestMessageContentPartTextArgs, ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestToolMessageArgs, ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs, CreateImageRequestArgs, ImageModel, ImageSize, ResponseFormat, Role}, Client,
};
use tokio::task;
use reqwest::{header::HeaderValue, Response, Url};
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
            let author_id: String = env::var("USER_ID").unwrap_or("-1".to_owned());
            let debug_enabled: String = env::var("DEBUG").unwrap_or("0".to_owned());

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
                        let typing = match Typing::start(ctx.clone().http, msg.channel_id.into())
                            {
                                Ok(t) => t,
                                Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
                            };
                        let msg_safe = msg.borrow().content_safe(ctx.clone().cache);
                        let mut full_command = msg_safe.splitn(2, " ");
                        let command_string = match full_command.next()
                            {
                                Some(t) => t,
                                None => return_error(msg.clone(), "Unable to process command string".to_owned()).await.unwrap(),
                            };
                        let command_data = match full_command.next()
                            {
                                Some(t) => t,
                                None => return_error(msg.clone(), "Unable to process command data string".to_owned()).await.unwrap(),
                            };
                        let function_json_string = match tokio::fs::read_to_string("assets/functions.json").await
                            {
                                Ok(t) => t,
                                Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
                            };
                        let function_object: JsonObject = match serde_json::from_str(&function_json_string)
                            {
                                Ok(t) => t,
                                Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
                            };
                        let current_function: FunctionData = match function_object.function_data.into_iter()
                            .filter(|function| function.function_command == command_string).next()
                            {
                                Some(t) => t,
                                None => return_error(msg.clone(), "Unable to process current function string".to_owned()).await.unwrap(),
                            };
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
                                // If a reply can't be made... is there a point in trying to reply with a different error?
                                msg.reply(ctx.http, format!("{}Your command has not been recognised, sorry I couldn't help!", message_prefix)).await.expect("Unable to send default command reply");
                                return;
                            }
                        }

                        let image_file_result = &tokio::fs::File::open(image_path.clone().unwrap()).await;
                        let image_file = [(match image_file_result
                            {
                                Ok(t) => t,
                                Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
                            }, 
                            "image.png")];

                        match msg.borrow().channel_id.send_message(ctx.http, |message: &mut CreateMessage<'_>| {
                            message.reference_message(&msg);
                            message.allowed_mentions(|am| {
                                am.replied_user(true);
                                am
                            });
                            message.files(image_file);
                            message.content(format!("{}Hello, thank you for the request! Here is the image you've requested!", message_prefix));
                            message
                        }).await
                            {
                                Ok(t) => t,
                                Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
                            };
                        match typing.stop()
                            {
                                Some(t) => t,
                                None => return_error(msg.clone(), "Unable to process stop typing after sending image".to_owned()).await.unwrap(),
                            };
                        match fs::remove_file(image_path.unwrap())
                            {
                                Ok(t) => t,
                                Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
                            };
                    } else if msg.mentions_user_id(ctx.cache.current_user().id) {
                        let typing = match Typing::start(ctx.clone().http, msg.channel_id.into())
                            {
                                Ok(t) => t,
                                Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
                            };
                        let mut reply_msg = msg.clone();
                        let response_vec = text_reply(msg.clone(), &ctx, ctx.cache.current_user().id.into()).await;
                        for response in response_vec {
                            // If the reply cannot be sent, then sending an error message might be pointless
                            // If the issue is within generating the text, that will be handled before now
                            reply_msg = reply_msg.reply(&ctx.http, format!("{}{}", message_prefix, response)).await.expect("Error sending message");
                        }
                        match typing.stop()
                            {
                                Some(t) => t,
                                None => return_error(msg.clone(), "Unable to process stop typing".to_owned()).await.unwrap(),
                            };

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
    let current_channel = msg.clone().channel(cache)
        .await
        .expect("Failed to get current channel to get the ChatGPT context");
    let mut current_message = msg.clone();
    let context_messages: &mut Vec<ChatCompletionRequestMessage> = &mut Vec::new();
    // Default to user role as the bot needs to be called to reply
    let mut current_role = Role::User;
    let chatgpt_system_details = env::var("SYSTEM_DETAILS").unwrap_or("".to_owned());
    let max_tokens: u16 = 4096;
    let mut message_content: String;
    let mut message_vec_content: Vec<ChatCompletionRequestMessageContentPart> = Vec::new();
    let mut message_model = "gpt-4-turbo-preview";
    let mut using_vision = false;

    if current_message.message_reference.is_none() {
        message_content = current_message.author.id.0.to_string() + "|" + &current_message.author.name + ": " + &current_message.content;
        message_vec_content.push(ChatCompletionRequestMessageContentPartTextArgs::default().text(message_content.clone()).build().unwrap().into());
        if !current_message.attachments.is_empty() {
            // OpenAI only supports JPGs, PNGs and static GIFs
            // Until I impliment this better, I'm going to hedge my bets that any gifs will be animated
            let image_extentions = vec!(
                ".jpg",
                ".jpeg",
                ".png"
            );
            // Bunch of text formats, just don't want to try and process a 5MB binary or video
            // Seriously, I've used the Wiki page for file formats, this should cover almost everything
            // Plain text docs, scripting, programming source
            let text_extentions = get_text_type();
            for message_attachment in current_message.attachments {
                let attachment_link = message_attachment.url;
                let mut parsed_attachment_link = match Url::parse(&attachment_link)
                    {
                        Ok(t) => t,
                        Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
                    };
                parsed_attachment_link.set_query(None);
                let parsed_attachement_link_string = parsed_attachment_link.to_string();

                if image_extentions.iter().any(|suffix| parsed_attachement_link_string.ends_with(suffix)) {
                    message_vec_content.push(ChatCompletionRequestMessageContentPartImageArgs::default().image_url(attachment_link).build().unwrap().into());

                    if !using_vision {
                        message_model = "gpt-4-vision-preview";
                        using_vision = true;
                    }
                }
                else if text_extentions.iter().any(|suffix| parsed_attachement_link_string.ends_with(suffix)) {
                    let text_response = match reqwest::get(attachment_link).await
                    {
                        Ok(t) => t,
                        Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
                    };
                    let text_content = match text_response.text().await
                    {
                        Ok(t) => t,
                        Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
                    };
                    let mut attachment_path_split = parsed_attachement_link_string.rsplit("/");
                    let attachment_name = match attachment_path_split.next()
                    {
                        Some(t) => t,
                        None => return_error(msg.clone(), "Unable to process stop typing".to_owned()).await.unwrap(),
                    };

                    message_vec_content.push(ChatCompletionRequestMessageContentPartTextArgs::default().text(format!("Content of the file: {attachment_name} ```{text_content}```")).build().unwrap().into());
                }
            }
        }
        context_messages.push(
            generate_chat_messages(
                current_role, 
                message_vec_content,
                message_content,
                msg.clone()
            )
            .await
        );
    } else {
        loop{
            message_vec_content = Vec::new();
            message_content = match current_role {
                Role::User => current_message.author.id.0.to_string() + "|" + &current_message.author.name + ": " + &current_message.content,
                _ => current_message.content,
            };
            message_vec_content.push(ChatCompletionRequestMessageContentPartTextArgs::default().text(message_content.clone()).build().unwrap().into());
            if !current_message.attachments.is_empty() {
                // OpenAI only supports JPGs, PNGs and static GIFs
                // Until I impliment this better, I'm going to hedge my bets that any gifs will be animated
                let image_extentions = vec!(
                    ".jpg",
                    ".jpeg",
                    ".png"
                );
                // Bunch of text formats, just don't want to try and process a 5MB binary or video
                // Seriously, I've used the Wiki page for file formats, this should cover almost everything
                // Plain text docs, scripting, programming source
                let text_extentions: Vec<&str> = get_text_type();
                for message_attachment in current_message.attachments {
                    let attachment_link = message_attachment.url;
                    let mut parsed_attachment_link = match Url::parse(&attachment_link)
                        {
                            Ok(t) => t,
                            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
                        };
                    parsed_attachment_link.set_query(None);
                    let parsed_attachement_link_string = parsed_attachment_link.to_string();
    
                    if image_extentions.iter().any(|suffix| parsed_attachement_link_string.ends_with(suffix)) {
                        message_vec_content.push(ChatCompletionRequestMessageContentPartImageArgs::default().image_url(attachment_link).build().unwrap().into());
    
                        if !using_vision {
                            message_model = "gpt-4-vision-preview";
                            using_vision = true;
                        }
                    }
                    else if text_extentions.iter().any(|suffix| parsed_attachement_link_string.ends_with(suffix)) {
                        let text_response = match reqwest::get(attachment_link).await
                        {
                            Ok(t) => t,
                            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
                        };
                        let text_content = match text_response.text().await
                        {
                            Ok(t) => t,
                            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
                        };
                        let mut attachment_path_split = parsed_attachement_link_string.rsplit("/");
                        let attachment_name = match attachment_path_split.next()
                        {
                            Some(t) => t,
                            None => return_error(msg.clone(), "Unable to process stop typing".to_owned()).await.unwrap(),
                        };
    
                        message_vec_content.push(ChatCompletionRequestMessageContentPartTextArgs::default().text(format!("Content of the file: {attachment_name} ```{text_content}```")).build().unwrap().into());
                    }
                }
            }
            context_messages.push(
                generate_chat_messages(
                    current_role, 
                    message_vec_content.clone(),
                    message_content,
                    msg.clone()
                )
                .await
            );

            let current_message_reference = current_message.message_reference.expect("Unable to get previous message in chain (1)");
            let current_message_id = current_message_reference.message_id.expect("Unable to get ID of previous message in chain (2)");

            current_message = current_channel
            .id()
            .message(&current_http, current_message_id)
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

    // Not needed now as GPT4-Turbo fixed the system message problem
    //context_messages.push(
    //    generate_chat_messages(Role::User, "Do not start any reply with 'Delta:', 'Delta Bot:' or anything similar. If you wish to mention someone, you can use <@[USER ID]>, user messages start with [USER ID]|[USER NAME]: and the ID for the mention can be pulled from there".to_owned())
    //);

    context_messages.push(
        generate_chat_messages(
            Role::System, 
            Vec::new(),
            format!("{chatgpt_system_details} You are a cheerful android that responds to the name Delta, you care very much for your creator and do a lot of errands around your local town for them. She is also fond of using emotes in her replies. If someone asks you a question then you do your best to reply! Do not start any reply with 'Delta:', 'Delta Bot:' or anything similar. If you wish to mention someone, you can use <@[USER ID]>, user messages start with [USER ID]|[USER NAME]: and the ID for the mention can be pulled from there. Please don't put an @ in front of usernames when you reply, that is only needed when using the user ID. Please do not mention people in the [USER ID]|[USER NAME] format, this is only for your information, please do not start your own messages with this format. Make all of your responses longform").to_owned(),
            msg.clone()
        )
        .await
    );

    context_messages.reverse();

    let chatgpt_request = match CreateChatCompletionRequestArgs::default()
        .model(message_model)
        .temperature(1.0)
        .messages(&**context_messages)
        .max_tokens(max_tokens)
        .build()
        {
            Ok(t) => t,
            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
        };

    let response_choices = match client.chat().create(chatgpt_request).await
        {
            Ok(t) => t,
            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
        };
    let response = &response_choices.choices[0].message.content;
    let response_text = match response.as_ref()
        {
            Some(t) => t,
            None => return_error(msg.clone(), "Unable to process stop typing".to_owned()).await.unwrap(),
        };
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
    let request = match CreateImageRequestArgs::default()
        .prompt(prompt_text)
        .n(1)
        .response_format(ResponseFormat::Url)
        .size(ImageSize::S1024x1024)
        .user("Delta-Bot")
        .model(ImageModel::DallE3)
        .build()
        {
            Ok(t) => t,
            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
        };

    let response = match client.images().create(request).await
        {
            Ok(t) => t,
            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
        };

    let mut image_path = match response.save("./images").await
        {
            Ok(t) => t,
            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
        };

    return image_path.remove(0);
}

async fn generate_runpod_image (prompt_text: String, model_ref: String, msg: Message) -> PathBuf {
    let runpod_auth_key = match env::var("RUNPOD_API_KEY")
        {
            Ok(t) => t,
            Err(e) => return_error(msg.clone(), "No runpod API key found".to_owned()).await.unwrap(),
        };
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("accept", HeaderValue::from_static("application/json"));
    headers.insert("authorization", HeaderValue::from_str(&runpod_auth_key).unwrap());
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    let client = reqwest::Client::new();
    let run_response: Response = match client.post(format!("https://api.runpod.ai/v2/{}/run", model_ref))
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
        {
            Ok(t) => t,
            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
        };
       

    let run_response_json: RunResponseObject = match run_response.json()
        .await
        {
            Ok(t) => t,
            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
        };

    let mut status_response_json: OutputResponseObject;

    loop {
        let status_response = match client.post(format!("https://api.runpod.ai/v2/{}/status/{}", model_ref, run_response_json.id))
            .headers(headers.clone())
            .send()
            .await
            {
                Ok(t) => t,
                Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
            };
        

        status_response_json = match status_response.json()
            .await
            {
                Ok(t) => t,
                Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
            };

        if !(status_response_json.status.clone().unwrap() == "IN_QUEUE" || status_response_json.status.clone().unwrap() == "IN_PROGRESS") {
            break;
        }

        sleep(Duration::from_secs(2));
    }
    
    let image_path: PathBuf = Path::new(&format!("./images/{}.png", Uuid::from_u128(rand::random::<u128>()))).to_path_buf();
    let mut image_data_response: Response = match reqwest::get(status_response_json.output.unwrap()[0]
        .image.to_owned())
        .await
        {
            Ok(t) => t,
            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
        };
        
    let image_data_response_bytes = match image_data_response.bytes()
        .await
        {
            Ok(t) => t,
            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
        };

    let mut image_file = File::create(image_path.clone()).expect("Unable to create image file");
    match copy(&mut image_data_response_bytes.as_ref(), &mut image_file)
        {
            Ok(t) => t,
            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
        };
    return image_path;
}

async fn return_error<T> (msg: Message, error_msg : String) -> Option<T> {
    let current_http = Http::new(&env::var("DISCORD_TOKEN")
    .expect("Expected a token in the environment - ERROR HANDLER"));    
    // Not using the return_error function as it leads here and if there's an issue here, it'll just loop
    msg.reply(current_http, format!("Apologies, your request cannot be completed, the error is as follows:\n```{}```", error_msg))
    .await
    .expect("Error showing an error - ERROR HANDLER");

    panic!("{}", format!("An error has occured: {error_msg}"))
}

async fn generate_chat_messages (current_role: Role, content: Vec<ChatCompletionRequestMessageContentPart>, content_string_only: String, msg: Message) -> ChatCompletionRequestMessage {
    if current_role == Role::Assistant {
        return match ChatCompletionRequestAssistantMessageArgs::default()
        .content(content_string_only)
        .build()
        {
            Ok(t) => async_openai::types::ChatCompletionRequestMessage::Assistant(t),
            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
        }
    } else if current_role == Role::Function {
        return match ChatCompletionRequestFunctionMessageArgs::default()
        .content(content_string_only)
        .build()
        {
            Ok(t) => async_openai::types::ChatCompletionRequestMessage::Function(t),
            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
        }
    } else if current_role == Role::System {
        return match ChatCompletionRequestSystemMessageArgs::default()
        .content(content_string_only)
        .build()
        {
            Ok(t) => async_openai::types::ChatCompletionRequestMessage::System(t),
            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
        }
    } else if current_role == Role::Tool {
        return match ChatCompletionRequestToolMessageArgs::default()
        .content(content_string_only)
        .build()
        {
            Ok(t) => async_openai::types::ChatCompletionRequestMessage::Tool(t),
            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
        }
    } else {
        return match ChatCompletionRequestUserMessageArgs::default()
        .content(content)
        .build()
        {
            Ok(t) => async_openai::types::ChatCompletionRequestMessage::User(t),
            Err(e) => return_error(msg.clone(), e.to_string()).await.unwrap(),
        }
    }
}

fn get_text_type() -> Vec<&'static str> {
    return vec!(
        ".txt",
        ".json",
        ".md",
        ".adb",
        ".ads",
        ".ahk",
        ".applescript",
        ".scpt",
        ".scptd",
        ".as",
        ".au3",
        ".awk",
        ".bat",
        ".bas",
        ".btm",
        ".class",
        ".cljs",
        ".cmd",
        ".coffee",
        ".c",
        ".cia",
        ".cpp",
        ".cs",
        ".fs",
        ".egg",
        ".egt",
        ".erb",
        ".go",
        ".hta",
        ".ibi",
        ".ici",
        ".ijs",
        ".ino",
        ".ipynb",
        ".itcl",
        ".js",
        ".jsfl",
        ".kt",
        ".lua",
        ".m",
        ".mrc",
        ".ncf",
        ".nuc",
        ".nud",
        ".nut",
        ".nqp",
        ".o",
        ".pde",
        ".php",
        ".pl",
        ".pm",
        ".ps1",
        ".ps1xml",
        ".psc1",
        ".psd1",
        ".psm1",
        ".py",
        ".pyc",
        ".pyo",
        ".r",
        ".rb",
        ".rdp",
        ".red",
        ".rs",
        ".sb2",
        ".sb3",
        ".scpt",
        ".scptd",
        ".sdl",
        ".sh",
        ".sprite3",
        ".spwn",
        ".syjs",
        ".sypy",
        ".tcl",
        ".tns",
        ".ts",
        ".vbs",
        ".xpl",
        ".ebuild",
        ".csv",
        ".html",
        ".css",
        ".ini",
        ".tsv",
        ".yaml",
        ".rst",
        ".adoc",
        ".asciidoc",
        ".yni",
        ".cnf",
        ".conf",
        ".cfg",
        ".log",
        ".asc",
        ".text",
        ".ADA",
        ".ADB",
        ".ADS",
        ".ASM",
        ".S",
        ".BAS",
        ".BB",
        ".BMX",
        ".C",
        ".CLJ",
        ".CLS",
        ".COB",
        ".CBL",
        ".CPP",
        ".CC",
        ".CXX",
        ".CBP",
        ".CS",
        ".CSPROJ",
        ".D",
        ".DBA",
        ".DBPro123",
        ".E",
        ".EFS",
        ".EGT",
        ".EL",
        ".FOR",
        ".FTN",
        ".F",
        ".F77",
        ".F90",
        ".FRM",
        ".FRX",
        ".FTH",
        ".GED",
        ".GM6",
        ".GMD",
        ".GMK",
        ".GML",
        ".GO",
        ".H",
        ".HPP",
        ".HXX",
        ".HS",
        ".HX",
        ".I",
        ".INC",
        ".JAVA",
        ".JS",
        ".L",
        ".LGT",
        ".LISP",
        ".M",
        ".M4",
        ".ML",
        ".MSQR",
        ".N",
        ".NB",
        ".P",
        ".PAS",
        ".PP",
        ".PHP",
        ".PHP3",
        ".PHP4",
        ".PHP5",
        ".PHPS",
        ".Phtml",
        ".PIV",
        ".PL",
        ".PM",
        ".PLI",
        ".PL1",
        ".PRG",
        ".PRO",
        ".POL",
        ".PY",
        ".R",
        ".raku",
        ".rakumod",
        ".rakudoc",
        ".rakutest",
        ".nqp",
        ".RED",
        ".REDS",
        ".RB",
        ".RESX",
        ".RC",
        ".RC2",
        ".RKT",
        ".RKTL",
        ".RS",
        ".SCALA",
        ".SCI",
        ".SCE",
        ".SCM",
        ".SD7",
        ".SKB",
        ".SKC",
        ".SKD",
        ".SKF",
        ".SKG",
        ".SKI",
        ".SKK",
        ".SKM",
        ".SKO",
        ".SKP",
        ".SKQ",
        ".SKS",
        ".SKT",
        ".SKZ",
        ".SLN",
        ".SPIN",
        ".STK",
        ".SWG",
        ".TCL",
        ".VAP",
        ".VB",
        ".VBG",
        ".VBP",
        ".VIP",
        ".VBPROJ",
        ".VCPROJ",
        ".VDPROJ",
        ".XPL",
        ".XQ",
        ".XSL",
        ".Y"
    );
}
