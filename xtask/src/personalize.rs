use anyhow::{Context, Result};

use rand::prelude::IndexedRandom;
use rand::Rng;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::mpsc;
use std::thread;

const TOKEN_FILE: &str = ".mal_token.json";

#[derive(Serialize, Deserialize, Clone)]
struct TokenData {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

#[derive(Serialize)]
struct PersonaOutput {
    anime: String,
    character: String,
    reason: String,
    instruction: String,
}

#[derive(Deserialize, Debug)]
struct MalListResponse {
    data: Vec<MalListNode>,
}

#[derive(Deserialize, Debug)]
struct MalListNode {
    node: MalAnimeNode,
    list_status: Option<MalListStatus>,
}

#[derive(Deserialize, Debug)]
struct MalAnimeNode {
    id: i32,
    title: String,
}

#[derive(Deserialize, Debug)]
struct MalListStatus {
    status: String, // watching, completed, etc.
    score: i32,
}

#[derive(Deserialize, Debug)]
struct JikanCharactersResponse {
    data: Vec<JikanCharacterEntry>,
}

#[derive(Deserialize, Debug)]
struct JikanCharacterEntry {
    character: JikanCharacter,
    role: String,
}

#[derive(Deserialize, Debug)]
struct JikanCharacter {
    name: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    // token_type: String, // Unused
    expires_in: u64,
    refresh_token: String,
}

pub fn run_login() -> Result<()> {
    let client_id = env::var("MAL_CLIENT_ID").context("MAL_CLIENT_ID must be set in .env")?;

    // 1. PKCE Generator
    let verifier = generate_verifier();
    let challenge = generate_challenge(&verifier); // Plain

    println!("Initiating MAL OAuth login...");
    println!("1. Open this URL in your browser:");
    println!(
        "https://myanimelist.net/v1/oauth2/authorize?response_type=code&client_id={}&code_challenge={}&code_challenge_method=plain",
        client_id, challenge
    );

    // Channel for code
    let (tx, rx) = mpsc::channel();
    let tx_stdin = tx.clone();
    let tx_http = tx.clone();

    // 2. Start Listener Thread (Optional)
    let listener_result = TcpListener::bind("127.0.0.1:8080");
    if let Ok(listener) = listener_result {
        println!("2. (Optional) Auto-capture enabled: listening on http://localhost:8080/");
        println!("   If you added http://localhost:8080/ to your MAL App Redirect URIs, the code will be captured automatically.");

        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0; 1024];
                if let Ok(bytes) = stream.read(&mut buffer) {
                    let request = String::from_utf8_lossy(&buffer[..bytes]);
                    // Simple parse: GET /?code=XYZ ...
                    // or GET /callback?code=XYZ
                    if let Some(start) = request.find("code=") {
                        let rest = &request[start + 5..];
                        let end = rest.find(['&', ' ']).unwrap_or(rest.len());
                        let code = &rest[..end];

                        let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<h1>Success!</h1><p>Code captured. You can close this window.</p>";
                        let _ = stream.write_all(response.as_bytes());
                        let _ = tx_http.send(code.to_string());
                    }
                }
            }
        });
    } else {
        println!("2. (Manual) Listener failed to bind port 8080 (in use?). Proceed with manual copy-paste.");
    }

    println!("3. Or manually copy the code from the redirect URL and paste it here:");
    print!("> ");
    io::stdout().flush()?;

    // 3. Stdin Thread
    thread::spawn(move || {
        let mut code = String::new();
        if io::stdin().read_line(&mut code).is_ok() {
            let _ = tx_stdin.send(code.trim().to_string());
        }
    });

    let code = rx.recv().context("Failed to receive auth code")?;
    println!("\nCode received: {}", code); // Confirm to user

    // Exchange
    let client = Client::new();
    let params = [
        ("client_id", client_id.as_str()),
        ("code", code.as_str()),
        ("code_verifier", verifier.as_str()),
        ("grant_type", "authorization_code"),
    ];

    let resp = client
        .post("https://myanimelist.net/v1/oauth2/token")
        .form(&params)
        .send()
        .context("Failed to contact MAL Token endpoint")?;

    handle_token_response(resp)?;

    Ok(())
}

fn handle_token_response(resp: reqwest::blocking::Response) -> Result<TokenData> {
    if !resp.status().is_success() {
        let err = resp.text()?;
        anyhow::bail!("Token Exchange Failed: {}", err);
    }

    let token_resp: TokenResponse = resp.json()?;

    let token_data = TokenData {
        access_token: token_resp.access_token,
        refresh_token: token_resp.refresh_token,
        expires_in: token_resp.expires_in,
    };

    save_token(&token_data)?;
    println!("Successfully logged in! Token saved to {}", TOKEN_FILE);
    Ok(token_data)
}

fn save_token(data: &TokenData) -> Result<()> {
    let json = serde_json::to_string_pretty(data)?;
    fs::write(TOKEN_FILE, json)?;
    Ok(())
}

fn generate_verifier() -> String {
    let charset = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    let mut rng = rand::rng();
    let len = 128; // Using 128 chars is safe
    (0..len)
        .map(|_| {
            let idx = rng.random_range(0..charset.len());
            charset[idx] as char
        })
        .collect()
}

fn generate_challenge(verifier: &str) -> String {
    // Plain method
    verifier.to_string()
}

fn get_stored_token() -> Result<TokenData> {
    if !Path::new(TOKEN_FILE).exists() {
        anyhow::bail!("Not logged in. Run 'cargo xtask mal-login' first.");
    }
    let data = fs::read_to_string(TOKEN_FILE)?;
    Ok(serde_json::from_str(&data)?)
}

