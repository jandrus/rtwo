use std::fs::{create_dir_all, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use anyhow::{anyhow, ensure, Result};
use bat::PrettyPrinter;
use chrono::Local;
use clap::ArgMatches;
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Confirm, Input};
use directories::ProjectDirs;
use serde_derive::{Deserialize, Serialize};
use toml::to_string;

struct Project<T: AsRef<str>> {
    qualifier: T,
    org: T,
    app: T,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub host: String,  // Server Addr
    pub port: u16,     // Server port
    pub model: String, // Model name
    pub verbose: bool, // Verbose output following response
    pub color: bool,   // Color output
    pub save: bool,    // Autosave conversation
}

pub enum ContentType {
    Error,
    Info,
    Answer,
    Exit,
}

#[derive(Debug)]
pub enum LogLevel {
    Debug,
    Error,
    Info,
}

pub enum ProjFiles {
    Conf,
    Data,
    Log,
}

const PROJECT: Project<&'static str> = Project {
    qualifier: "io",
    org: "rtwo",
    app: "rtwo",
};

const LOG_FILE: &str = "rtwo.log";
const CONF_FILE: &str = "rtwo.toml";
const DB_FILE: &str = "rtwo.db";

pub fn log(lvl: LogLevel, descriptor: &str, msg: &str) -> Result<()> {
    let log_msg = format!("{:?} {:?} [{}]: {}\n", Local::now(), lvl, descriptor, msg);
    let log_file = get_project_file(ProjFiles::Log)?;
    let mut f = match Path::new(&log_file).exists() {
        true => OpenOptions::new().append(true).open(log_file)?,
        false => File::create(log_file)?,
    };
    f.write_all(log_msg.as_bytes())?;
    Ok(())
}

pub fn setup_file_struct() -> Result<()> {
    if let Some(proj) = ProjectDirs::from(PROJECT.qualifier, PROJECT.org, PROJECT.app) {
        if !proj.data_dir().exists() {
            create_dir_all(proj.data_dir())?;
        }
        if !proj.config_dir().exists() {
            create_dir_all(proj.config_dir())?;
        }
        let conf_file = format!("{}/{}", proj.config_dir().to_str().unwrap(), CONF_FILE);
        if !Path::new(&conf_file).exists() {
            println!("Configuration not detected: initiating config setup");
            let color = get_confirm("Enable color", Some(true), false)?;
            let mut host: String;
            let mut port: u16;
            loop {
                host = get_input(
                    "Enter Ollama server address",
                    Some("localhost".to_owned()),
                    color,
                )?;
                port = match color {
                    true => Input::with_theme(&ColorfulTheme::default())
                        .with_prompt("Enter Ollama server port")
                        .default("11434".to_owned())
                        .validate_with(|input: &String| -> Result<(), String> {
                            validate_port_str(input)
                        })
                        .report(true)
                        .interact_text()?,
                    false => Input::new()
                        .with_prompt("Enter Ollama server port")
                        .default("11434".to_owned())
                        .validate_with(|input: &String| -> Result<(), String> {
                            validate_port_str(input)
                        })
                        .report(true)
                        .interact_text()?,
                }
                .parse::<u16>()?;
                let url = format!("http://{}:{}", host, port);
                if reqwest::blocking::get(&url).is_ok() {
                    break;
                }
                let msg = format!("Ollama server not found at {}", url);
                fmt_print(&msg, ContentType::Error, color);
            }
            let model = get_input("Enter model", Some("llama3:latest".to_owned()), color)?;
            let verbose = get_confirm("Enable verbose output", Some(true), color)?;
            let save = get_confirm("Enable autosave on exit", Some(true), color)?;
            let conf = Config {
                host,
                port,
                model,
                verbose,
                color,
                save,
            };
            let mut file = File::create(conf_file)?;
            file.write_all(to_string(&conf)?.as_bytes())?;
            fmt_print(
                "NOTE: Params can be changed in config file.",
                ContentType::Info,
                color,
            );
        }
        return Ok(());
    }
    Err(anyhow!("Could not create project directory"))
}

