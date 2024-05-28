use std::{
    collections::HashMap,
    error::Error,
    fmt::{Debug, Display},
    ops::RangeInclusive,
    sync::Arc,
};

use miette::{Diagnostic, LabeledSpan, NamedSource, SourceOffset, SourceSpan};
use proc_macro2::{Delimiter, Span, TokenTree};
use serde::{de, Deserialize, Serialize};

#[derive(Clone)]
pub struct Named(String, String, Arc<Span>);
impl Debug for Named {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("{}(\"{}\")", self.0, self.1).as_str())
    }
}

unsafe impl Send for Named {}
unsafe impl Sync for Named {}

impl Into<Named> for proc_macro2::Ident {
    fn into(self) -> Named {
        Named("Ident".to_string(), self.to_string(), self.span().into())
    }
}

impl Into<Named> for proc_macro2::Punct {
    fn into(self) -> Named {
        Named("Punct".to_string(), self.to_string(), self.span().into())
    }
}

impl Into<Named> for proc_macro2::Literal {
    fn into(self) -> Named {
        Named("Literal".to_string(), self.to_string(), self.span().into())
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
            Named("Delim".to_string(), a.to_string(), s1.into()),
            Named("Delim".to_string(), b.to_string(), s2.into()),
        ]
    }

    pub fn span(&self) -> Arc<Span> {
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

#[derive(Deserialize, Serialize, Clone)]
pub struct Needle(pub String, pub Option<String>);
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

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Rule {
    pub name: String,
    pub description: String,
    pub help: Option<String>,
    #[serde(deserialize_with = "deser_range_from_array")]
    pub range: RangeInclusive<usize>,
    pub pattern: Vec<Needle>,
    pub link: Option<String>,
    #[serde(default)]
    pub fails: bool,
}

impl Rule {
    pub fn test(&self, s: &Vec<Named>) -> Result<(), Vec<Named>> {
        for (m, _) in s
            .iter()
            .enumerate()
            .filter(|(_, v)| self.pattern.first().unwrap().0 == v.0)
        {
            let window = s.iter().skip(m).take(self.pattern.len());
            if window.len() != self.pattern.len() {
                continue;
            }

            if window
                .clone()
                .zip(self.pattern.iter())
                .all(|(a, b)| b.test(a))
            {
                return Err(window.cloned().collect());
            }
        }

        Ok(())
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Rules {
    pub rules: HashMap<String, Rule>,
}

#[derive(Debug, Clone)]
pub struct LintError {
    pub rule: Rule,
    pub fails: bool,
    pub line: (String, usize),
    pub window: Vec<Named>,
    pub source: NamedSource<String>,
}

impl Display for LintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{}: {}", self.rule.name, self.rule.description))
    }
}

impl Error for LintError {}
impl Diagnostic for LintError {
    fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        Some(Box::new(self.rule.name.clone()))
    }

    fn severity(&self) -> Option<miette::Severity> {
        Some(match self.fails {
            true => miette::Severity::Error,
            false => miette::Severity::Warning,
        })
    }

    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        if let Some(v) = &self.rule.help {
            Some(Box::new(v.clone()))
        } else {
            None
        }
    }

    fn url<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        if let Some(v) = &self.rule.link {
            Some(Box::new(v.clone()))
        } else {
            None
        }
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.source)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        Some(Box::new(
            [LabeledSpan::new_primary_with_span(
                None,
                span(
                    self.source.inner().to_string(),
                    self.window.clone(),
                    self.rule.range.clone(),
                ),
            )]
            .into_iter(),
        ))
    }

    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>> {
        None
    }

    fn diagnostic_source(&self) -> Option<&dyn Diagnostic> {
        None
    }
}

pub fn span(c: String, s: Vec<Named>, r: RangeInclusive<usize>) -> SourceSpan {
    let s = s[r.clone()].iter().map(|v| v.span()).collect::<Vec<_>>();
    let f = s.first().unwrap();
    let lc = f.start();
    let length = s
        .clone()
        .iter()
        .map(|v| v.byte_range().end - v.byte_range().start)
        .sum::<usize>();

    SourceSpan::new(
        SourceOffset::from_location(c, lc.line, lc.column + 1),
        length,
    )
}

pub fn deser_range_from_array<'de, D>(deserializer: D) -> Result<RangeInclusive<usize>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v: Vec<usize> = Vec::deserialize(deserializer)
        .map_err(|_| de::Error::custom("Invalid range, expected 2 element array"))?;
    if v.len() != 2 {
        return Err(de::Error::custom("Invalid range, expected 2 element array"));
    }
    Ok(v[0]..=v[1])
}
