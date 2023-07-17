use std::{env, path::PathBuf};

use serenity::{
    async_trait,
    model::{channel::Message, gateway::Ready},
    prelude::*, client::Cache, http::{CacheHttp, Http, Typing}, builder::CreateMessage,
};

use async_openai::{
    types::{ChatCompletionRequestMessageArgs, Role, ChatCompletionRequestMessage, CreateChatCompletionRequestArgs, CreateImageRequestArgs, ImageSize, ResponseFormat}, Client,
};
use tokio::task;

#[derive(serde::Deserialize)]
struct FunctionData {
    function_command: String,
    function_type: String,
    // Planned to be used when connecting to serverless image generation services
    function_api_key: String
}

#[derive(serde::Deserialize)]
struct JsonObject{
    function_data: Vec<FunctionData>
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        let author_id: String = env::var("USER_ID").expect("Can't get USER_ID system variable");

        if env::var("DEBUG").expect("Can't get DEBUG system variable") != 1.to_string() ||
        author_id == msg.author.id.to_string() {
            if msg.author.id != ctx.cache.current_user().id {
                if msg.content.starts_with("!delta") {
                    let msg_safe = msg.content_safe(ctx.clone().cache);
                    let mut full_command = msg_safe.splitn(2, " ");
                    let command_string = full_command.next().unwrap();
                    let command_data = full_command.next().unwrap();
                    let function_json_string = tokio::fs::read_to_string("assets/functions.json").await.expect("Unable to get function data");
                    let function_object: JsonObject = serde_json::from_str(&function_json_string).unwrap();
                    let current_function: FunctionData = function_object.function_data.into_iter().filter(|function| function.function_command == command_string).next().unwrap();
                    let command_function_str = current_function.function_type.as_str();

                    match command_function_str {
                        "openai_dalle"=>{
                            let image_path: PathBuf = generate_dalle(command_data.to_owned()).await;
                            let image_file = [(&tokio::fs::File::open(image_path).await.expect("Unable to open DALL-E image"), "image.png")];

                            msg.channel_id.send_message(ctx.http, |message: &mut CreateMessage<'_>| {
                                message.reference_message(&msg);
                                message.allowed_mentions(|am| {
                                    am.replied_user(true);
                                    am
                                });
                                message.files(image_file);
                                message.content("Hello, thank you for the request! Here is the image you've requested!");
                                message
                            }).await.expect("DALL-E message failed to send");
                        }
                        _=>{msg.reply(ctx.http, "Your command has not been recognised, sorry I couldn't help!".to_owned()).await.expect("Unable to send default command reply");}
                    }
                } else if msg.mentions_user_id(ctx.cache.current_user().id) {
                    task::spawn(async move {
                        let typing = Typing::start(ctx.clone().http, msg.channel_id.into())
                            .expect("Unable to start typing");
                        let mut reply_msg = msg.clone();
                        let response_vec = text_reply(msg, &ctx, ctx.cache.current_user().id.into()).await;
                        for response in response_vec {
                            reply_msg = reply_msg.reply(&ctx.http, format!("DEBUG: {response}")).await.expect("Error sending message");
                        }
                        typing.stop().expect("Unable to stop typing");
                    });

                }
            }
        }

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

    if current_message.message_reference.is_none() {
        context_messages.push(
            ChatCompletionRequestMessageArgs::default()
            .role(current_role)
            .content(current_message.content_safe(current_cache.as_ref()))
            .build()
            .expect("Failed to setup ChatGPT context")
        );
    } else {
        loop{
            context_messages.push(
                ChatCompletionRequestMessageArgs::default()
                .role(current_role)
                .content(current_message.content_safe(current_cache.as_ref()))
                .build()
                .expect("Failed to setup ChatGPT context")
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

    context_messages.push(
        ChatCompletionRequestMessageArgs::default()
        .role(Role::User)
        .content("Keep your replies short, do not start any reply with 'Delta:', 'Delta Bot:' or anything similar. If you wish to mention someone, you can use <@[USER ID]>, user messages start with [USER NAME]|[USER ID] and the ID for the mention can be pulled from there")
        .build()
        .expect("Failed to setup ChatGPT context")
    );

    context_messages.push(
        ChatCompletionRequestMessageArgs::default()
        .role(Role::System)
        .content(format!("{chatgpt_system_details} You are a cheerful android that responds to the name Delta, you care very much for your creator and do a lot of errands around your local town for them. She is also fond of using emotes in her replies. Your replies are short and rather to the point. If someone asks you a question then you do your best to reply!"))
        .build()
        .expect("Failed to setup ChatGPT context")
    );

    context_messages.reverse();

    let chatgpt_request = CreateChatCompletionRequestArgs::default()
        .model("gpt-3.5-turbo")
        .temperature(1.3)
        .messages(&**context_messages)
        .build()
            .expect("Unable to construct message reply");

    let response_choices = client.chat().create(chatgpt_request).await.expect("Unable to generate reply");
    let response = &response_choices.choices[0].message.content;
    let mut return_vec: Vec<String> = Vec::new();

    if response.len() > 2000 {
        let chars: Vec<char> = response.chars().collect();
        let total_chunks = (response.len()/1980) + 1;
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
        return_vec.push(response.to_owned())
    }
    return return_vec;
}

async fn generate_dalle (prompt_text: String) -> PathBuf {
    let client = Client::new();
    let request = CreateImageRequestArgs::default()
        .prompt(prompt_text)
        .n(1)
        .response_format(ResponseFormat::Url)
        .size(ImageSize::S1024x1024)
        .user("Delta-Bot")
        .build()
        .expect("Exception when building DALL-E image request");

    let response = client.images().create(request).await.expect("Exception when getting DALL-E image response");

    let mut image_path = response.save("./images").await.expect("Unable to save returned image");

    return image_path.remove(0);
}