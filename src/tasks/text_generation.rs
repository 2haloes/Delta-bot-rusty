use std::env;

use async_openai::{types::{ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestFunctionMessageArgs, ChatCompletionRequestMessage, ChatCompletionRequestMessageContentPart, ChatCompletionRequestMessageContentPartImageArgs, ChatCompletionRequestMessageContentPartTextArgs, ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestToolMessageArgs, ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs, Role}, Client};
use reqwest::Url;
use serenity::all::{CacheHttp, ChannelId, Http, Message, UserId};

use super::handle_errors::{return_error, return_error_reply};

pub async fn generate_chat_messages (current_role: Role, content: Vec<ChatCompletionRequestMessageContentPart>, content_string_only: String, msg: Message) -> ChatCompletionRequestMessage {
    if current_role == Role::Assistant {
        return match ChatCompletionRequestAssistantMessageArgs::default()
        .content(content_string_only)
        .build()
        {
            Ok(t) => async_openai::types::ChatCompletionRequestMessage::Assistant(t),
            Err(e) => return_error_reply(msg.clone(), e.to_string()).await.unwrap(),
        }
    } else if current_role == Role::Function {
        return match ChatCompletionRequestFunctionMessageArgs::default()
        .content(content_string_only)
        .build()
        {
            Ok(t) => async_openai::types::ChatCompletionRequestMessage::Function(t),
            Err(e) => return_error_reply(msg.clone(), e.to_string()).await.unwrap(),
        }
    } else if current_role == Role::System {
        return match ChatCompletionRequestSystemMessageArgs::default()
        .content(content_string_only)
        .build()
        {
            Ok(t) => async_openai::types::ChatCompletionRequestMessage::System(t),
            Err(e) => return_error_reply(msg.clone(), e.to_string()).await.unwrap(),
        }
    } else if current_role == Role::Tool {
        return match ChatCompletionRequestToolMessageArgs::default()
        .content(content_string_only)
        .build()
        {
            Ok(t) => async_openai::types::ChatCompletionRequestMessage::Tool(t),
            Err(e) => return_error_reply(msg.clone(), e.to_string()).await.unwrap(),
        }
    } else {
        return match ChatCompletionRequestUserMessageArgs::default()
        .content(content)
        .build()
        {
            Ok(t) => async_openai::types::ChatCompletionRequestMessage::User(t),
            Err(e) => return_error_reply(msg.clone(), e.to_string()).await.unwrap(),
        }
    }
}