pub fn get_input(prompt: &str, default_opt: Option<String>, color: bool) -> Result<String> {
    let (default, show_default) = match default_opt {
        Some(s) => (s, true),
        None => (String::new(), false),
    };
    let user_input: String = match color {
        true => Input::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .default(default)
            .show_default(show_default)
            .report(true)
            .interact_text()?,
        false => Input::new()
            .with_prompt(prompt)
            .default(default)
            .show_default(show_default)
            .report(true)
            .interact_text()?,
    };
    Ok(user_input)
}

pub fn get_confirm(prompt: &str, default_opt: Option<bool>, color: bool) -> Result<bool> {
    let (default, show_default) = match default_opt {
        Some(b) => (b, true),
        None => (false, false),
    };
    let ans = match color {
        true => Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .default(default)
            .show_default(show_default)
            .wait_for_newline(true)
            .interact()?,
        false => Confirm::new()
            .with_prompt(prompt)
            .default(default)
            .show_default(show_default)
            .wait_for_newline(true)
            .interact()?,
    };
    Ok(ans)
}

pub fn get_config(matches: ArgMatches) -> Result<Config> {
    let toml_string = read_file(&get_project_file(ProjFiles::Conf)?)?;
    let mut conf: Config = toml::from_str(&toml_string)?;
    if matches.value_source("host").is_some() {
        conf.host = matches.get_one::<String>("host").unwrap().to_string();
    }
    if matches.value_source("port").is_some() {
        conf.port = matches
            .get_one::<String>("port")
            .unwrap()
            .to_string()
            .parse::<u16>()?;
    }
    if matches.value_source("model").is_some() {
        conf.model = matches.get_one::<String>("model").unwrap().to_string();
    }
    if matches.get_flag("verbose") {
        conf.verbose = true;
    }
    if matches.get_flag("color") {
        conf.color = true;
    }
    if matches.get_flag("save") {
        conf.save = true;
    }
    ensure!(conf.port < 65535, "Port out of bounds");
    let msg = format!(
        "Ollama host {}:{} with model \"{}\"",
        &conf.host, &conf.port, &conf.model
    );
    log(LogLevel::Info, "conf", &msg)?;
    Ok(conf)
}

pub fn fmt_print(s: &str, content_type: ContentType, color: bool) {
    if color {
        match content_type {
            ContentType::Error => eprintln!("{}", s.red()),
            ContentType::Info => println!("{}", s.yellow().italic()),
            ContentType::Answer => {
                PrettyPrinter::new()
                    .input_from_bytes(s.as_bytes())
                    .grid(true)
                    .language("markdown")
                    .theme("DarkNeon")
                    .print()
                    .unwrap();
            }
            ContentType::Exit => println!("{}", s.green()),
        }
    } else {
        match content_type {
            ContentType::Error => eprintln!("{}", s),
            _ => println!("{}", s),
        }
    }
}

pub fn get_project_file(file: ProjFiles) -> Result<String> {
    if let Some(proj) = ProjectDirs::from(PROJECT.qualifier, PROJECT.org, PROJECT.app) {
        match file {
            ProjFiles::Conf => {
                return Ok(format!(
                    "{}/{}",
                    proj.config_dir().to_str().unwrap(),
                    CONF_FILE
                ));
            }
            ProjFiles::Log => {
                return Ok(format!(
                    "{}/{}",
                    proj.data_dir().to_str().unwrap(),
                    LOG_FILE
                ));
            }
            ProjFiles::Data => {
                return Ok(format!("{}/{}", proj.data_dir().to_str().unwrap(), DB_FILE));
            }
        }
    }
    Err(anyhow!("Could not get project file"))
}

fn read_file(path: &str) -> Result<String> {
    let mut s = String::new();
    let mut f = File::open(path)?;
    f.read_to_string(&mut s)?;
    Ok(s)
}

fn validate_port_str(port_str: &str) -> Result<(), String> {
    if port_str.parse::<u16>().is_err() {
        return Err("Invalid port".to_owned());
    }
    Ok(())
}
