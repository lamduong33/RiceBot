//! Cleans up replies where the model writes a chat transcript instead of a
//! single message, e.g.
//!
//! ```text
//! RiceBot: Garrett: I've got spirit, boys.
//!
//! RiceBot: Well shit, Garrett.
//! ```
//!
//! Messages reach the model as "Name: message" (with server-wide memory,
//! "[#channel] Name: message"), and some models copy that layout: they label
//! their own reply, repeat the message they're answering, or carry on writing
//! lines for other people.

/// Who a line of the model's output is attributed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Speaker {
    Bot,
    Other,
}

/// Returns only the bot's own words from `reply`:
///
/// - `bot_names:` labels are removed wherever a line starts with one.
/// - Lines labelled with someone in `other_names` before the bot has said
///   anything are dropped: the model repeating the message it's answering.
/// - Output stops at the first such line after the bot has started: the model
///   writing the next turn for someone else.
///
/// Only names in the two lists count as labels, so ordinary text such as
/// "Note: ..." is left alone. If nothing would be left, the reply is returned
/// with just its leading bot labels removed, so there is always something to
/// send.
pub fn clean(reply: &str, bot_names: &[&str], other_names: &[&str]) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let mut current = Speaker::Bot;
    let mut started = false;

    for line in reply.lines() {
        let (speaker, text) = split_labels(line, bot_names, other_names);
        match speaker {
            Some(Speaker::Bot) => current = Speaker::Bot,
            Some(Speaker::Other) if started => break,
            Some(Speaker::Other) => current = Speaker::Other,
            None => {}
        }
        if current == Speaker::Bot && (started || !text.trim().is_empty()) {
            kept.push(text);
            started = true;
        }
    }

    let cleaned = kept.join("\n").trim().to_owned();
    if cleaned.is_empty() {
        let (_, text) = split_labels(reply, bot_names, &[]);
        return text.trim().to_owned();
    }
    cleaned
}

/// Removes any "Name:" labels and "[#channel]" tags at the start of `line`
/// (there can be several, as in "RiceBot: [#general] Garrett: hi") and
/// returns who the last label names.
fn split_labels<'a>(
    mut line: &'a str,
    bot_names: &[&str],
    other_names: &[&str],
) -> (Option<Speaker>, &'a str) {
    let mut speaker = None;
    loop {
        let untagged = strip_channel_tag(line.trim_start());
        let tagged = untagged.len() != line.trim_start().len();
        let trimmed = untagged.trim_start();
        let found = bot_names
            .iter()
            .map(|name| (Speaker::Bot, *name))
            .chain(other_names.iter().map(|name| (Speaker::Other, *name)))
            .find_map(|(who, name)| strip_label(trimmed, name).map(|rest| (who, rest)));
        match found {
            Some((who, rest)) => {
                speaker = Some(who);
                line = rest;
            }
            None if tagged || speaker.is_some() => return (speaker, trimmed),
            None => return (speaker, line),
        }
    }
}

/// Removes a leading "[#channel]" tag, as added to messages when chat memory
/// is shared across a server.
pub fn strip_channel_tag(text: &str) -> &str {
    text.strip_prefix("[#")
        .and_then(|rest| rest.split_once(']'))
        .filter(|(channel, _)| !channel.is_empty() && channel.len() <= 100 && !channel.contains('\n'))
        .map_or(text, |(_, rest)| rest.trim_start())
}

/// If `text` starts with `name:` (ignoring case, optionally bold as
/// `**name:**` or `**name**:`), returns what follows it.
fn strip_label<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let (bold, text) = match text.strip_prefix("**") {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let head = text.get(..name.len())?;
    if !head.eq_ignore_ascii_case(name) {
        return None;
    }
    let mut rest = text[name.len()..].trim_start_matches(' ');
    if bold {
        rest = rest.strip_prefix("**").unwrap_or(rest);
    }
    let rest = rest.strip_prefix(':')?;
    let rest = if bold { rest.strip_prefix("**").unwrap_or(rest) } else { rest };
    // A label is followed by a space or the end of the line, which rules out
    // things like "Sam:3".
    (rest.is_empty() || rest.starts_with(char::is_whitespace)).then(|| rest.trim_start())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOT: &[&str] = &["RiceBot"];
    const OTHERS: &[&str] = &["Garrett", "Kevin"];

    fn run(reply: &str) -> String {
        clean(reply, BOT, OTHERS)
    }

    #[test]
    fn plain_reply_is_unchanged() {
        assert_eq!(run("Well shit, Garrett."), "Well shit, Garrett.");
        assert_eq!(run("line one\n\nline two"), "line one\n\nline two");
    }

    #[test]
    fn removes_echo_and_own_labels() {
        let reply = "RiceBot: Garrett: I've got spirit, boys.\n\nRiceBot: Well shit, Garrett.";
        assert_eq!(run(reply), "Well shit, Garrett.");
    }

    #[test]
    fn removes_leading_echo_without_bot_label() {
        assert_eq!(run("Garrett: hi\nRiceBot: hello"), "hello");
    }

    #[test]
    fn strips_own_label_and_is_case_insensitive() {
        assert_eq!(run("ricebot: hey sprout"), "hey sprout");
        assert_eq!(run("**RiceBot:** hey"), "hey");
        assert_eq!(run("**RiceBot**: hey"), "hey");
    }

    #[test]
    fn stops_when_writing_for_someone_else() {
        let reply = "rice. next question\nKevin: what about noodles\nRiceBot: no";
        assert_eq!(run(reply), "rice. next question");
    }

    #[test]
    fn keeps_multiline_bot_turns_and_drops_extra_labels() {
        let reply = "RiceBot: first\nstill me\nRiceBot: second";
        assert_eq!(run(reply), "first\nstill me\nsecond");
    }

    #[test]
    fn leaves_unknown_labels_and_mid_line_names_alone() {
        assert_eq!(run("Note: rice is life"), "Note: rice is life");
        assert_eq!(run("Ask Garrett: he knows"), "Ask Garrett: he knows");
        assert_eq!(run("Garrettson: hi"), "Garrettson: hi");
    }

    #[test]
    fn removes_channel_tags() {
        assert_eq!(run("[#general] RiceBot: hey"), "hey");
        assert_eq!(run("RiceBot: [#gaming] Garrett: gg\nRiceBot: nice"), "nice");
        assert_eq!(run("[#general] hey"), "hey");
        assert_eq!(run("[link](https://x.y) is cool"), "[link](https://x.y) is cool");
        assert_eq!(strip_channel_tag("[#gaming] Kevin: hi"), "Kevin: hi");
        assert_eq!(strip_channel_tag("Kevin: hi"), "Kevin: hi");
    }

    #[test]
    fn never_returns_nothing() {
        assert_eq!(run("RiceBot: Garrett: you're wrong"), "Garrett: you're wrong");
    }
}
