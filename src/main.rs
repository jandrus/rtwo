use std::process;

use clap::{Arg, ArgMatches, Command};

mod db;
mod ollama;

fn main() {
    // Setup Directories -> config, data
    if let Err(e) = lib::setup_file_struct() {
        eprintln!("Error setting up file structure: {}", e);
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
            eprintln!("{}", err_msg);
            process::exit(1);
        }
    };
    // Is ollama server in config/args up?
    if ollama::valid_server(&conf).is_err() {
        kill("Invalid server".to_owned(), "ollama", conf.color);
    }
    // Models on ollama host
    let avail_models: Vec<String> = match ollama::get_models(&conf) {
        Ok(m) => m,
        Err(e) => {
            let err_msg = format!(
                "Failed to get available models from {}:{} -> {}",
                conf.host, conf.port, e
            );
            kill(err_msg, "ollama", conf.color);
        }
    };
    if matches.get_flag("list_models") {
        lib::fmt_print(
            &format!("Available models: {:?}", avail_models),
            lib::ContentType::Info,
            conf.color,
        );
        lib::fmt_print(
            &format!("Selected model: \"{}\"", &conf.model),
            lib::ContentType::Info,
            conf.color,
        );
        process::exit(0);
    }
    // Pull provided model to ollama host
    if matches.value_source("pull").is_some() {
        let model = matches.get_one::<String>("pull").unwrap().to_string();
        match ollama::pull_model(model.clone(), avail_models, &conf) {
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
                kill(err_msg, "ollama", conf.color);
            }
        }
    }
    // Delete provided model on ollama host
    if matches.value_source("del").is_some() {
        let model = matches.get_one::<String>("del").unwrap().to_string();
        match ollama::del_model(model.clone(), avail_models, &conf) {
            Ok(_) => {
                let msg = format!(
                    "Model \"{}\" deleted from {}:{}",
                    &model, conf.host, conf.port
                );
                lib::log(lib::LogLevel::Info, "ollama", &msg).unwrap();
                lib::fmt_print(&msg, lib::ContentType::Exit, conf.color);
                process::exit(0);
            }
            Err(e) => {
                let err_msg = format!(
                    "Failed to delete model \"{}\" from {}:{} -> {}",
                    &model, conf.host, conf.port, e
                );
                kill(err_msg, "ollama", conf.color);
            }
        }
    }
    // Ensure model in config is on ollama host
    if !avail_models.iter().any(|m| m.contains(&conf.model)) {
        let err_msg = format!(
            "Model \"{}\" not available.\nAvailable models for {} include: {:?}",
            &conf.model, &conf.host, avail_models
        );
        kill(err_msg, "ollama", conf.color);
    }
    // List saved conversations
    if matches.get_flag("list") && db::list_conversations(conf.color).is_err() {
        kill("Failed to list conversations".to_owned(), "db", conf.color);
    }
    // Delete saved conversations
    if matches.get_flag("del_convo") && db::delete_conversations(conf.color).is_err() {
        kill("Failed to delete conversation".to_owned(), "db", conf.color);
    }
    let mut conversation: Vec<db::Chat> = vec![];
    let mut context: Option<String> = None;
    // Restore conversation
    if matches.get_flag("restore") {
        (context, conversation) = match db::restore_conversation(conf.color) {
            Ok((ctx, convo)) => (ctx, convo),
            Err(e) => {
                let err_msg = format!("Failed to restore conversation -> {}", e);
                kill(err_msg, "db", conf.color);
            }
        }
    }
    // Main loop (Q&A)
    loop {
        let prompt: String = match lib::get_input("Ask R2", None, conf.color) {
            Ok(s) => s,
            Err(_) => {
                kill("Failed to get user input".to_owned(), "main", conf.color);
            }
        };
        conversation.push(db::Chat {
            role: "user".to_string(),
            content: prompt.clone(),
        });
        context = match ollama::gen(prompt.replace('\"', "'"), context, &conf) {
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
                kill(err_msg, "ollama", conf.color);
            }
        };
        let ask_again = match lib::get_confirm("Ask another question?", None, conf.color) {
            Ok(b) => b,
            Err(_) => {
                kill(
                    "Failed to get user confirmation".to_owned(),
                    "main",
                    conf.color,
                );
            }
        };
        if !ask_again {
            break;
        }
    }
    if conf.save || lib::get_confirm("Save conversation?", None, conf.color).unwrap() {
        if let Err(e) = db::save_conversation(conversation, context, &conf) {
            let err_msg = format!(
                "\nFailed to save conversation {}:{} -> {}",
                conf.host, conf.port, e
            );
            kill(err_msg, "db", conf.color);
        }
    }
    lib::fmt_print("Goodbye", lib::ContentType::Exit, conf.color);
}

fn kill(msg: String, descriptor: &str, color: bool) -> ! {
    lib::log(lib::LogLevel::Error, descriptor, &msg).unwrap();
    lib::fmt_print(&msg, lib::ContentType::Error, color);
    process::exit(1)
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
                .long_help("Host address for ollama server. e.g.: localhost, 192.168.1.5, etc.")
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
                .long_help("Host port for ollama server. e.g.: 11434, 1776, etc.")
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
                .long_help("Model name to query. e.g.: mistral, llama3:70b, etc.\nNOTE: If model is not available on HOST, rtwo will not automatically download the model to the HOST. Use \"pull\" [-P, --pull] to download the model to the HOST.")
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
                .long_help("Enable verbose output. Prints: model, tokens in prompt, tokens in response, and time taken after response is rendered to user.\nExample: \n\t* Model: llama3:70b\n\t* Tokens in prompt: 23\n\t* Tokens in response: 216\n\t* Time taken: 27.174")
                .required(false)
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("color")
                .short('c')
                .long("color")
                .help("Enable color output")
                .long_help("Enable color output.")
                .required(false)
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("save")
                .short('s')
                .long("save")
                .help("Save conversation for recall (places conversation in DB)")
                .long_help("Save conversation for recall (places conversation in DB)")
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
                .long("listmodels")
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
                .long("delmodel")
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
