//! Slash commands in the composer. Local commands act in the app; prompt
//! commands expand to an instruction for Evee and show as a chip in the transcript.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Local,
    Prompt,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlashCommand {
    pub name: &'static str,
    pub argument: Option<&'static str>,
    pub summary: &'static str,
    pub kind: Kind,
}
pub const COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: "new",
        argument: None,
        summary: "Start a new conversation",
        kind: Kind::Local,
    },
    SlashCommand {
        name: "model",
        argument: Some("name"),
        summary: "Choose the model for new replies",
        kind: Kind::Local,
    },
    SlashCommand {
        name: "clear",
        argument: None,
        summary: "Clear this conversation and start over",
        kind: Kind::Local,
    },
    SlashCommand {
        name: "help",
        argument: None,
        summary: "Show what Evee can do",
        kind: Kind::Local,
    },
    SlashCommand {
        name: "tickets",
        argument: None,
        summary: "List your Tickets and their states",
        kind: Kind::Prompt,
    },
    SlashCommand {
        name: "ticket",
        argument: Some("title"),
        summary: "Create a Ticket",
        kind: Kind::Prompt,
    },
    SlashCommand {
        name: "runs",
        argument: None,
        summary: "Show the latest runs",
        kind: Kind::Prompt,
    },
    SlashCommand {
        name: "automations",
        argument: None,
        summary: "List your Automations",
        kind: Kind::Prompt,
    },
    SlashCommand {
        name: "http",
        argument: Some("url"),
        summary: "Fetch a URL with http_request",
        kind: Kind::Prompt,
    },
];

/// Commands matching what the user has typed so far, or none when the text
/// is not a command in progress.
pub fn suggestions(input: &str) -> Vec<&'static SlashCommand> {
    let Some(rest) = input.strip_prefix('/') else {
        return Vec::new();
    };
    if rest.contains(char::is_whitespace) || rest.contains('\n') {
        return Vec::new();
    }
    let needle = rest.to_ascii_lowercase();
    COMMANDS
        .iter()
        .filter(|command| command.name.starts_with(&needle))
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Parsed {
    New,
    Clear,
    Help,
    Model(Option<String>),
    Prompt { command: String, prompt: String },
    Plain(String),
    Unknown(String),
}
pub fn parse(input: &str) -> Parsed {
    let text = input.trim();
    let Some(rest) = text.strip_prefix('/') else {
        return Parsed::Plain(text.to_owned());
    };
    let (name, argument) = match rest.split_once(char::is_whitespace) {
        Some((name, argument)) => (name, argument.trim()),
        None => (rest, ""),
    };
    let Some(command) = COMMANDS.iter().find(|c| c.name.eq_ignore_ascii_case(name)) else {
        return Parsed::Unknown(format!("/{name}"));
    };
    let shown = if argument.is_empty() {
        format!("/{}", command.name)
    } else {
        format!("/{} {argument}", command.name)
    };
    match command.name {
        "new" => Parsed::New,
        "clear" => Parsed::Clear,
        "help" => Parsed::Help,
        "model" => Parsed::Model((!argument.is_empty()).then(|| argument.to_owned())),
        "tickets" => Parsed::Prompt { command: shown, prompt: "List my Tickets grouped by state, with assignees, and point out anything blocked or overdue.".into() },
        "ticket" => Parsed::Prompt {
            command: shown,
            prompt: if argument.is_empty() {
                "Ask me for a title, then create a Ticket in Backlog.".into()
            } else {
                format!("Create a Ticket titled \"{argument}\" in Backlog and confirm its ID.")
            },
        },
        "runs" => Parsed::Prompt { command: shown, prompt: "List the latest runs with their status and timing, and flag any failures.".into() },
        "automations" => Parsed::Prompt { command: shown, prompt: "List my Automations with their schedules and the last time each fired.".into() },
        "http" => Parsed::Prompt {
            command: shown,
            prompt: if argument.is_empty() {
                "Ask me which URL to fetch, then fetch it with http_request and summarize the response.".into()
            } else {
                format!("Fetch {argument} with http_request and summarize the response, including the status code.")
            },
        },
        _ => Parsed::Unknown(shown),
    }
}
pub const HELP: &str = "**Evee** coordinates your AgentInc: Tickets, Automations, Runs and the web.\n\nType a message, or start with `/` for commands:\n\n- `/tickets`, `/ticket <title>`, `/runs`, `/automations`, `/http <url>` ask Evee to use a tool\n- `/model` picks the model, `/new` starts fresh, `/clear` clears this conversation\n\nEvery reply, tool call and result is stored and runs durably, so closing the window never loses work.";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn suggestions_follow_the_typed_prefix() {
        assert_eq!(suggestions("hello").len(), 0);
        assert_eq!(suggestions("/").len(), COMMANDS.len());
        let matched: Vec<_> = suggestions("/ti").iter().map(|c| c.name).collect();
        assert_eq!(matched, vec!["tickets", "ticket"]);
        assert_eq!(suggestions("/tickets now").len(), 0);
        assert_eq!(suggestions("/HT").len(), 1);
    }
    #[test]
    fn commands_parse_into_actions_and_prompts() {
        assert_eq!(parse("/new"), Parsed::New);
        assert_eq!(parse("/model sonnet"), Parsed::Model(Some("sonnet".into())));
        assert_eq!(parse("/model"), Parsed::Model(None));
        assert_eq!(parse("/nope"), Parsed::Unknown("/nope".into()));
        assert_eq!(parse("plain text"), Parsed::Plain("plain text".into()));
        let Parsed::Prompt { command, prompt } = parse("/http https://example.test/x") else {
            panic!("prompt")
        };
        assert_eq!(command, "/http https://example.test/x");
        assert!(prompt.contains("https://example.test/x"));
        let Parsed::Prompt { command, .. } = parse("/ticket  Fix the login  ") else {
            panic!("prompt")
        };
        assert_eq!(command, "/ticket Fix the login");
    }
}
