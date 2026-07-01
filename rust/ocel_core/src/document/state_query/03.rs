
fn compare_ordered(left: &QueryValue, right: &QueryValue) -> Option<i8> {
    match (query_value_as_number(left), query_value_as_number(right)) {
        (Some(left), Some(right)) => {
            if left < right {
                Some(-1)
            } else if left > right {
                Some(1)
            } else {
                Some(0)
            }
        }
        _ => {
            let left = left.as_state_string();
            let right = right.as_state_string();
            match left.cmp(&right) {
                std::cmp::Ordering::Less => Some(-1),
                std::cmp::Ordering::Equal => Some(0),
                std::cmp::Ordering::Greater => Some(1),
            }
        }
    }
}

fn query_value_as_number(value: &QueryValue) -> Option<f64> {
    match value {
        QueryValue::Number(value) => Some(*value),
        QueryValue::String(value) => value.trim().parse::<f64>().ok(),
        QueryValue::Boolean(_) => None,
    }
}

fn sql_like(value: &str, pattern: &str) -> bool {
    if pattern == "%" {
        return true;
    }

    let parts = pattern.split('%').collect::<Vec<_>>();
    if parts.len() == 1 {
        return value == pattern;
    }

    let mut remainder = value;
    let starts_with_wildcard = pattern.starts_with('%');
    let ends_with_wildcard = pattern.ends_with('%');

    for (index, part) in parts.iter().filter(|part| !part.is_empty()).enumerate() {
        if index == 0 && !starts_with_wildcard {
            if !remainder.starts_with(part) {
                return false;
            }
            remainder = &remainder[part.len()..];
            continue;
        }

        let Some(position) = remainder.find(part) else {
            return false;
        };
        remainder = &remainder[position + part.len()..];
    }

    ends_with_wildcard
        || parts
            .last()
            .is_none_or(|last| last.is_empty() || remainder.is_empty())
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Identifier(String),
    String(String),
    Number(f64),
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Dot,
    LParen,
    RParen,
    Semicolon,
}

fn tokenize(input: &str) -> OcelResult<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();

    while let Some((index, character)) = chars.next() {
        match character {
            character if character.is_whitespace() => {}
            '-' if chars.peek().is_some_and(|(_, next)| *next == '-') => {
                for (_, next) in chars.by_ref() {
                    if next == '\n' {
                        break;
                    }
                }
            }
            '\'' | '"' => {
                let quote = character;
                let mut value = String::new();
                let mut closed = false;
                while let Some((_, next)) = chars.next() {
                    if next == quote {
                        if chars.peek().is_some_and(|(_, repeated)| *repeated == quote) {
                            chars.next();
                            value.push(quote);
                        } else {
                            closed = true;
                            break;
                        }
                    } else {
                        value.push(next);
                    }
                }
                if !closed {
                    return Err(OcelError::new("unterminated string literal in state query"));
                }
                tokens.push(Token::String(value));
            }
            '=' => tokens.push(Token::Eq),
            '!' if chars.peek().is_some_and(|(_, next)| *next == '=') => {
                chars.next();
                tokens.push(Token::Ne);
            }
            '<' if chars.peek().is_some_and(|(_, next)| *next == '=') => {
                chars.next();
                tokens.push(Token::Le);
            }
            '<' if chars.peek().is_some_and(|(_, next)| *next == '>') => {
                chars.next();
                tokens.push(Token::Ne);
            }
            '<' => tokens.push(Token::Lt),
            '>' if chars.peek().is_some_and(|(_, next)| *next == '=') => {
                chars.next();
                tokens.push(Token::Ge);
            }
            '>' => tokens.push(Token::Gt),
            '.' => tokens.push(Token::Dot),
            '(' => tokens.push(Token::LParen),
            ')' => tokens.push(Token::RParen),
            ';' => tokens.push(Token::Semicolon),
            character if character.is_ascii_digit() || character == '-' => {
                let mut literal = character.to_string();
                while let Some((_, next)) = chars.peek() {
                    if next.is_ascii_digit() || *next == '.' {
                        literal.push(*next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let number = literal.parse::<f64>().map_err(|err| {
                    OcelError::new(format!("invalid numeric literal '{literal}': {err}"))
                })?;
                tokens.push(Token::Number(number));
            }
            character if is_identifier_start(character) => {
                let mut identifier = character.to_string();
                while let Some((_, next)) = chars.peek() {
                    if is_identifier_part(*next) {
                        identifier.push(*next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Identifier(identifier));
            }
            other => {
                return Err(OcelError::new(format!(
                    "unexpected character '{other}' at byte {index} in state query"
                )));
            }
        }
    }

    Ok(tokens)
}

fn is_identifier_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_'
}

fn is_identifier_part(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '-'
}
