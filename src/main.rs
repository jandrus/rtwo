use std::process;

use clap::{Arg, ArgMatches, Command};
use dialoguer::{theme::ColorfulTheme, Confirm, Input};

mod db;
mod ollama;

#[tokio::main]
async fn main() {
    // Setup Directories -> config, data
    if let Err(e) = lib::setup_file_struct() {
        println!("{}", e);
        process::exit(1);
    }
    // Args
    let matches = get_matches();
    // Config
    let conf = match lib::get_config(matches.clone()) {
        Ok(c) => c,
        Err(e) => {
            let err_msg = format!("Failed to read config from file or args -> {}", e);
            lib::log(lib::LogLevel::Error, "main", &err_msg).unwrap();
            println!("{}", err_msg);
            process::exit(1);
        }
    };
    // Models on ollama host
    let avail_models: Vec<String> = match ollama::get_models(&conf).await {
        Ok(m) => m,
        Err(e) => {
            let err_msg = format!(
                "Failed to get available models from {}:{} -> {}",
                conf.host, conf.port, e
            );
            lib::log(lib::LogLevel::Error, "ollama", &err_msg).unwrap();
            lib::fmt_print(&err_msg, lib::ContentType::Error, &conf);
            process::exit(1);
        }
    };
    if matches.get_flag("list_models") {
        lib::fmt_print(
            &format!("Available models: {:?}", avail_models),
            lib::ContentType::Info,
            &conf,
        );
        lib::fmt_print(
            &format!("Selected model: \"{}\"", &conf.model),
            lib::ContentType::Info,
            &conf,
        );
    }
    // Ensure model in config is on ollama host
    if !avail_models.iter().any(|m| m.contains(&conf.model)) {
        let err_str = format!(
            "Model \"{}\" not available.\nAvailable models for {} include: {:?}",
            &conf.model, &conf.host, avail_models
        );
        lib::fmt_print(&err_str, lib::ContentType::Error, &conf);
        let err_msg = format!(
            "Provided model not available at {}:{}",
            conf.host, conf.port
        );
        lib::log(lib::LogLevel::Error, "ollama", &err_msg).unwrap();
        process::exit(1);
    }
    // Pull provided model to ollama host
    if matches.value_source("pull").is_some() {
        let model = matches.get_one::<String>("pull").unwrap().to_string();
        match ollama::pull_model(model.clone(), avail_models, &conf).await {
            Ok(_) => {
                let msg = format!("Model \"{}\" pulled to {}:{}", &model, conf.host, conf.port);
                lib::log(lib::LogLevel::Info, "ollama", &msg).unwrap();
                process::exit(0);
            }
            Err(e) => {
                let err_msg = format!(
                    "Failed to pull model \"{}\" to {}:{} -> {}",
                    &model, conf.host, conf.port, e
                );
                lib::log(lib::LogLevel::Error, "ollama", &err_msg).unwrap();
                lib::fmt_print(&err_msg, lib::ContentType::Error, &conf);
                process::exit(1);
            }
        }
    }
    // Delete provided model on ollama host
    if matches.value_source("del").is_some() {
        let model = matches.get_one::<String>("del").unwrap().to_string();
        match ollama::del_model(model.clone(), avail_models, &conf).await {
            Ok(_) => {
                let msg = format!(
                    "Model \"{}\" deleted from {}:{}",
                    &model, conf.host, conf.port
                );
                lib::log(lib::LogLevel::Info, "ollama", &msg).unwrap();
                lib::fmt_print(&msg, lib::ContentType::Exit, &conf);
                process::exit(0);
            }
            Err(e) => {
                let err_msg = format!(
                    "Failed to delete model \"{}\" from {}:{} -> {}",
                    &model, conf.host, conf.port, e
                );
                lib::log(lib::LogLevel::Error, "ollama", &err_msg).unwrap();
                lib::fmt_print(&err_msg, lib::ContentType::Error, &conf);
                process::exit(1);
            }
        }
    }
    // List saved conversations
    if matches.get_flag("list") {
        if let Err(e) = db::list_conversations(&conf) {
            lib::fmt_print(
                "Failed to list conversations. See log for details",
                lib::ContentType::Error,
                &conf,
            );
            lib::log(lib::LogLevel::Error, "db", &e.to_string()).unwrap();
            process::exit(1);
        }
    }
    // Delete saved conversations
    if matches.get_flag("del_convo") {
        if let Err(e) = db::delete_conversations(&conf) {
            lib::fmt_print(
                "Failed to delete conversations. See log for details",
                lib::ContentType::Error,
                &conf,
            );
            lib::log(lib::LogLevel::Error, "db", &e.to_string()).unwrap();
            process::exit(1);
        }
    }
    let mut conversation: Vec<db::Chat> = vec![];
    let mut context: Option<String> = None;
    // Restore conversation
    if matches.get_flag("restore") {
        (context, conversation) = match db::restore_conversation(&conf) {
            Ok((ctx, convo)) => (ctx, convo),
            Err(e) => {
                let err_msg = format!("Failed to restore conversation -> {}", e);
                lib::log(lib::LogLevel::Error, "main", &err_msg).unwrap();
                lib::fmt_print(&err_msg, lib::ContentType::Error, &conf);
                process::exit(1);
            }
        }
    }
    // Main loop (Q&A)
    loop {
        let question: String = match conf.color {
            true => Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Ask R2")
                .interact_text()
                .unwrap(),
            false => Input::new().with_prompt("Ask R2").interact_text().unwrap(),
        };
        conversation.push(db::Chat {
            role: "user".to_string(),
            content: question.clone(),
        });
        context = match ollama::generate(question.replace('\"', "'"), context, &conf).await {
            Ok((ctx, resp)) => {
                conversation.push(db::Chat {
                    role: "assistant".to_string(),
                    content: resp,
                });
                Some(ctx)
            }
            Err(e) => {
                let err_msg = format!(
                    "Failed to generate response from {}:{} -> {}",
                    conf.host, conf.port, e
                );
                lib::log(lib::LogLevel::Error, "ollama", &err_msg).unwrap();
                lib::fmt_print(&err_msg, lib::ContentType::Error, &conf);
                process::exit(1);
            }
        };
        let ask_again = match conf.color {
            true => Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("Ask another question?")
                .wait_for_newline(true)
                .interact()
                .unwrap(),
            false => Confirm::new()
                .with_prompt("Ask another question?")
                .wait_for_newline(true)
                .interact()
                .unwrap(),
        };
        if !ask_again {
            break;
        }
    }
    if conf.save {
        if let Err(e) = db::save_conversation(conversation, context, &conf) {
            let err_msg = format!(
                "\nFailed to save conversation {}:{} -> {}",
                conf.host, conf.port, e
            );
            lib::log(lib::LogLevel::Error, "db", &err_msg).unwrap();
            lib::fmt_print(&err_msg, lib::ContentType::Error, &conf);
            process::exit(1);
        }
    }
    lib::fmt_print("Goodbye", lib::ContentType::Exit, &conf);
}

