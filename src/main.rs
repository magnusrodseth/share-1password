use arboard::Clipboard;
use clap::Parser;
use serde_json::Value;
use std::io::{self, Read, Write};
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
    let mut tmp_env_content = NamedTempFile::new()?;

    // Write the text content to a temporary file
    writeln!(tmp_env_content, "{}", text_content)?;

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
    let content =
        std::fs::read_to_string(tmp_env_content.path()).expect("Failed to read text content");

    let mut modified_template = template.clone();
    if let Some(fields) = modified_template
        .get_mut("fields")
        .and_then(|f| f.as_array_mut())
    {
        for field in fields {
            if field.get("id").and_then(|id| id.as_str()) == Some("notesPlain") {
                field["value"] = content.clone().into();
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
