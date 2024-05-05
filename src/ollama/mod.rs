use std::collections::HashMap;
use std::str;
use std::time::Duration;

use anyhow::{bail, Result};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::StatusCode;
use serde_derive::Deserialize;

const SPINNER: &[&str] = &["▹▹▹▹▹", "▸▹▹▹▹", "▹▸▹▹▹", "▹▹▸▹▹", "▹▹▹▸▹", "▹▹▹▹▸", "✔"];

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
    pub digest: Option<String>,
    pub total: Option<usize>,
    pub completed: Option<usize>,
}

pub async fn del_model(
    name: String,
    avail_models: Vec<String>,
    conf: &lib::RTwoConfig,
) -> Result<()> {
    let del_msg = format!("Attempting to delete model \"{}\"", &name);
    lib::fmt_print(&del_msg, lib::ContentType::Exit, conf);
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
    let client = reqwest::Client::builder().build()?;
    let resp = client.delete(full_url).body(body).send().await?;
    if resp.status() != StatusCode::OK {
        bail!("Server error deleting model");
    }
    Ok(())
}

pub async fn pull_model(
    name: String,
    avail_models: Vec<String>,
    conf: &lib::RTwoConfig,
) -> Result<()> {
    let msg = format!(
        "Attempting to pull model \"{}\" to {}:{}",
        &name, conf.host, conf.port
    );
    lib::log(lib::LogLevel::Debug, "ollama", &msg)?;
    if avail_models.contains(&name) {
        lib::fmt_print("Model already exists on server", lib::ContentType::Exit, conf);
        return Ok(());
    }
    let full_url = format!("http://{}:{}/api/pull", conf.host, conf.port);
    let mut payload: HashMap<String, String> = HashMap::new();
    payload.insert("name".to_string(), name.clone());
    let body = get_postdata(payload);
    let client = reqwest::Client::builder().build()?;
    let mut resp = client.post(full_url).body(body).send().await?;
    let pb = start_spinner(conf.color);
    let mut layers: Vec<String> = vec![];
    let mut layer_num = 0;
    let dl_msg = format!("Downloading \"{}\"", &name);
    lib::fmt_print(&dl_msg, lib::ContentType::Exit, conf);
    pb.set_message(dl_msg);
    while let Some(chunk) = resp.chunk().await? {
        let raw_resp: &str = str::from_utf8(&chunk)?;
        let o_resp: PullResponse = serde_json::from_str(raw_resp)?;
        if let Some(e) = o_resp.error {
            pb.finish_with_message("Error");
            bail!(e);
        }
        if let Some(status) = o_resp.status {
            if layers.contains(&status) {
                continue;
            }
            match o_resp.digest {
                Some(_) => {
                    layers.push(status.clone());
                    layer_num += 1;
                    let msg = format!("Downloading: Layer {}", layer_num);
                    pb.set_message(msg);
                    continue;
                }
                None => pb.set_message(status.clone()),
            }
            if &status == "success" {
                pb.finish_with_message("Done");
            }
        }
    }
    Ok(())
}

pub async fn generate(
    question: String,
    context: Option<String>,
    conf: &lib::RTwoConfig,
) -> Result<(String, String)> {
    let msg = format!(
        "Attempting to generate response from {}:{}",
        conf.host, conf.port
    );
    lib::log(lib::LogLevel::Debug, "ollama", &msg)?;
    let full_url = format!("http://{}:{}/api/generate", conf.host, conf.port);
    let mut payload: HashMap<String, String> = HashMap::new();
    payload.insert("model".to_string(), conf.model.to_string());
    payload.insert("prompt".to_string(), question);
    if !conf.stream {
        payload.insert("stream".to_string(), "false".to_string());
    }
    if let Some(ctx) = context {
        payload.insert("context".to_string(), ctx);
    }
    let body = get_postdata(payload);
    let client = reqwest::Client::builder().build()?;
    let mut context = "[]".to_string();
    let mut full_response = String::new();
    if conf.stream {
        let mut raw_resp = String::new();
        let mut resp = client.post(full_url).body(body).send().await?;
        while let Some(chunk) = resp.chunk().await? {
            // tmp variable used because large responses (final resp) may sometimes be split amongst multiple chunks
            let tmp: &str = str::from_utf8(&chunk)?.trim_end();
            raw_resp.push_str(tmp);
            if !raw_resp.ends_with('}') {
                continue;
            }
            let ollama_resp: GenerateResponse = serde_json::from_str(&raw_resp)?;
            raw_resp = String::new();
            let (ctx_opt, response) = process_gen_response(ollama_resp, conf)?;
            full_response.push_str(&response);
            if let Some(ctx) = ctx_opt {
                context = ctx;
            }
        }
    } else {
        let pb = start_spinner(conf.color);
        pb.set_message("Processing");
        let resp = client.post(full_url).body(body).send().await?;
        let ollama_resp: GenerateResponse = serde_json::from_str(&resp.text().await?)?;
        pb.finish_and_clear();
        let (ctx_opt, response) = process_gen_response(ollama_resp, conf)?;
        full_response.push_str(&response);
        if let Some(ctx) = ctx_opt {
            context = ctx;
        }
    }
    Ok((context, full_response))
}

pub async fn get_models(conf: &lib::RTwoConfig) -> Result<Vec<String>> {
    let msg = format!(
        "Attempting to get available models from {}:{}",
        conf.host, conf.port
    );
    lib::log(lib::LogLevel::Debug, "ollama", &msg)?;
    let full_url = format!("http://{}:{}/api/tags", conf.host, conf.port);
    let resp: ModelResponse = reqwest::get(full_url).await?.json().await?;
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

fn process_gen_response(
    ollama_resp: GenerateResponse,
    conf: &lib::RTwoConfig,
) -> Result<(Option<String>, String)> {
    if let Some(err) = ollama_resp.error {
        bail!(err);
    }
    let mut context: Option<String> = None;
    let response = match ollama_resp.response {
        Some(s) => {
            lib::fmt_print(&s, lib::ContentType::Answer, conf);
            s
        }
        None => bail!("Response not found"),
    };
    let done = ollama_resp.done.unwrap_or(false);
    if done {
        if let Some(ctx) = ollama_resp.context {
            context = Some(format!("{:?}", ctx));
        }
        let model = ollama_resp.model.unwrap_or("Unknown".to_string());
        let prompt_eval_count = ollama_resp.prompt_eval_count.unwrap_or(0);
        let eval_count = ollama_resp.eval_count.unwrap_or(0);
        let total_duration: f64 = ollama_resp.total_duration.unwrap_or(0) as f64 / 1000000000.0;
        let msg = format!(
            "Response generated from {}:{} -> [\"{}\",{},{},{}]",
            conf.host, conf.port, model, prompt_eval_count, eval_count, total_duration
        );
        lib::log(lib::LogLevel::Debug, "ollama", &msg)?;
        lib::fmt_print("\nDone", lib::ContentType::Info, conf);
        if conf.verbose {
            let x = format!("* Model: {}\n* Tokens in prompt: {}\n* Tokens in response: {}\n* Time taken: {:.3}s", model, prompt_eval_count, eval_count, total_duration);
            lib::fmt_print(&x, lib::ContentType::Info, conf);
        }
    }
    Ok((context, response))
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
