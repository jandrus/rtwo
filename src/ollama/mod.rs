use std::collections::HashMap;
use std::str;
use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::StatusCode;
use serde_derive::Deserialize;

const SPINNER: &[&str] = &["▹▹▹▹▹", "▸▹▹▹▹", "▹▸▹▹▹", "▹▹▸▹▹", "▹▹▹▸▹", "▹▹▹▹▸", "✔"];
const SPINNER_ERR: &[&str] = &["✘"];

#[derive(Deserialize)]
pub struct GenerateResponse {
    pub error: Option<String>,
    pub model: Option<String>,
    pub created_at: Option<String>,
    pub response: Option<String>,
    pub done: Option<bool>,
    pub context: Option<Vec<i64>>,
    pub total_duration: Option<u64>,
    pub load_duration: Option<u64>,
    pub prompt_eval_count: Option<u64>,
    pub prompt_eval_duration: Option<u64>,
    pub eval_count: Option<u64>,
    pub eval_duration: Option<u64>,
}

#[derive(Deserialize)]
pub struct ModelResponse {
    pub models: Vec<Model>,
}

#[derive(Deserialize)]
pub struct Model {
    pub name: String,
    pub modified_at: String,
    pub size: i64,
    pub digest: String,
    pub details: Details,
}

#[derive(Deserialize)]
pub struct Details {
    pub format: String,
    pub family: String,
    pub families: Option<Vec<String>>,
    pub parameter_size: String,
    pub quantization_level: String,
}

#[derive(Deserialize)]
pub struct PullResponse {
    pub error: Option<String>,
    pub status: Option<String>,
}

pub fn valid_server(conf: &lib::Config) -> Result<()> {
    let full_url = format!("http://{}:{}", conf.host, conf.port);
    let _ = reqwest::blocking::get(full_url)?;
    Ok(())
}

pub fn del_model(name: String, avail_models: Vec<String>, conf: &lib::Config) -> Result<()> {
    let del_msg = format!("Attempting to delete model \"{}\"", &name);
    lib::fmt_print(&del_msg, lib::ContentType::Exit, conf.color);
    if !avail_models.contains(&name) {
        bail!("Model not found");
    }
    let msg = format!(
        "Attempting to delete model \"{}\" from {}:{}",
        &name, conf.host, conf.port
    );
    lib::log(lib::LogLevel::Debug, "ollama", &msg)?;
    let full_url = format!("http://{}:{}/api/delete", conf.host, conf.port);
    let mut payload: HashMap<String, String> = HashMap::new();
    payload.insert("name".to_string(), name.clone());
    let body = get_postdata(payload);
    let client = reqwest::blocking::Client::new();
    let resp = client.delete(full_url).body(body).send()?;
    if resp.status() != StatusCode::OK {
        bail!("Server error deleting model");
    }
    Ok(())
}

pub fn pull_model(name: String, avail_models: Vec<String>, conf: &lib::Config) -> Result<()> {
    let msg = format!(
        "Attempting to pull model \"{}\" to {}:{}",
        &name, conf.host, conf.port
    );
    lib::log(lib::LogLevel::Debug, "ollama", &msg)?;
    if avail_models.contains(&name) {
        lib::fmt_print(
            "Model already exists on server",
            lib::ContentType::Exit,
            conf.color,
        );
        return Ok(());
    }
    let full_url = format!("http://{}:{}/api/pull", conf.host, conf.port);
    let mut payload: HashMap<String, String> = HashMap::new();
    payload.insert("name".to_string(), name.clone());
    payload.insert("stream".to_string(), "false".to_string());
    let body = get_postdata(payload);
    let client = reqwest::blocking::Client::builder().timeout(None).build()?;
    let pb = start_spinner(conf.color);
    pb.set_message(format!("Downloading \"{}\"", &name));
    let resp = client.post(full_url).body(body).send()?;
    let ollama_resp: PullResponse = serde_json::from_str(&resp.text()?)?;
    if let Some(err) = ollama_resp.error {
        finish_spinner_error(pb, conf.color);
        bail!(err);
    }
    if let Some(status) = ollama_resp.status {
        if status == "success" {
            pb.finish_with_message("Done");
            return Ok(());
        }
        pb.finish_with_message("Error");
    }
    Err(anyhow!("Error downloading model"))
}

