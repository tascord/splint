use std::{collections::HashMap, fmt::Debug};

use proc_macro2::{Delimiter, Span, TokenTree};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub(crate) struct Named(String, String, Span);
impl Debug for Named {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("{}(\"{}\")", self.0, self.1).as_str())
    }
}

impl Into<Named> for proc_macro2::Ident {
    fn into(self) -> Named {
        Named("Ident".to_string(), self.to_string(), self.span())
    }
}

impl Into<Named> for proc_macro2::Punct {
    fn into(self) -> Named {
        Named("Punct".to_string(), self.to_string(), self.span())
    }
}

impl Into<Named> for proc_macro2::Literal {
    fn into(self) -> Named {
        Named("Literal".to_string(), self.to_string(), self.span())
    }
}

impl Into<Named> for proc_macro2::TokenTree {
    fn into(self) -> Named {
        match self {
            TokenTree::Ident(t) => t.into(),
            TokenTree::Punct(t) => t.into(),
            TokenTree::Literal(t) => t.into(),
            TokenTree::Group(_) => panic!(),
        }
    }
}

impl Named {
    pub fn delim_pair(d: Delimiter, s1: Span, s2: Span) -> [Named; 2] {
        let [a, b] = match_delim(d);
        [
            Named("Delim".to_string(), a.to_string(), s1),
            Named("Delim".to_string(), b.to_string(), s2),
        ]
    }

    pub fn span(&self) -> Span {
        self.2.clone()
    }
}

fn match_delim(d: Delimiter) -> [char; 2] {
    match d {
        Delimiter::Parenthesis => ['(', ')'],
        Delimiter::Brace => ['{', '}'],
        Delimiter::Bracket => ['[', ']'],
        Delimiter::None => [' ', ' '],
    }
}

/* ----------------- */

#[derive(Deserialize, Serialize)]
pub(crate) struct Needle(pub String, pub Option<String>);
impl Debug for Needle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!(
            "{}(\"{}\")",
            self.0,
            self.1.clone().unwrap_or_default()
        ))
    }
}

impl Needle {
    pub fn test(&self, s: &Named) -> bool {
        s.0 == self.0 && {
            if let Some(v) = &self.1 {
                if v.starts_with('/') && v.ends_with('/') {
                    let re = regex::Regex::new(v).unwrap();
                    re.is_match(&s.1)
                } else {
                    s.1 == *v
                }
            } else {
                true
            }
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(transparent)]
pub(crate) struct Rule(pub Vec<Needle>);
impl Rule {
    pub fn test(&self, s: &Vec<Named>) -> Result<(), Vec<Named>> {
        for (m, _) in s
            .iter()
            .enumerate()
            .filter(|(_, v)| self.0.first().unwrap().0 == v.0)
        {
            let window = s.iter().skip(m).take(self.0.len());
            if window.len() != self.0.len() {
                continue;
            }

            if window.clone().zip(self.0.iter()).all(|(a, b)| b.test(a)) {
                return Err(window.cloned().collect());
            }
        }

        Ok(())
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct Rules {
    pub rules: HashMap<String, Rule>,
}
