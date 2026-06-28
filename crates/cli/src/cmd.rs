//! Command parser for lyra-cli's REPL.
//!
//! `parse(line)` is a pure function — no I/O, no state — so it is easy to
//! unit-test in isolation.

/// Parsed REPL command.
#[derive(Debug, PartialEq)]
pub enum Command {
    /// `scan [dir]`
    Scan(Option<String>),
    /// `list [n]`
    List(usize),
    /// `search <query>`
    Search(String),
    /// `play <i>` (1-based index)
    Play(usize),
    Pause,
    Resume,
    Stop,
    Next,
    Prev,
    Status,
    Help,
    Quit,
    Unknown(String),
}

/// Parse a trimmed input line into a `Command`.
pub fn parse(line: &str) -> Command {
    let line = line.trim();
    // Split on the first whitespace to separate the verb from the rest.
    let (verb, rest) = match line.find(|c: char| c.is_whitespace()) {
        Some(pos) => (&line[..pos], line[pos..].trim()),
        None => (line, ""),
    };

    match verb.to_lowercase().as_str() {
        "scan" => {
            let dir = if rest.is_empty() {
                None
            } else {
                Some(rest.to_string())
            };
            Command::Scan(dir)
        }
        "list" => {
            let n = if rest.is_empty() {
                30
            } else {
                rest.parse::<usize>().unwrap_or(30)
            };
            Command::List(n)
        }
        "search" => {
            if rest.is_empty() {
                Command::Unknown("search requires a query".to_string())
            } else {
                Command::Search(rest.to_string())
            }
        }
        "play" => match rest.parse::<usize>() {
            Ok(i) if i > 0 => Command::Play(i),
            _ => Command::Unknown(format!("play requires a positive integer, got {rest:?}")),
        },
        "pause" => Command::Pause,
        "resume" => Command::Resume,
        "stop" => Command::Stop,
        "next" => Command::Next,
        "prev" | "previous" => Command::Prev,
        "status" => Command::Status,
        "help" | "?" => Command::Help,
        "quit" | "exit" | "q" => Command::Quit,
        other => Command::Unknown(other.to_string()),
    }
}

// ── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_play_with_index() {
        assert_eq!(parse("play 3"), Command::Play(3));
    }

    #[test]
    fn parse_play_with_leading_whitespace() {
        assert_eq!(parse("  play 1  "), Command::Play(1));
    }

    #[test]
    fn parse_search_multi_word() {
        assert_eq!(parse("search hello world"), Command::Search("hello world".to_string()));
    }

    #[test]
    fn parse_scan_no_arg() {
        assert_eq!(parse("scan"), Command::Scan(None));
    }

    #[test]
    fn parse_scan_with_dir() {
        assert_eq!(
            parse("scan ~/Music"),
            Command::Scan(Some("~/Music".to_string()))
        );
    }

    #[test]
    fn parse_quit() {
        assert_eq!(parse("quit"), Command::Quit);
    }

    #[test]
    fn parse_exit_alias() {
        assert_eq!(parse("exit"), Command::Quit);
    }

    #[test]
    fn parse_unknown_command() {
        // An unrecognised verb should yield Unknown, not panic.
        match parse("frobble") {
            Command::Unknown(_) => {}
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn parse_list_default() {
        assert_eq!(parse("list"), Command::List(30));
    }

    #[test]
    fn parse_list_with_n() {
        assert_eq!(parse("list 5"), Command::List(5));
    }

    #[test]
    fn parse_pause_resume_stop() {
        assert_eq!(parse("pause"), Command::Pause);
        assert_eq!(parse("resume"), Command::Resume);
        assert_eq!(parse("stop"), Command::Stop);
    }

    #[test]
    fn parse_next_prev() {
        assert_eq!(parse("next"), Command::Next);
        assert_eq!(parse("prev"), Command::Prev);
        assert_eq!(parse("previous"), Command::Prev);
    }

    #[test]
    fn parse_status_and_help() {
        assert_eq!(parse("status"), Command::Status);
        assert_eq!(parse("help"), Command::Help);
        assert_eq!(parse("?"), Command::Help);
    }

    #[test]
    fn parse_case_insensitive() {
        assert_eq!(parse("PLAY 2"), Command::Play(2));
        assert_eq!(parse("QUIT"), Command::Quit);
    }

    #[test]
    fn parse_play_zero_is_unknown() {
        // Index 0 is not valid (1-based).
        match parse("play 0") {
            Command::Unknown(_) => {}
            other => panic!("expected Unknown for play 0, got {other:?}"),
        }
    }

    #[test]
    fn parse_play_no_arg_is_unknown() {
        match parse("play") {
            Command::Unknown(_) => {}
            other => panic!("expected Unknown for `play` with no arg, got {other:?}"),
        }
    }
}
