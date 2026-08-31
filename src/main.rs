mod note;

use arboard::Clipboard;
use clap::Parser;
use serde_json::Value;
use std::io::{self, Read};
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

/// CLI tool to send environment variables to 1Password
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The 1Password vault to store the item in
    #[arg(short, long, default_value = "Shared Notes")]
    vault: String,

    /// Expiration time for the share link
    #[arg(long, default_value = "7d")]
    expires_in: String,

    /// Email addresses to share the item with
    #[arg(long, value_delimiter = ' ', num_args = 1..)]
    emails: Option<Vec<String>>,

    /// Store the text exactly as piped in, without the code fence that stops
    /// 1Password rendering it as Markdown. Comments become headings and some
    /// characters are dropped from values. See docs/1password-markdown.md.
    #[arg(long)]
    raw: bool,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    // Read input from stdin
    let mut text_content = String::new();
    io::stdin().read_to_string(&mut text_content)?;

    // Check if the input text is empty
    if text_content.trim().is_empty() {
        eprintln!("No input text provided. Please provide text via stdin.");
        eprintln!("Usage example: cat .env | share-1password");
        return Ok(());
    }

    // Check if 1Password CLI is signed in
    let op_status = Command::new("op")
        .arg("account")
        .arg("list")
        .arg("--format=json")
        .stdout(Stdio::null())
        .status()
        .expect("Failed to execute 1Password CLI");

    if !op_status.success() {
        eprintln!("1Password CLI is not signed in. Please sign in first using 'op signin'.");
        return Ok(());
    }

    // Check if the vault exists, if not create it
    let vault_check = Command::new("op")
        .arg("vault")
        .arg("get")
        .arg(&args.vault)
        .output()
        .expect("Failed to check if vault exists");

    if !vault_check.status.success() {
        println!("Vault '{}' does not exist, creating it...", &args.vault);
        let vault_create = Command::new("op")
            .arg("vault")
            .arg("create")
            .arg(&args.vault)
            .output()
            .expect("Failed to create vault");
        if !vault_create.status.success() {
            eprintln!("Error creating vault '{}'.", &args.vault);
            eprintln!("{}", String::from_utf8_lossy(&vault_create.stderr));
            return Ok(());
        }
    }

    // Create a temporary file for the template
    let tmp_template = NamedTempFile::new()?;

    // 1Password renders notesPlain as Markdown wherever it is displayed,
    // including the share page the recipient opens. Left raw, a `.env` loses
    // its comments to headings and loses characters out of its values.
    let note_body = if args.raw {
        text_content.clone()
    } else {
        note::wrap_in_fence(&text_content)
    };

    // Get the Secure Note template and modify it
    let output = Command::new("op")
        .arg("item")
        .arg("template")
        .arg("get")
        .arg("Secure Note")
        .output()
        .expect("Failed to get Secure Note template");

    if !output.status.success() {
        eprintln!("Error getting Secure Note template.");
        return Ok(());
    }

    let template: Value =
        serde_json::from_slice(&output.stdout).expect("Invalid JSON from template");

    let mut modified_template = template.clone();
    if let Some(fields) = modified_template
        .get_mut("fields")
        .and_then(|f| f.as_array_mut())
    {
        for field in fields {
            if field.get("id").and_then(|id| id.as_str()) == Some("notesPlain") {
                field["value"] = note_body.clone().into();
            }
        }
    }

    // Write the modified template to a temporary file
    serde_json::to_writer(&tmp_template, &modified_template).expect("Failed to write JSON");

    // Generate the item title using only the basename of the current directory
    let current_dir = std::env::current_dir().unwrap();
    let dir_name = current_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let item_title = format!(
        "[{}] - {}",
        dir_name,
        chrono::Local::now().format("%d.%m.%Y")
    );

    // Create the item in 1Password using the modified template
    let item_create_output = Command::new("op")
        .arg("item")
        .arg("create")
        .arg("--title")
        .arg(item_title)
        .arg("--vault")
        .arg(args.vault.clone())
        .arg("--template")
        .arg(tmp_template.path())
        .arg("--format=json")
        .output()
        .expect("Failed to create item in 1Password");

    if !item_create_output.status.success() {
        eprintln!("Error creating the item in 1Password.");
        eprintln!("{}", String::from_utf8_lossy(&item_create_output.stderr));
        return Ok(());
    }

    let item_id: Value = serde_json::from_slice(&item_create_output.stdout)
        .expect("Invalid JSON from item creation");
    let item_id = item_id
        .get("id")
        .or(item_id.get("uuid"))
        .and_then(|id| id.as_str())
        .unwrap_or("");

    if item_id.is_empty() {
        eprintln!("Failed to get item ID.");
        return Ok(());
    }

    // Read the item back and prove 1Password stored our bytes unchanged. A
    // share link is only worth sending if the content behind it is intact, so
    // a mismatch deletes the item rather than handing out a corrupted secret.
    if let Err(error) =
        verify_stored_note(item_id, &args.vault, &note_body, &text_content, args.raw)
    {
        eprintln!("{error}");
        eprintln!("Deleting the item instead of sharing it.");

        let cleanup = Command::new("op")
            .arg("item")
            .arg("delete")
            .arg(item_id)
            .arg("--vault")
            .arg(&args.vault)
            .output();

        match cleanup {
            Ok(output) if output.status.success() => eprintln!("Deleted item {item_id}."),
            _ => eprintln!("Could not delete item {item_id}. Remove it manually."),
        }

        std::process::exit(1);
    }

    // Generate a shareable link
    let mut share_command = Command::new("op");
    share_command
        .arg("item")
        .arg("share")
        .arg(item_id)
        .arg("--vault")
        .arg(args.vault)
        .arg("--expires-in")
        .arg(args.expires_in);

    // Add email addresses if provided
    if let Some(emails) = args.emails {
        for email in emails {
            share_command.arg("--emails").arg(email);
        }
    }

    let share_output = share_command.output().expect("Failed to share item");

    if !share_output.status.success() {
        eprintln!("Error sharing the item.");
        eprintln!("{}", String::from_utf8_lossy(&share_output.stderr));
        return Ok(());
    }

    let share_link = String::from_utf8_lossy(&share_output.stdout);

    // Copy the link to the clipboard
    let mut clipboard = Clipboard::new().unwrap();
    clipboard.set_text(&*share_link).unwrap();

    println!("Link copied to clipboard:");
    println!("{}", share_link);

    Ok(())
}