pub async fn text_reply(msg: Message, cache: impl CacheHttp, user_id: u64, override_system: Option<String>) -> Vec<String> {
    let current_http = Http::new(&env::var("DISCORD_TOKEN")
    .expect("Expected a token in the environment"));
    let current_channel = msg.clone().channel(cache)
        .await
        .expect("Failed to get current channel to get the ChatGPT context");
    let mut current_message = msg.clone();
    let context_messages: &mut Vec<ChatCompletionRequestMessage> = &mut Vec::new();
    // Default to user role as the bot needs to be called to reply
    let mut current_role = Role::User;
    let chatgpt_system_details = override_system.unwrap_or(env::var("SYSTEM_DETAILS").unwrap_or("".to_owned()));
    let mut message_content: String;
    let mut message_vec_content: Vec<ChatCompletionRequestMessageContentPart> = Vec::new();

    if current_message.message_reference.is_none() {
        message_content = current_message.author.id.to_string() + "|" + &current_message.author.name + ": " + &current_message.content;
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
                        Err(e) => return_error_reply(msg.clone(), e.to_string()).await.unwrap(),
                    };
                parsed_attachment_link.set_query(None);
                let parsed_attachement_link_string = parsed_attachment_link.to_string();
                
                /*
                    If an image is in the context then attach it to the request
                */
                if image_extentions.iter().any(|suffix| parsed_attachement_link_string.ends_with(suffix)) {
                    message_vec_content.push(ChatCompletionRequestMessageContentPartImageArgs::default().image_url(attachment_link).build().unwrap().into());
                }
                /*
                    This adds the content of any attached text files to the message
                */
                else if text_extentions.iter().any(|suffix| parsed_attachement_link_string.ends_with(suffix)) {
                    let text_response = match reqwest::get(attachment_link).await
                    {
                        Ok(t) => t,
                        Err(e) => return_error_reply(msg.clone(), e.to_string()).await.unwrap(),
                    };
                    let text_content = match text_response.text().await
                    {
                        Ok(t) => t,
                        Err(e) => return_error_reply(msg.clone(), e.to_string()).await.unwrap(),
                    };
                    let mut attachment_path_split = parsed_attachement_link_string.rsplit("/");
                    let attachment_name = match attachment_path_split.next()
                    {
                        Some(t) => t,
                        None => return_error_reply(msg.clone(), "Unable to process stop typing".to_owned()).await.unwrap(),
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
                Role::User => current_message.author.id.to_string() + "|" + &current_message.author.name + ": " + &current_message.content,
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
                            Err(e) => return_error_reply(msg.clone(), e.to_string()).await.unwrap(),
                        };
                    parsed_attachment_link.set_query(None);
                    let parsed_attachement_link_string = parsed_attachment_link.to_string();
    
                    /*
                        If an image is in the context then use the vision model instead of the text model
                        The vison model has a lower call rate per day
                    */
                    if image_extentions.iter().any(|suffix| parsed_attachement_link_string.ends_with(suffix)) {
                        message_vec_content.push(ChatCompletionRequestMessageContentPartImageArgs::default().image_url(attachment_link).build().unwrap().into());
                    }
                    /*
                        This adds the content of any attached text files to the message
                    */
                    else if text_extentions.iter().any(|suffix| parsed_attachement_link_string.ends_with(suffix)) {
                        let text_response = match reqwest::get(attachment_link).await
                        {
                            Ok(t) => t,
                            Err(e) => return_error_reply(msg.clone(), e.to_string()).await.unwrap(),
                        };
                        let text_content = match text_response.text().await
                        {
                            Ok(t) => t,
                            Err(e) => return_error_reply(msg.clone(), e.to_string()).await.unwrap(),
                        };
                        let mut attachment_path_split = parsed_attachement_link_string.rsplit("/");
                        let attachment_name = match attachment_path_split.next()
                        {
                            Some(t) => t,
                            None => return_error_reply(msg.clone(), "Unable to process stop typing".to_owned()).await.unwrap(),
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
    context_messages.push(
        generate_chat_messages(
            Role::System, 
            Vec::new(),
            chatgpt_system_details,
            msg.clone()
        )
        .await
    );

    context_messages.reverse();
    return call_text_completion(context_messages.to_vec(), Some(msg), None, None).await;
}

async fn call_text_completion(context_messages: Vec<ChatCompletionRequestMessage>, msg: Option<Message>, requester_id: Option<UserId>, channel_id: Option<ChannelId>) -> Vec<String> {
    let client = Client::new();
    let max_tokens: u16 = 4096;
    let message_model = "gpt-4o";

    let chatgpt_request = match CreateChatCompletionRequestArgs::default()
        .model(message_model)
        .temperature(1.0)
        .messages(&*context_messages)
        .max_tokens(max_tokens)
        .build()
        {
            Ok(t) => t,
            Err(e) => {
                if msg.is_some() {
                    return_error_reply(msg.clone().unwrap(), e.to_string()).await.unwrap()
                } else {
                    return_error(requester_id.clone().unwrap(), channel_id.clone().unwrap(), e.to_string()).await.unwrap()
                }
                
            },
        };

    let response_choices = match client.chat().create(chatgpt_request).await
        {
            Ok(t) => t,
            Err(e) => {
                if msg.is_some() {
                    return_error_reply(msg.clone().unwrap(), e.to_string()).await.unwrap()
                } else {
                    return_error(requester_id.clone().unwrap(), channel_id.clone().unwrap(), e.to_string()).await.unwrap()
                }
                
            },
        };
    let response = &response_choices.choices[0].message.content;
    let response_text = match response.as_ref()
        {
            Some(t) => t,
            None => {
                if msg.is_some() {
                    return_error_reply(msg.clone().unwrap(), "Unable to process stop typing".to_owned()).await.unwrap()
                } else {
                    return_error(requester_id.clone().unwrap(), channel_id.clone().unwrap(), "Unable to process stop typing".to_owned()).await.unwrap()
                }
            },
            
        };
    let mut return_vec: Vec<String> = Vec::new();
    
    /*
        If the response is longer than a discord message will allow, split the message
        This splits the message into slightly shorter chunks than 2000 to allow for adding what part of the message it is and DEBUG: for debug mode
    */
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