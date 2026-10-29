//! Command tokenizing and parsing.
//!
//! Player input is tokenized in-process and mapped to a typed [`Command`].
//! Input is *never* handed to a host shell. Quoted arguments are supported
//! (`"` and `'`); escape sequences inside quotes are not (documented subset).

use std::fmt;

/// A parsed command with validated arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    Pwd,
    Ls {
        path: Option<String>,
    },
    Cd {
        path: String,
    },
    Cat {
        paths: Vec<String>,
    },
    Grep {
        pattern: String,
        path: String,
        show_line_numbers: bool,
    },
}

/// Parse failures, rendered as terminal-style messages by `Display`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// No tokens in the input; the engine prints nothing.
    EmptyInput,
    UnterminatedQuote,
    UnknownCommand(String),
    MissingOperand {
        command: &'static str,
        usage: &'static str,
    },
    TooManyArgs {
        command: &'static str,
        usage: &'static str,
    },
    UnknownOption {
        command: &'static str,
        option: String,
        usage: &'static str,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::EmptyInput => Ok(()),
            ParseError::UnterminatedQuote => write!(f, "syntax error: unterminated quote"),
            ParseError::UnknownCommand(cmd) => {
                write!(
                    f,
                    "unknown command: {cmd}\nType 'help' to list supported commands."
                )
            }
            ParseError::MissingOperand { command, usage } => {
                write!(f, "{command}: missing operand\nusage: {usage}")
            }
            ParseError::TooManyArgs { command, usage } => {
                write!(f, "{command}: too many arguments\nusage: {usage}")
            }
            ParseError::UnknownOption {
                command,
                option,
                usage,
            } => {
                write!(f, "{command}: unsupported option: {option}\nusage: {usage}")
            }
        }
    }
}

impl std::error::Error for ParseError {}

const USAGE_LS: &str = "ls [PATH]";
const USAGE_CD: &str = "cd [PATH]";
const USAGE_CAT: &str = "cat PATH...";
const USAGE_GREP: &str = "grep [-n] PATTERN PATH";

/// Split input into tokens. Whitespace separates tokens unless it appears
/// inside single or double quotes. An unterminated quote is an error.
pub fn tokenize(input: &str) -> Result<Vec<String>, ParseError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' | '"' => {
                in_token = true;
                loop {
                    match chars.next() {
                        None => return Err(ParseError::UnterminatedQuote),
                        Some(quote_end) if quote_end == c => break,
                        Some(inner) => current.push(inner),
                    }
                }
            }
            c if c.is_whitespace() => {
                if in_token {
                    tokens.push(std::mem::take(&mut current));
                    in_token = false;
                }
            }
            c => {
                in_token = true;
                current.push(c);
            }
        }
    }
    if in_token {
        tokens.push(current);
    }
    Ok(tokens)
}

impl Command {
    /// Tokenize and validate player input into a typed command.
    pub fn parse(input: &str) -> Result<Command, ParseError> {
        let tokens = tokenize(input)?;
        let Some((name, rest)) = tokens.split_first() else {
            return Err(ParseError::EmptyInput);
        };
        match name.as_str() {
            "help" => no_operands("help", rest),
            "pwd" => no_operands("pwd", rest),
            "ls" => match rest {
                [] => Ok(Command::Ls { path: None }),
                [path] => Ok(Command::Ls {
                    path: Some(path.clone()),
                }),
                _ => Err(ParseError::TooManyArgs {
                    command: "ls",
                    usage: USAGE_LS,
                }),
            },
            "cd" => match rest {
                [] => Ok(Command::Cd {
                    path: "/".to_string(),
                }),
                [path] => Ok(Command::Cd { path: path.clone() }),
                _ => Err(ParseError::TooManyArgs {
                    command: "cd",
                    usage: USAGE_CD,
                }),
            },
            "cat" if !rest.is_empty() => Ok(Command::Cat {
                paths: rest.to_vec(),
            }),
            "cat" => Err(ParseError::MissingOperand {
                command: "cat",
                usage: USAGE_CAT,
            }),
            "grep" => parse_grep(rest),
            other => Err(ParseError::UnknownCommand(other.to_string())),
        }
    }
}

fn no_operands(command: &'static str, rest: &[String]) -> Result<Command, ParseError> {
    if rest.is_empty() {
        Ok(match command {
            "help" => Command::Help,
            _ => Command::Pwd,
        })
    } else {
        Err(ParseError::TooManyArgs {
            command,
            usage: command,
        })
    }
}