fn get_matches() -> ArgMatches {
    Command::new("rtwo")
        .about("rtwo - ollama cli tool in Rust. Used to query and manage ollama server")
        .version("0.1.0")
        .author("ash")
        .arg_required_else_help(false)
        .arg(
            Arg::new("host")
                .short('H')
                .long("host")
                .help("Host address for ollama server")
                .long_help("Host address for ollama server. e.g.: localhost, 192.168.1.5, etc.\nCan be set in configuration file.\nDefault: localhost")
                .value_name("HOST")
                .required(false)
                .action(clap::ArgAction::Set)
                .num_args(1),
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .help("Host port for ollama server")
                .long_help("Host port for ollama server. e.g.: 11434, 1776, etc.\nCan be set in configuration file.\nDefault: 11434")
                .value_name("PORT")
                .required(false)
                .action(clap::ArgAction::Set)
                .num_args(1),
        )
        .arg(
            Arg::new("model")
                .short('m')
                .long("model")
                .help("Model name to query. eg: llama3")
                .long_help("Model name to query. e.g.: mistral, llama3:70b, etc.\nNOTE: If model is not available on HOST, rtwo will not automatically download the model to the HOST. Use \"pull\" [-P, --pull] to download the model to the HOST.\nCan be set in configuration file.\nDefault: llama3")
                .value_name("MODEL")
                .required(false)
                .action(clap::ArgAction::Set)
                .num_args(1),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Enable verbose output")
                .long_help("Enable verbose output. Prints: model, tokens in prompt, tokens in response, and time taken after response is rendered to user.\nExample: \n\t* Model: llama3:70b\n\t* Tokens in prompt: 23\n\t* Tokens in response: 216\n\t* Time taken: 27.174\nCan be set in configuration file.\nDefault: false")
                .required(false)
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("color")
                .short('c')
                .long("color")
                .conflicts_with("no_color")
                .help("Enable color output")
                .long_help("Enable color output.\nCan be set in configuration file.\nDefault: true")
                .required(false)
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("no_color")
                .short('n')
                .long("nocolor")
                .conflicts_with("color")
                .help("Disable color output")
                .long_help("Disable color output.\nCan be set in configuration file [color = false].")
                .required(false)
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("save")
                .short('s')
                .long("save")
                .conflicts_with("no_save")
                .help("Save conversation for recall (places conversation in DB)")
                .long_help("Save conversation for recall (places conversation in DB)\nCan be set in configuration file.\nDefault: true")
                .required(false)
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("no_save")
                .short('i')
                .long("incogneto")
                .conflicts_with("save")
                .help("Do NOT save conversation for recall.")
                .long_help("Do NOT save conversation for recall.\nCan be set in configuration file [save = false].")
                .required(false)
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("batch")
                .short('b')
                .long("batch")
                .conflicts_with("stream")
                .help("Do not stream llm output (Enables response formatting)")
                .long_help("Do not stream llm output (wait for full response to generate before rendering to user).\nNOTE: This allows response formatting.\nCan be set in configuration file [stream = false].")
                .required(false)
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("stream")
                .short('S')
                .long("stream")
                .conflicts_with("batch")
                .help("Stream llm output (Disables response formatting).")
                .long_help("Stream llm output (Display response as it is rendered by host)\nNOTE: This disables response formatting.\nCan be set in configuration file.\nDefault: false")
                .required(false)
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("list")
                .short('l')
                .long("list")
                .conflicts_with("del")
                .conflicts_with("del_convo")
                .conflicts_with("restore")
                .conflicts_with("pull")
                .help("List previous conversations")
                .required(false)
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("list_models")
                .short('L')
                .long("listmodel")
                .help("List available models on ollama server (HOST:PORT)")
                .required(false)
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("restore")
                .short('r')
                .long("restore")
                .conflicts_with("list")
                .conflicts_with("del")
                .conflicts_with("del_convo")
                .conflicts_with("pull")
                .help("Restore previous conversation from local storage (pick up where you left off).")
                .long_help("Select previous conversation from local storage and pick up where you left off. This restores the context from a saved conversation and prints the saved output.\nInteractive")
                .required(false)
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("del_convo")
                .short('d')
                .long("delete")
                .conflicts_with("list")
                .conflicts_with("del")
                .conflicts_with("restore")
                .conflicts_with("pull")
                .help("Delete previous conversations from local storage.")
                .long_help("Delete previous conversations from local storage.\nNOTE: action is irreversible.\nInteractive")
                .required(false)
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("pull")
                .short('P')
                .long("pull")
                .conflicts_with("list")
                .conflicts_with("del")
                .conflicts_with("del_convo")
                .conflicts_with("restore")
                .help("Pull model to ollama server for use")
                .long_help("Pull model to ollama server for use (downloads model on HOST). e.g.: llama3.")
                .value_name("MODEL")
                .required(false)
                .action(clap::ArgAction::Set)
                .num_args(1),
        )
        .arg(
            Arg::new("del")
                .short('D')
                .long("delete-model")
                .conflicts_with("list")
                .conflicts_with("del_convo")
                .conflicts_with("pull")
                .conflicts_with("restore")
                .help("Delete model from ollama server")
                .long_help("Delete model from ollama server (deletes model on HOST). e.g.: llama2.")
                .value_name("MODEL")
                .required(false)
                .action(clap::ArgAction::Set)
                .num_args(1),
        )
        .get_matches()
}
