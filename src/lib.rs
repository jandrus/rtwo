use std::fs::{create_dir_all, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use anyhow::{anyhow, ensure, Result};
use bat::PrettyPrinter;
use chrono::Local;
use clap::ArgMatches;
use colored::Colorize;
use directories::ProjectDirs;
use serde_derive::{Deserialize, Serialize};
use toml::to_string;

struct Project<T: AsRef<str>> {
    qualifier: T,
    org: T,
    app: T,
}

#[derive(Serialize, Deserialize)]
pub struct RTwoConfig {
    pub host: String,
    pub port: u16,
    pub model: String,
    pub verbose: bool,
    pub color: bool,
    pub save: bool,
    pub stream: bool,
}

impl Default for RTwoConfig {
    fn default() -> Self {
        RTwoConfig {
            host: "localhost".to_owned(),
            port: 11434,
            model: "llama3".to_owned(),
            verbose: false,
            color: true,
            save: true,
            stream: false,
        }
    }
}

pub enum ContentType {
    Error,
    Info,
    Answer,
    Exit,
}

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
    let now = format!("{:?}", Local::now());
    let log_msg = match lvl {
        LogLevel::Debug => {
            format!("{} DEBUG [{}]: {}\n", now, descriptor, msg)
        }
        LogLevel::Error => {
            format!("{} ERROR [{}]: {}\n", now, descriptor, msg)
        }
        LogLevel::Info => {
            format!("{} INFO [{}]: {}\n", now, descriptor, msg)
        }
    };
    let log_file = get_project_file(ProjFiles::Log)?;
    let mut f;
    if Path::new(&log_file).exists() {
        f = OpenOptions::new().append(true).open(log_file)?;
    } else {
        f = File::create(log_file)?;
    }
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
            let mut file = File::create(format!(
                "{}/{}",
                proj.config_dir().to_str().unwrap(),
                CONF_FILE
            ))?;
            file.write_all(to_string(&RTwoConfig::default())?.as_bytes())?;
            println!(
                "Default configuration file created at {}/{}. Please edit.",
                proj.config_dir().to_str().unwrap(),
                CONF_FILE
            );
        }
        return Ok(());
    }
    Err(anyhow!("Could not create project directory"))
}

pub fn get_config(matches: ArgMatches) -> Result<RTwoConfig> {
    let toml_string = read_file(&get_project_file(ProjFiles::Conf)?)?;
    let mut conf: RTwoConfig = toml::from_str(&toml_string)?;
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
    if matches.get_flag("no_color") {
        conf.color = false;
    }
    if matches.get_flag("color") {
        conf.color = true;
    }
    if matches.get_flag("no_save") {
        conf.save = false;
    }
    if matches.get_flag("save") {
        conf.save = true;
    }
    if matches.get_flag("stream") {
        conf.stream = true;
    }
    if matches.get_flag("batch") {
        conf.stream = false;
    }
    ensure!(conf.port < 65535, "Port out of bounds");
    let msg = format!(
        "Ollama host {}:{} with model \"{}\"",
        &conf.host, &conf.port, &conf.model
    );
    log(LogLevel::Info, "conf", &msg)?;
    Ok(conf)
}

pub fn fmt_print(s: &str, content_type: ContentType, conf: &RTwoConfig) {
    if conf.color {
        match content_type {
            ContentType::Error => println!("{}", s.red().bold()),
            ContentType::Info => println!("{}", s.yellow().italic()),
            ContentType::Answer => {
                if conf.stream {
                    print!("{}", s.magenta());
                } else {
                    PrettyPrinter::new()
                        .input_from_bytes(s.as_bytes())
                        .grid(true)
                        .language("markdown")
                        .theme("DarkNeon")
                        .print()
                        .unwrap();
                }
            }
            ContentType::Exit => println!("{}", s.green().bold()),
        }
    } else {
        match content_type {
            ContentType::Answer => print!("{}", s),
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
    match f.read_to_string(&mut s) {
        Ok(_) => Ok(s),
        Err(e) => Err(anyhow!(e.to_string())),
    }
}