fn parse_grep(rest: &[String]) -> Result<Command, ParseError> {
    let mut show_line_numbers = false;
    let mut operands: Vec<&String> = Vec::new();
    for token in rest {
        if token.starts_with('-') && token.len() > 1 {
            for flag in token[1..].chars() {
                match flag {
                    'n' => show_line_numbers = true,
                    other => {
                        return Err(ParseError::UnknownOption {
                            command: "grep",
                            option: format!("-{other}"),
                            usage: USAGE_GREP,
                        });
                    }
                }
            }
        } else {
            operands.push(token);
        }
    }
    match operands.as_slice() {
        [pattern, path] => Ok(Command::Grep {
            pattern: (*pattern).clone(),
            path: (*path).clone(),
            show_line_numbers,
        }),
        // Zero or one operand means the PATH (or the pattern) is missing.
        [] | [_] => Err(ParseError::MissingOperand {
            command: "grep",
            usage: USAGE_GREP,
        }),
        _ => Err(ParseError::TooManyArgs {
            command: "grep",
            usage: USAGE_GREP,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_splits_on_whitespace() {
        assert_eq!(
            tokenize("  ls   /var/log  ").unwrap(),
            vec!["ls", "/var/log"]
        );
    }

    #[test]
    fn tokenize_supports_double_and_single_quotes() {
        assert_eq!(
            tokenize(r#"grep -n "failed login" /var/log/auth.log"#).unwrap(),
            vec!["grep", "-n", "failed login", "/var/log/auth.log"]
        );
        assert_eq!(
            tokenize("grep 'two words' file").unwrap(),
            vec!["grep", "two words", "file"]
        );
    }

    #[test]
    fn tokenize_keeps_empty_quoted_argument() {
        assert_eq!(tokenize("cat \"\"").unwrap(), vec!["cat", ""]);
    }

    #[test]
    fn tokenize_rejects_unterminated_quote() {
        assert_eq!(tokenize("cat \"oops"), Err(ParseError::UnterminatedQuote));
        assert_eq!(tokenize("cat 'oops"), Err(ParseError::UnterminatedQuote));
    }

    #[test]
    fn empty_input_is_empty_command() {
        assert_eq!(Command::parse("   "), Err(ParseError::EmptyInput));
        assert_eq!(Command::parse(""), Err(ParseError::EmptyInput));
    }

    #[test]
    fn parses_navigation_commands() {
        assert_eq!(Command::parse("pwd").unwrap(), Command::Pwd);
        assert_eq!(Command::parse("help").unwrap(), Command::Help);
        assert_eq!(
            Command::parse("help me").unwrap_err(),
            ParseError::TooManyArgs {
                command: "help",
                usage: "help"
            }
        );
        assert_eq!(Command::parse("ls").unwrap(), Command::Ls { path: None });
        assert_eq!(
            Command::parse("ls /var/log").unwrap(),
            Command::Ls {
                path: Some("/var/log".to_string())
            }
        );
        assert_eq!(
            Command::parse("cd ..").unwrap(),
            Command::Cd {
                path: "..".to_string()
            }
        );
        // Bare `cd` returns to the virtual root (no home directory exists).
        assert_eq!(
            Command::parse("cd").unwrap(),
            Command::Cd {
                path: "/".to_string()
            }
        );
    }

    #[test]
    fn parses_cat_with_multiple_paths() {
        assert_eq!(
            Command::parse("cat a.txt b.txt").unwrap(),
            Command::Cat {
                paths: vec!["a.txt".to_string(), "b.txt".to_string()]
            }
        );
    }

    #[test]
    fn parses_grep_flags_and_operands() {
        assert_eq!(
            Command::parse(r#"grep -n "failed login" /var/log/auth.log"#).unwrap(),
            Command::Grep {
                pattern: "failed login".to_string(),
                path: "/var/log/auth.log".to_string(),
                show_line_numbers: true,
            }
        );
        assert_eq!(
            Command::parse("grep login auth.log").unwrap(),
            Command::Grep {
                pattern: "login".to_string(),
                path: "auth.log".to_string(),
                show_line_numbers: false,
            }
        );
    }

    #[test]
    fn rejects_invalid_commands() {
        assert_eq!(
            Command::parse("rm -rf /"),
            Err(ParseError::UnknownCommand("rm".to_string()))
        );
        assert_eq!(
            Command::parse("cat"),
            Err(ParseError::MissingOperand {
                command: "cat",
                usage: USAGE_CAT
            })
        );
        assert_eq!(
            Command::parse("cd /a /b"),
            Err(ParseError::TooManyArgs {
                command: "cd",
                usage: USAGE_CD
            })
        );
        assert_eq!(
            Command::parse("ls /a /b"),
            Err(ParseError::TooManyArgs {
                command: "ls",
                usage: USAGE_LS
            })
        );
        assert_eq!(
            Command::parse("grep -x a b"),
            Err(ParseError::UnknownOption {
                command: "grep",
                option: "-x".to_string(),
                usage: USAGE_GREP
            })
        );
        assert_eq!(
            Command::parse("grep onlypattern"),
            Err(ParseError::MissingOperand {
                command: "grep",
                usage: USAGE_GREP
            })
        );
    }

    #[test]
    fn display_renders_terminal_style_messages() {
        let msg = ParseError::UnknownCommand("rm".to_string()).to_string();
        assert!(msg.starts_with("unknown command: rm"));
        let msg = ParseError::MissingOperand {
            command: "cat",
            usage: USAGE_CAT,
        }
        .to_string();
        assert_eq!(msg, "cat: missing operand\nusage: cat PATH...");
    }
}
