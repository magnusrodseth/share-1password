//! Turning raw stdin content into a note body that 1Password renders verbatim.
//!
//! 1Password renders a Secure Note's `notesPlain` field as Markdown, both in
//! the apps and on the `share.1password.com` page the recipient opens. A `.env`
//! file pasted in as-is therefore gets mangled: `# comments` become headings,
//! and characters inside values are silently deleted. `docs/1password-markdown.md`
//! records the measurements.
//!
//! Wrapping the content in a fenced code block stops all of it. The fence is
//! the only text we add, and the recipient copies the block back out unchanged.

/// The shortest backtick fence the content cannot close early.
///
/// A fenced block ends at the first line whose backtick run is at least as long
/// as the opening fence, so the opening fence has to out-run every backtick run
/// in the content. Three backticks is the floor.
fn fence_for(content: &str) -> String {
    "`".repeat(3.max(longest_backtick_run(content) + 1))
}

fn longest_backtick_run(content: &str) -> usize {
    let mut longest = 0;
    let mut current = 0;

    for ch in content.chars() {
        if ch == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }

    longest
}

/// Wrap `content` in a code fence so 1Password renders it character for character.
///
/// One trailing newline is absorbed by the fence, the way it would be in any
/// Markdown document. Everything else survives, including comments, blank lines,
/// indentation, and values containing `_`, `*`, `-`, `>` or `#`.
pub fn wrap_in_fence(content: &str) -> String {
    let fence = fence_for(content);
    let body = content.strip_suffix('\n').unwrap_or(content);

    format!("{fence}\n{body}\n{fence}")
}

/// Recover the content that [`wrap_in_fence`] was given.
///
/// Returns `None` if `note` is not a single fenced block, which is what makes
/// this usable as a verification step rather than a guess.
pub fn unwrap_fence(note: &str) -> Option<&str> {
    let fence_len = note.chars().take_while(|&c| c == '`').count();
    if fence_len < 3 {
        return None;
    }

    let fence = &note[..fence_len];
    note.strip_prefix(fence)?
        .strip_prefix('\n')?
        .strip_suffix(fence)?
        .strip_suffix('\n')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The content that made this necessary: comments 1Password turned into headings.
    #[test]
    fn keeps_comments_out_of_headings() {
        let env = "# Databases\nDATABASE_URL=postgresql://localhost:5432/db\n";
        let note = wrap_in_fence(env);

        assert_eq!(
            note,
            "```\n# Databases\nDATABASE_URL=postgresql://localhost:5432/db\n```"
        );
        assert_eq!(
            unwrap_fence(&note),
            Some("# Databases\nDATABASE_URL=postgresql://localhost:5432/db")
        );
    }

    /// Values 1Password silently ate characters out of when rendered as Markdown.
    #[test]
    fn survives_markdown_active_characters() {
        let env = "A=_under_ and *star*\nB=a|b\nC=---\n- dash\n> quote\n1. numbered\n";

        assert_eq!(
            unwrap_fence(&wrap_in_fence(env)),
            Some(env.strip_suffix('\n').unwrap())
        );
    }

    #[test]
    fn widens_the_fence_past_backticks_in_the_content() {
        assert_eq!(fence_for("no backticks"), "```");
        assert_eq!(fence_for("a `b` c"), "```");
        assert_eq!(fence_for("a ``b`` c"), "```");
        assert_eq!(fence_for("```\nnested\n```"), "````");
        assert_eq!(fence_for("`````"), "``````");
    }

    /// A content line matching the fence would end the block early and drop the rest.
    #[test]
    fn content_cannot_close_the_fence_early() {
        let env = "SECRET=```\nKEEP_ME=1\n";
        let note = wrap_in_fence(env);

        assert!(note.starts_with("````\n"));
        assert_eq!(unwrap_fence(&note), Some("SECRET=```\nKEEP_ME=1"));
    }

    #[test]
    fn round_trips_blank_lines_and_indentation() {
        let env = "A=1\n\n\n    indented=2\n\tTABBED=3\n";

        assert_eq!(
            unwrap_fence(&wrap_in_fence(env)),
            Some(env.strip_suffix('\n').unwrap())
        );
    }

    #[test]
    fn round_trips_content_without_a_trailing_newline() {
        assert_eq!(unwrap_fence(&wrap_in_fence("A=1")), Some("A=1"));
    }

    /// CRLF input keeps its carriage returns; only the final newline is absorbed.
    #[test]
    fn round_trips_crlf() {
        assert_eq!(
            unwrap_fence(&wrap_in_fence("A=1\r\nB=2\r\n")),
            Some("A=1\r\nB=2\r")
        );
    }

    #[test]
    fn rejects_notes_that_are_not_a_fenced_block() {
        assert_eq!(unwrap_fence("A=1\nB=2"), None);
        assert_eq!(unwrap_fence("``\nA=1\n``"), None);
        assert_eq!(unwrap_fence("```\nunterminated"), None);
    }
}