pub fn gen(prompt: String, ctx: Option<String>, conf: &lib::Config) -> Result<(String, String)> {
    let msg = format!(
        "Attempting to generate response from {}:{}",
        conf.host, conf.port
    );
    lib::log(lib::LogLevel::Debug, "ollama", &msg)?;
    let full_url = format!("http://{}:{}/api/generate", conf.host, conf.port);
    let mut payload: HashMap<String, String> = HashMap::new();
    payload.insert("model".to_string(), conf.model.to_string());
    payload.insert("prompt".to_string(), prompt);
    payload.insert("stream".to_string(), "false".to_string());
    if let Some(context) = ctx {
        payload.insert("context".to_string(), context);
    }
    let body = get_postdata(payload);
    let client = reqwest::blocking::Client::builder().timeout(None).build()?;
    let pb = start_spinner(conf.color);
    pb.set_message("Processing");
    let resp = client.post(full_url).body(body).send()?;
    let ollama_resp: GenerateResponse = serde_json::from_str(&resp.text()?)?;
    if let Some(err) = ollama_resp.error {
        finish_spinner_error(pb, conf.color);
        bail!(err);
    }
    pb.finish_with_message("Done");
    let response = match ollama_resp.response {
        Some(s) => {
            lib::fmt_print(&s, lib::ContentType::Answer, conf.color);
            s
        }
        None => bail!("Response not found"),
    };
    let context = match ollama_resp.context {
        Some(s) => format!("{:?}", s),
        None => bail!("Context not found"),
    };
    if conf.verbose {
        let model = ollama_resp.model.unwrap_or("Unknown".to_string());
        let prompt_eval_count = ollama_resp.prompt_eval_count.unwrap_or(0);
        let eval_count = ollama_resp.eval_count.unwrap_or(0);
        let total_duration: f64 = ollama_resp.total_duration.unwrap_or(0) as f64 / 1000000000.0;
        let msg = format!(
            "Response generated from {}:{} -> [\"{}\",{},{},{}]",
            conf.host, conf.port, model, prompt_eval_count, eval_count, total_duration
        );
        lib::log(lib::LogLevel::Debug, "ollama", &msg)?;
        lib::fmt_print("\nDone", lib::ContentType::Info, conf.color);
        let info = format!(
            "* Model: {}\n* Tokens in prompt: {}\n* Tokens in response: {}\n* Time taken: {:.3}s",
            model, prompt_eval_count, eval_count, total_duration
        );
        lib::fmt_print(&info, lib::ContentType::Info, conf.color);
    }
    Ok((context, response))
}

pub fn get_models(conf: &lib::Config) -> Result<Vec<String>> {
    let msg = format!(
        "Attempting to get available models from {}:{}",
        conf.host, conf.port
    );
    lib::log(lib::LogLevel::Debug, "ollama", &msg)?;
    let full_url = format!("http://{}:{}/api/tags", conf.host, conf.port);
    let resp: ModelResponse = reqwest::blocking::get(full_url)?.json()?;
    let models = resp.models.into_iter().map(|m| m.name).collect();
    let msg = format!(
        "Available models at {}:{} : {:?}",
        conf.host, conf.port, models
    );
    lib::log(lib::LogLevel::Debug, "ollama", &msg)?;
    Ok(models)
}

fn start_spinner(color: bool) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.enable_steady_tick(Duration::from_millis(120));
    if color {
        pb.set_style(
            ProgressStyle::with_template("{msg:.green} {spinner:.blue}")
                .unwrap()
                .tick_strings(SPINNER),
        );
    } else {
        pb.set_style(
            ProgressStyle::with_template("{msg} {spinner}")
                .unwrap()
                .tick_strings(SPINNER),
        );
    }
    pb
}

fn finish_spinner_error(pb: ProgressBar, color: bool) {
    if color {
        pb.set_style(
            ProgressStyle::with_template("{msg:.red} {spinner:.red}")
                .unwrap()
                .tick_strings(SPINNER_ERR),
        );
    } else {
        pb.set_style(
            ProgressStyle::with_template("{msg} {spinner}")
                .unwrap()
                .tick_strings(SPINNER_ERR),
        );
    }
    pb.finish_with_message("Error");
}

fn get_postdata(hm: HashMap<String, String>) -> String {
    let mut output = String::new();
    output.push('{');
    for (k, v) in hm.iter() {
        if k == "context" || k == "stream" {
            let segment = format!("\"{}\":{},", k, v);
            output.push_str(&segment);
        } else {
            let segment = format!("\"{}\":\"{}\",", k, v);
            output.push_str(&segment);
        }
    }
    output.pop();
    output.push('}');
    output
}
