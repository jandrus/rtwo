use anyhow::{bail, Result};
use chrono::{DateTime, Local};
use dialoguer::{theme::ColorfulTheme, Confirm, MultiSelect, Select};
use rusqlite::Connection;
use serde_derive::{Deserialize, Serialize};

#[derive(Clone)]
struct DBEntry {
    timestamp: u64,
    host: String,
    model: String,
    conversation: Vec<Chat>,
    context: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Chat {
    pub role: String,
    pub content: String,
}

const DB_CREATE_STMT: &str = "CREATE TABLE IF NOT EXISTS Conversations (timestamp INTEGER, host TEXT, model TEXT, conversation TEXT, context TEXT)";
const DB_INSERT_STMT: &str = "INSERT INTO Conversations (timestamp, host, model, conversation, context) VALUES (?1, ?2, ?3, ?4, ?5)";
const DB_SELECT_STMT: &str =
    "SELECT timestamp, host, model, conversation, context FROM Conversations";
const DB_DELETE_STMT: &str = "DELETE FROM Conversations WHERE timestamp=(?1)";

pub fn save_conversation(
    conversation: Vec<Chat>,
    context: Option<String>,
    conf: &lib::RTwoConfig,
) -> Result<()> {
    if conversation.is_empty() {
        return Ok(());
    }
    let con = Connection::open(lib::get_project_file(lib::ProjFiles::Data)?)?;
    con.execute(DB_CREATE_STMT, ())?;
    let convo = serde_json::to_string(&conversation)?;
    let ctx = match context {
        Some(c) => format!("{:?}", c),
        None => "[]".to_string(),
    };
    let now = Local::now().timestamp_millis();
    let host = format!("{}:{}", conf.host, conf.port);
    con.execute(DB_INSERT_STMT, (now, host, conf.model.clone(), convo, ctx))?;
    lib::log(lib::LogLevel::Debug, "db", "Conversation saved to DB")?;
    Ok(())
}

pub fn restore_conversation(conf: &lib::RTwoConfig) -> Result<(Option<String>, Vec<Chat>)> {
    let (entries, conversations) = get_conversation_entries()?;
    let idx = match conf.color {
        true => Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Choose conversation to restore")
            .items(&conversations[..])
            .report(false)
            .interact()?,
        false => Select::new()
            .with_prompt("Choose conversation to restore")
            .items(&conversations[..])
            .report(false)
            .interact()?,
    };
    lib::fmt_print("* Restoring conversation *", lib::ContentType::Info, conf);
    for chat in &entries[idx].conversation {
        match chat.role.as_str() {
            "user" => {
                let content = format!("\n{}\n", &chat.content);
                lib::fmt_print(&content, lib::ContentType::Exit, conf)
            }
            "assistant" => lib::fmt_print(&chat.content, lib::ContentType::Answer, conf),
            _ => lib::fmt_print(&chat.content, lib::ContentType::Info, conf),
        }
    }
    println!("\n");
    Ok((
        Some(entries[idx].context.clone().replace('\"', "")),
        entries[idx].conversation.clone(),
    ))
}

pub fn delete_conversations(conf: &lib::RTwoConfig) -> Result<()> {
    let (entries, conversations) = get_conversation_entries()?;
    let idxs = match conf.color {
        true => MultiSelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Choose conversations to delete (spacebar to select/deselect)")
            .items(&conversations[..])
            .report(false)
            .interact()?,
        false => MultiSelect::new()
            .with_prompt("Choose conversations to delete (spacebar to select/deselect)")
            .items(&conversations[..])
            .report(false)
            .interact()?,
    };
    if idxs.is_empty() {
        return Ok(());
    }
    lib::fmt_print(
        "DELETE (action is irreversible):",
        lib::ContentType::Error,
        conf,
    );
    for i in idxs.iter() {
        lib::fmt_print(&conversations[*i], lib::ContentType::Info, conf);
    }
    let confirm = match conf.color {
        true => Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Confirm delete conversations")
            .wait_for_newline(true)
            .interact()
            .unwrap(),
        false => Confirm::new()
            .with_prompt("Confirm delete conversations")
            .wait_for_newline(true)
            .interact()
            .unwrap(),
    };
    if !confirm {
        return Ok(());
    }
    let con = Connection::open(lib::get_project_file(lib::ProjFiles::Data)?)?;
    for i in idxs.into_iter() {
        con.execute(DB_DELETE_STMT, [entries[i].timestamp])?;
    }
    lib::fmt_print("Conversations DELETED", lib::ContentType::Exit, conf);
    lib::log(lib::LogLevel::Info, "db", "Conversations DELETED").unwrap();
    Ok(())
}

pub fn list_conversations(conf: &lib::RTwoConfig) -> Result<()> {
    let (_, conversations) = get_conversation_entries()?;
    lib::fmt_print("Previous conversations:", lib::ContentType::Exit, conf);
    for conversation in conversations.iter() {
        lib::fmt_print(conversation, lib::ContentType::Info, conf);
    }
    Ok(())
}

fn get_conversation_entries() -> Result<(Vec<DBEntry>, Vec<String>)> {
    let con = Connection::open(lib::get_project_file(lib::ProjFiles::Data)?)?;
    let mut stmt = match con.prepare(DB_SELECT_STMT) {
        Ok(st) => st,
        Err(_) => bail!("No responses saved"),
    };
    let rows = stmt.query_map([], |row| {
        let convo_str: String = row.get(3)?;
        let conversation: Vec<Chat> = serde_json::from_str(&convo_str).unwrap();
        Ok(DBEntry {
            timestamp: row.get(0)?,
            host: row.get(1)?,
            model: row.get(2)?,
            conversation,
            context: row.get(4)?,
        })
    })?;
    let mut entries: Vec<DBEntry> = vec![];
    let mut conversations: Vec<String> = vec![];
    for row in rows {
        let entry = row?.clone();
        let ts: DateTime<Local> = DateTime::from_timestamp_millis(entry.timestamp as i64)
            .unwrap()
            .into();
        let len_context = entry.context.matches(',').collect::<Vec<&str>>().len() + 1;
        conversations.push(format!(
            "{}: {}@{} -> {:.32} [{} context len]",
            ts.format("%Y-%m-%d %H%M"),
            entry.model,
            entry.host,
            entry.conversation.first().unwrap().content,
            len_context
        ));
        entries.push(entry.clone());
    }
    if entries.is_empty() {
        bail!("No responses saved");
    }
    Ok((entries, conversations))
}
