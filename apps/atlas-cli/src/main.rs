use clap::{Parser, Subcommand};
use reqwest::Client;
use std::env;
use std::io::{self, BufRead, Write};

#[derive(Parser)]
#[command(name = "atlas")]
#[command(about = "Atlas Orchestration CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    List,
    Send {
        to: String,
        content: String,
    },
    Inbox {
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },
    Status,
    Mcp,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let client = Client::new();

    let server_url =
        env::var("ATLAS_SERVER_URL").unwrap_or_else(|_| "http://localhost:4000".to_string());
    let current_session_id = env::var("ATLAS_SESSION_ID").unwrap_or_else(|_| "unknown".to_string());

    match cli.command {
        Commands::Mcp => {
            let stdin = io::stdin();
            let mut stdout = io::stdout();
            let url = format!("{}/api/mcp", server_url);

            for line in stdin.lock().lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }

                let res = client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .header("X-Atlas-Session-Id", &current_session_id)
                    .body(line)
                    .send()
                    .await;

                match res {
                    Ok(response) => {
                        let text = response.text().await?;
                        writeln!(stdout, "{}", text)?;
                        stdout.flush()?;
                    }
                    Err(e) => {
                        eprintln!("[ATLAS-MCP] Error communicating with server: {}", e);
                    }
                }
            }
        }
        Commands::List => {
            let url = format!("{}/api/sessions/active", server_url);
            let res = client
                .get(&url)
                .send()
                .await?
                .json::<serde_json::Value>()
                .await?;
            if let Some(sessions) = res["data"].as_array() {
                println!("\x1b[1;34mACTIVE SESSIONS:\x1b[0m");
                for s in sessions {
                    let id = s[0].as_str().unwrap_or("?");
                    let status = if id == current_session_id {
                        " (YOU)"
                    } else {
                        ""
                    };
                    println!("  - \x1b[32m{}\x1b[0m{}", id, status);
                }
            }
        }
        Commands::Send { to, content } => {
            let url = format!("{}/api/sessions/{}/send", server_url, current_session_id);
            let payload = serde_json::json!({ "toSessionId": to, "content": content });
            let res = client.post(&url).json(&payload).send().await?;
            if res.status().is_success() {
                println!("\x1b[32m✔\x1b[0m Message sent to {}", to);
            }
        }
        Commands::Inbox { limit } => {
            let url = format!(
                "{}/api/sessions/{}/messages",
                server_url, current_session_id
            );
            let res = client
                .get(&url)
                .send()
                .await?
                .json::<serde_json::Value>()
                .await?;
            if let Some(msgs) = res["data"].as_array() {
                let start = if msgs.len() > limit {
                    msgs.len() - limit
                } else {
                    0
                };
                println!("\x1b[1;35m[ATLAS INBOX]\x1b[0m");
                for msg_val in &msgs[start..] {
                    let from = msg_val["fromId"].as_str().unwrap_or("unknown");
                    let content = msg_val["content"].as_str().unwrap_or("");
                    println!("\x1b[1;33m{}\x1b[0m: {}", from, content);
                }
            }
        }
        Commands::Status => {
            println!("\x1b[1;34mATLAS SESSION STATUS:\x1b[0m");
            println!("  Session ID: \x1b[32m{}\x1b[0m", current_session_id);
            println!("  Server URL: \x1b[32m{}\x1b[0m", server_url);
        }
    }

    Ok(())
}
