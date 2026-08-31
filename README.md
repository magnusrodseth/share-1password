# 🔐 Share 1Password

## What is it?

**Securely share notes with others using 1Password. Perfect for developers sending credentials to clients.**

I often find myself needing to send `.env` files, credentials, and other sensitive information to others, in particular team members or clients. I would like something as simple as:

1. I pipe the file with the text to share into the CLI program 🚀

2. A secure link to share is copied to my clipboard 📋

3. Simple as that. There is no step 3 ✅

**So I wrote just that! 🎉**

In my opinion, 1Password is the single best password manager out there, so I based my implementation on their CLI.

## Installation

### Prerequisites

Ensure you have [1Password](https://1password.com/) installed on your machine.

Next, in the 1Password application, navigate to **Preferences** > **Developer**. Enable the **Integrate with 1Password CLI** option.

Also ensure you have the 1Password CLI installed. Refer to the [installation guide](https://support.1password.com/command-line-getting-started/) for more information.

### Using Cargo

Ensure you have `cargo` installed. Then, run the following command:

```bash
# Install the application
cargo install share-1password
```

## Usage

Ensure you have `share-1password` installed. Then, run the following commands:

```bash
# Navigate to the directory with the note you want to share, e.g. a `.env` file
cd <directory>

# Pipe the file into the application with default settings
cat <file> | share-1password

# Use `--help` to see all available options
share-1password --help

# Use `--vault` to specify the vault to store the note in
cat <file> | share-1password --vault <vault-name>

# Use `--emails` to specify the emails to share the note with. Default to anyone with the link.
cat <file> | share-1password --emails <email1> <email2> <email3>

# Use `--raw` to store the text as-is and let 1Password render it as Markdown
cat <file> | share-1password --raw
```

## How the content is stored

1Password renders a shared note as Markdown on the page the recipient opens. A
`.env` piped in unchanged comes out mangled: `# comments` turn into headings, and
values lose characters, so `SECRET=_a_*b*` arrives as `SECRET=ab`. Nothing warns
either side.

So `share-1password` wraps the content in a code fence before storing it. The
recipient sees the file verbatim in a monospace block and can copy it back out
byte for byte, comments included. After creating the item the CLI reads it back
and checks the stored text still unwraps to what was piped in; if it does not,
the item is deleted instead of shared.

Pass `--raw` to skip the fence and get the old behaviour, which is what you want
when the note is prose and the Markdown formatting is the point.

[`docs/1password-markdown.md`](docs/1password-markdown.md) records what was
measured, including the escape routes that do not work.

✂️ Note that `share-1password` automatically copies the link of the shared note to your clipboard.

You can now share this link securely with others, for instance using email, Slack, or any other messaging platform.