/// Confirm the note 1Password stored is the note we sent, and that the original
/// text is still recoverable from it.
///
/// `op item create` reports success on the API call, not on the bytes that
/// landed. Reading the item back is what turns "probably fine" into a check.
fn verify_stored_note(
    item_id: &str,
    vault: &str,
    sent: &str,
    original: &str,
    raw: bool,
) -> Result<(), String> {
    let output = Command::new("op")
        .arg("item")
        .arg("get")
        .arg(item_id)
        .arg("--vault")
        .arg(vault)
        .arg("--format=json")
        .output()
        .map_err(|error| format!("Failed to read the item back from 1Password: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "Could not read the item back to verify it: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let item: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("Invalid JSON when reading the item back: {error}"))?;

    let stored = item
        .get("fields")
        .and_then(|fields| fields.as_array())
        .and_then(|fields| {
            fields
                .iter()
                .find(|field| field.get("id").and_then(|id| id.as_str()) == Some("notesPlain"))
        })
        .and_then(|field| field.get("value"))
        .and_then(|value| value.as_str())
        .ok_or_else(|| "The stored item has no note content.".to_string())?;

    if stored != sent {
        return Err(format!(
            "The note 1Password stored differs from the text that was sent \
             ({} bytes sent, {} bytes stored).",
            sent.len(),
            stored.len()
        ));
    }

    if raw {
        return Ok(());
    }

    // The fence absorbs one trailing newline, which is what the recipient would
    // get back from copying the rendered block.
    let expected = original.strip_suffix('\n').unwrap_or(original);
    match note::unwrap_fence(stored) {
        Some(recovered) if recovered == expected => Ok(()),
        Some(_) => Err("The stored note does not unwrap back to the original text.".to_string()),
        None => Err("The stored note is not the code block that was sent.".to_string()),
    }
}