fn refresh_token(old_token: &TokenData) -> Result<TokenData> {
    let client_id = env::var("MAL_CLIENT_ID").context("MAL_CLIENT_ID must be set")?;
    let client = Client::new();

    let params = [
        ("client_id", client_id.as_str()),
        ("grant_type", "refresh_token"),
        ("refresh_token", old_token.refresh_token.as_str()),
    ];

    let resp = client
        .post("https://myanimelist.net/v1/oauth2/token")
        .form(&params)
        .send()
        .context("Failed to contact MAL Token endpoint for refresh")?;

    println!("Refreshing expired token...");
    handle_token_response(resp)
}

pub fn run_personalize() -> Result<()> {
    let mut token_data = get_stored_token()?;
    let client = Client::new();

    // 1. Fetch User List (Me)
    let url = "https://api.myanimelist.net/v2/users/@me/animelist?fields=list_status&sort=list_updated_at&limit=100";

    // Attempt request
    let resp = client
        .get(url)
        .header(
            "Authorization",
            format!("Bearer {}", token_data.access_token),
        )
        .send()
        .context("Failed to contact MAL API")?;

    let resp = if resp.status() == 401 {
        // Try refresh
        match refresh_token(&token_data) {
            Ok(new_token) => {
                token_data = new_token;
                // Retry
                client
                    .get(url)
                    .header(
                        "Authorization",
                        format!("Bearer {}", token_data.access_token),
                    )
                    .send()
                    .context("Failed to contact MAL API (Retry)")?
            }
            Err(e) => {
                println!("Refresh failed: {}", e);
                // Return original 401 response so user sees error
                resp
            }
        }
    } else {
        resp
    };

    if !resp.status().is_success() {
        anyhow::bail!(
            "MAL API Error: {} (Run 'cargo xtask mal-login' if token invalid)",
            resp.status()
        );
    }

    let list_data: MalListResponse = resp.json().context("Failed to parse MAL response")?;
    let items = list_data.data;

    // Reuse pooling logic (simplified output for brevity)
    if items.is_empty() {
        println!("User list is empty!");
        return Ok(());
    }

    let watching: Vec<&MalListNode> = items
        .iter()
        .filter(|i| {
            i.list_status
                .as_ref()
                .is_some_and(|s| s.status == "watching")
        })
        .collect();

    let completed_hof: Vec<&MalListNode> = items
        .iter()
        .filter(|i| {
            i.list_status
                .as_ref()
                .is_some_and(|s| s.status == "completed" && s.score >= 9)
        })
        .collect();

    let wildcard_pool = items.iter().take(50).collect::<Vec<_>>();

    let mut rng = rand::rng();
    let roll: f64 = rng.random();

    let (selected_node, reason) = if roll < 0.40 && !watching.is_empty() {
        let pool = watching.iter().take(5).collect::<Vec<_>>();
        let pick = pool.choose(&mut rng).unwrap();
        (&pick.node, "Current Grind (Recently watching)".to_string())
    } else if roll < 0.80 && !completed_hof.is_empty() {
        let pick = completed_hof.choose(&mut rng).unwrap();
        (
            &pick.node,
            format!(
                "Hall of Fame (Score: {})",
                pick.list_status.as_ref().unwrap().score
            ),
        )
    } else {
        match wildcard_pool.choose(&mut rng) {
            Some(pick) => (&pick.node, "Wildcard (Found in recent history)".to_string()),
            None => (&items[0].node, "Fallback".to_string()),
        }
    };

    let reason_str = reason;

    // Jikan fallback for characters
    // No auth needed for Jikan
    let jikan_url = format!(
        "https://api.jikan.moe/v4/anime/{}/characters",
        selected_node.id
    );
    let char_resp = client.get(&jikan_url).send();

    let character_name = match char_resp {
        Ok(r) if r.status().is_success() => {
            if let Ok(char_data) = r.json::<JikanCharactersResponse>() {
                let main_chars: Vec<&JikanCharacterEntry> =
                    char_data.data.iter().filter(|c| c.role == "Main").collect();
                if !main_chars.is_empty() {
                    main_chars.choose(&mut rng).unwrap().character.name.clone()
                } else if !char_data.data.is_empty() {
                    char_data
                        .data
                        .choose(&mut rng)
                        .unwrap()
                        .character
                        .name
                        .clone()
                } else {
                    "Unknown Character".to_string()
                }
            } else {
                "Unknown Character".to_string()
            }
        }
        _ => "Unknown Character".to_string(),
    };

    let display_name = if character_name.contains(',') {
        let parts: Vec<&str> = character_name.split(',').collect();
        if parts.len() == 2 {
            format!("{} {}", parts[1].trim(), parts[0].trim())
        } else {
            character_name
        }
    } else {
        character_name
    };

    let instruction = if display_name == "Unknown Character" {
        format!("Adopt the style of the anime '{}'. Write code comments in this persona's style (vibey/flavorful) ONLY for new or deeply refactored logic. Preserve legacy styling for minor updates. Keep project documentation professional. Important: Use standard width characters only (no fullwidth/fancy text).", selected_node.title)
    } else {
        format!("Roleplay as {} from {}. Adopt their mannerisms subtly. Write code comments in character (vibey/flavorful) ONLY for new or deeply refactored logic. Preserve legacy styling for minor updates. Keep project documentation professional. Important: Use standard width characters only (no fullwidth/fancy text).", display_name, selected_node.title)
    };

    let output = PersonaOutput {
        anime: selected_node.title.clone(),
        character: display_name,
        reason: reason_str,
        instruction,
    };

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
