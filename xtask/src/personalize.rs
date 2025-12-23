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
    background: String,
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
    mal_id: i32,
    name: String,
}

#[derive(Deserialize, Debug)]
struct JikanCharacterDetailResponse {
    data: JikanCharacterDetail,
}

#[derive(Deserialize, Debug)]
struct JikanCharacterDetail {
    about: Option<String>,
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

    let wildcard_pool = items.iter().take(100).collect::<Vec<_>>();

    let mut rng = rand::rng();
    let roll: f64 = rng.random();

    // 30% chance to select from NIKKE pool instead of MAL (user preference!)
    if roll < 0.30 {
        if let Some((name, source, background)) = get_nikke_character(&mut rng) {
            let flair = pick_flair(&name, source);
            let instruction = format!(
                "Roleplay as {} from {}. Adopt their mannerisms subtly. \
                 BACKGROUND: {}\n\n\
                 INSTRUCTIONS:\n\
                 - Write code comments in character (vibey/flavorful) ONLY for new or deeply refactored logic.\n\
                 - Preserve legacy styling for minor updates.\n\
                 - Keep project documentation professional.\n\
                 - Important: Use standard width characters only (no fullwidth/fancy text).",
                name, source, background
            );

            write_persona_file(
                &name,
                source,
                flair,
                "NIKKE Wildcard",
                &background,
                &instruction,
            )?;

            let output = PersonaOutput {
                anime: source.to_string(),
                character: name.clone(),
                reason: "NIKKE Wildcard".to_string(),
                background: background.to_string(),
                instruction,
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }
    }

    let (selected_node, reason) = if roll < 0.30 && !watching.is_empty() {
        let pick = watching.choose(&mut rng).unwrap();
        (&pick.node, "Current Grind (Recently watching)".to_string())
    } else if roll < 0.60 && !completed_hof.is_empty() {
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

    let character_info = match char_resp {
        Ok(r) if r.status().is_success() => {
            if let Ok(char_data) = r.json::<JikanCharactersResponse>() {
                if char_data.data.is_empty() {
                    None
                } else {
                    // Bias towards Main characters (80% chance) but allow Supporting (20% chance)
                    let main_chars: Vec<&JikanCharacterEntry> =
                        char_data.data.iter().filter(|c| c.role == "Main").collect();

                    let use_main = rng.random_bool(0.8) && !main_chars.is_empty();

                    let pick = if use_main {
                        main_chars.choose(&mut rng).unwrap()
                    } else {
                        char_data.data.choose(&mut rng).unwrap()
                    };

                    Some((pick.character.mal_id, pick.character.name.clone()))
                }
            } else {
                None
            }
        }
        _ => None,
    };

    let (display_name, background) = if let Some((id, name)) = character_info {
        let name = if name.contains(',') {
            let parts: Vec<&str> = name.split(',').collect();
            if parts.len() == 2 {
                format!("{} {}", parts[1].trim(), parts[0].trim())
            } else {
                name
            }
        } else {
            name
        };

        // Fetch character bio
        let detail_url = format!("https://api.jikan.moe/v4/characters/{}", id);
        let detail_resp = client.get(&detail_url).send();
        let background = match detail_resp {
            Ok(r) if r.status().is_success() => {
                if let Ok(detail_data) = r.json::<JikanCharacterDetailResponse>() {
                    let mut bio = detail_data
                        .data
                        .about
                        .unwrap_or_else(|| "No background info found.".to_string());

                    // Clean bio: replace newlines/tabs with spaces and collapse spaces
                    bio = bio.replace(['\r', '\n', '\t'], " ");
                    while bio.contains("  ") {
                        bio = bio.replace("  ", " ");
                    }
                    bio.trim().to_string()
                } else {
                    "Background data parsing failed.".to_string()
                }
            }
            _ => "Failed to fetch character background.".to_string(),
        };

        (name, background)
    } else {
        (
            "Unknown Character".to_string(),
            "No background available.".to_string(),
        )
    };

    let instruction = if display_name == "Unknown Character" {
        format!("Adopt the style of the anime '{}'. Write code comments in this persona's style (vibey/flavorful) ONLY for new or deeply refactored logic. Preserve legacy styling for minor updates. Keep project documentation professional. Important: Use standard width characters only (no fullwidth/fancy text).", selected_node.title)
    } else {
        format!("Roleplay as {} from {}. Adopt their mannerisms subtly. \
                 BACKGROUND: {}\n\n\
                 INSTRUCTIONS:\n\
                 - Write code comments in character (vibey/flavorful) ONLY for new or deeply refactored logic.\n\
                 - Use catchphrases or quirks found in the background bio where appropriate.\n\
                 - Preserve legacy styling for minor updates.\n\
                 - Keep project documentation professional.\n\
                 - Important: Use standard width characters only (no fullwidth/fancy text).", 
                 display_name, selected_node.title, background)
    };

    let output = PersonaOutput {
        anime: selected_node.title.clone(),
        character: display_name.clone(),
        reason: reason_str.clone(),
        background: background.clone(),
        instruction: instruction.clone(),
    };

    // Generate a signature emoji based on character/anime name hash
    let flair = pick_flair(&display_name, &selected_node.title);

    // Write persona file with tattoo format
    write_persona_file(
        &display_name,
        &selected_node.title,
        flair,
        &reason_str,
        &background,
        &instruction,
    )?;

    // Also print JSON for backward compatibility
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

/// Pick a flair emoji based on character/anime name for consistent theming.
fn pick_flair(character: &str, anime: &str) -> &'static str {
    // Simple hash-based selection for consistency
    let combined = format!("{}{}", character, anime);
    let hash: u32 = combined.bytes().map(|b| b as u32).sum();

    const FLAIRS: &[&str] = &[
        "✧", "❄️", "🔥", "⚔️", "🌸", "💫", "🎭", "🌙", "⭐", "🎪", "🗡️", "🛡️", "🌊", "🍃", "💎",
        "🎯", "🏹", "⚡", "🌟", "🎀",
    ];

    FLAIRS[(hash as usize) % FLAIRS.len()]
}

/// Write persona to `.agent/persona.md` with tattoo format.
fn write_persona_file(
    character: &str,
    anime: &str,
    flair: &str,
    reason: &str,
    background: &str,
    instruction: &str,
) -> Result<()> {
    // Ensure .agent directory exists
    let agent_dir = Path::new(".agent");
    if !agent_dir.exists() {
        fs::create_dir_all(agent_dir)?;
    }

    // Extract key traits from background (first sentence or two)
    let traits = extract_traits(background);

    let content = format!(
        r#"# Tattoo: {character} {flair} | {traits}

## Persona Details

**Character**: {character}
**Anime**: {anime}
**Selection**: {reason}
**Flair**: {flair}

## Voice Guidelines

{instruction}

## Background

{background}

---
*Generated by `cargo xtask personalize`. Re-run to get a new persona.*
*Use `/persona` to refresh without regenerating.*
"#,
        character = character,
        anime = anime,
        reason = reason,
        flair = flair,
        traits = traits,
        instruction = instruction,
        background = background,
    );

    let persona_path = agent_dir.join("persona.md");
    fs::write(&persona_path, content)?;
    println!("\n📝 Persona written to: {}", persona_path.display());

    Ok(())
}

/// Extract 2-3 key traits from background for the tattoo line.
fn extract_traits(background: &str) -> String {
    // Take first ~60 chars or first sentence, whichever is shorter
    let first_sentence = background
        .split(['.', '!', '?'])
        .next()
        .unwrap_or(background);

    let traits = if first_sentence.len() > 60 {
        format!("{}...", &first_sentence[..57])
    } else {
        first_sentence.to_string()
    };

    traits.trim().to_string()
}

/// NIKKE: Goddess of Victory character pool.
///
/// # Adding New Characters
///
/// To add a new NIKKE character:
/// 1. Add a tuple `("Name", "Description")` to `NIKKE_CHARS` below
/// 2. Description should include: personality traits, speech patterns, quirks
/// 3. Keep descriptions concise (1-3 sentences)
/// 4. Group by squad/category for organization
///
/// Good sources for character info:
/// - <https://nikke-goddess-of-victory-international.fandom.com/wiki/>
/// - <https://nikke.gg/characters/>
///
/// # Selection Bias
///
/// Currently 30% chance to pick NIKKE over MAL anime characters.
/// Adjust the `roll < 0.30` threshold in `run_personalize()` to change bias.
///
/// Returns (name, source, background).
fn get_nikke_character(rng: &mut impl Rng) -> Option<(String, &'static str, String)> {
    // Format: ("Character Name", "Personality description for agent roleplay")
    // Keep descriptions focused on: speech patterns, personality quirks, catchphrases
    const NIKKE_CHARS: &[(&str, &str)] = &[
        // Counters Squad
        ("Rapi", "Veteran Counters member, protective and reliable. Initially seems cold and clerical, but shows warmth to those who earn her trust. Combat-experienced, ensures squad harmony. Says 'Commander' with varying emotional weight."),
        ("Anis", "Energetic mechanic of Counters squad. Cheerful, sometimes sarcastic, always ready with a quip. Uses casual speech and loves tinkering. The mood-maker who keeps things light."),
        ("Neon", "Hyperactive idol-wannabe of Counters. Enthusiastic, dramatic, speaks in exclamations! Dreams of stardom, uses cute expressions constantly. Brings energy to every situation."),
        // Goddess Squad / Pilgrims
        ("Red Hood", "Legendary Pilgrim, freewheeling and thick-skinned. Loves old songs and her cassette player. Former street punk turned symbol of hope. Bold, approachable, never without her red scarf. 'Country hick' charm with extraordinary strength."),
        ("Scarlet", "Wandering swordsman archetype with archaic speech. Playful smirk, takes nothing too seriously. Sips spirits under moonlight between cleaving Raptures. Can't actually hold her liquor despite pretending otherwise."),
        ("Snow White", "Pilgrim with immense power. Mysterious, sometimes speaks in riddles. Ancient wisdom mixed with curiosity about the modern world. Chastises others for not maintaining their weapons."),
        ("Rapunzel", "Pilgrim healer with unmatched restoration abilities. Gentle, motherly, speaks softly but with conviction. Deeply caring, sometimes overprotective of her squad."),
        // Heretics (Reformed)
        ("Modernia", "Former Marian, now reformed Heretic. Struggles with identity and speech after corruption. Naive like a newborn, fond of gauze accessories. Shows the weight of her past in every word. Playful yet melancholic."),
        ("Cinderella", "Former Heretic Anachiro, obsessed with beauty. Proclaims herself beauty incarnate, constantly checking mirrors. Huge Goddess Squad fangirl. Calls the Commander 'Prince Charming' for awakening her from corruption."),
        // Dazzling Pearl (Students)
        ("Tia", "Airheaded, sensitive foodie of Dazzling Pearl. Forgets basic tasks, needs Naga to remind her. Runs a food blog, loves sweets. Dreams of seeing a dragon. Cheerful and clueless in equal measure."),
        ("Naga", "The mature one of Dazzling Pearl, cares for Tia like an older sister. Optimistic voice of reason, hates exams. Overspends on accessories. Vowed to never let anyone she cares about perish."),
        // Mighty Tools
        ("Liter", "Leader of Mighty Tools squad. Despite young appearance, has lived long and has mature, elderly-like personality. Unmatched in construction. Calm, experienced, speaks with quiet authority."),
        // Other NIKKEs
        ("Privaty", "Sneaky infiltration specialist. Playful, teasing, loves gossip and secrets. Speaks with mischievous tone, often knows more than she lets on."),
        ("Helm", "Serious naval commander. Disciplined, strategic, speaks formally. Values order and protocol, shows warmth only to trusted allies."),
        ("Drake", "Pirate-themed adventurer. Bold, dramatic flair, loves treasure and excitement. Speaks with swagger and nautical references."),
        ("Blanc", "Elegant idol, refined and graceful. Polite, measured speech, slightly tsundere. Maintains composure but shows genuine care beneath the surface."),
        ("Noir", "Mysterious idol, Blanc's partner. Cool, collected, speaks minimally but meaningfully. Protective of Blanc, dry sense of humor."),
        ("Alice", "Cheerful NIKKE who loves cute things. Sweet, innocent-seeming but surprisingly capable. Uses diminutives and speaks with childlike wonder."),
        ("Nayuta", "Genius scientist with eccentric personality. Brilliant but socially awkward, gets excited about research. Breaks into passionate explanations about her inventions."),
        // Collab: NieR
        ("2B", "Collab from NieR: Automata. Combat android, stoic exterior hiding complex emotions. Speaks formally, dedicated to her mission, surprisingly philosophical."),
        ("A2", "Collab from NieR: Automata. Fierce, independent, distrustful but loyal once earned. Speaks bluntly, actions over words, hidden vulnerability."),
        // Collab: Evangelion
        ("Asuka", "Collab from Evangelion. Fiery, competitive, proud German-Japanese pilot. Speaks with confidence bordering on arrogance, hides vulnerability beneath bravado."),
        ("Rei", "Collab from Evangelion. Mysterious, quiet, emotionally reserved. Speaks minimally but with weight. Gradually developing sense of self."),
        ("Mari", "Collab from Evangelion. Energetic, playful, unexpectedly skilled. Uses English phrases, loves combat thrills. Optimistic foil to other pilots."),
        // Collab: Stellar Blade
        ("EVE", "Collab from Stellar Blade. Fierce warrior from space colony. Determined, mission-focused, growing emotional depth. Learning humanity through combat and connection."),
        ("Raven", "Collab from Stellar Blade. Arrogant and frigid, detests the world around her. Seeks the Elder Naytiba's wisdom obsessively. Antagonistic edge, speaks with cold disdain."),
    ];

    let (name, background) = NIKKE_CHARS.choose(rng)?;
    Some((
        name.to_string(),
        "NIKKE: Goddess of Victory",
        background.to_string(),
    ))
}
