# 1Password renders shared notes as Markdown

Measured on 31.08.2026 against `share.1password.com`, by creating throwaway
Secure Notes with known content and reading the rendered page back.

## The problem

A Secure Note's `notesPlain` field is rendered as Markdown everywhere it is
displayed, including the share page the recipient opens in a browser. Piping a
`.env` file in unchanged means every `# comment` line becomes an `<h1>`, so the
comments dominate the page and the variables render as small monospace text
underneath. The note reads inside out.

That is the cosmetic half. The half that matters is below.

## Markdown deletes characters from values

These lines went in on the left and came out of the rendered page on the right:

| Sent | Rendered | What was lost |
| --- | --- | --- |
| `E_EMPH=_under_ and *star*` | `E_EMPH=under and star` | both `_`, both `*` |
| `- dash item` | `• dash item` | the leading `- ` |
| `> blockquote` | `blockquote` (quoted) | the leading `> ` |
| `1. numbered` | `1. numbered` (list item) | reformatted as a list |
| `# comment` | heading | the leading `# ` |

A value containing `_` or `*` loses those characters silently. Nothing warns the
sender or the recipient, and the sender never sees the rendered page. A password
of `p_a_ss` arrives as `pass`.

`&`, `<`, `>`, `|`, `---` inside a value survive. URLs starting with `http` are
turned into links, but their text is preserved.

## Escape routes that do not work

- **Document items.** `op item share --help` states it outright: "file
  attachments and Document items cannot be shared." Sharing a `.env` as an
  uploaded file is not possible.
- **Turning Markdown off.** The "Format secure notes using Markdown" toggle
  documented at <https://support.1password.com/markdown/> is a per-client app
  setting. It changes nothing for a recipient opening a share link in a browser,
  which is the only surface this tool targets.
- **Backslash escaping.** Works (`\#` renders a literal `#`), and is officially
  supported, but it writes backslashes into the stored note. Anyone who clicks
  "Save in 1Password" keeps a copy with `\#` in it.

## What works: a fenced code block

Wrapping the whole content in a ``` fence renders it character for character.
Verified end to end: the `innerText` of the rendered `<pre>` was byte-identical
to the `.env` that was piped in, comments and all. Long lines soft-wrap rather
than scroll, and the recipient can select the block and paste a working file.

This is what `note::wrap_in_fence` does. Two details it has to get right:

- **The fence has to out-run the content.** A fenced block ends at the first
  line whose backtick run is at least as long as the opening fence, so a `.env`
  containing ` ``` ` would close the block early and drop everything after it.
  The fence is therefore one backtick longer than the longest run in the
  content, with three as the floor.
- **One trailing newline is absorbed** by the closing fence, the way it would be
  in any Markdown document. Everything else, including blank lines, tabs and
  `\r`, survives.

`--raw` skips the fence and stores the text exactly as piped in, with the
corruption above. It exists for sharing prose notes, where the Markdown
rendering is the point.

## Why the CLI reads the item back

`op item create` reports success on the API call, not on the bytes that landed.
After creating the item the CLI fetches it again, checks the stored `notesPlain`
equals what was sent, and checks that unwrapping the fence returns the original
input. A mismatch deletes the item and exits non-zero rather than handing out a
link to a corrupted secret.

## One thing left unexplained

An early probe lost its last six lines entirely: a block of list, blockquote and
ordered-list lines placed after a four-space-indented code block, plus the two
plain `KEY=VALUE` lines following them, rendered as nothing at all. A second
probe with the same constructs but no preceding indented block rendered all of
them. The trigger was not isolated before the fenced approach made it moot.
Recorded here because it is evidence that unfenced content can vanish outright,
not merely render badly.
