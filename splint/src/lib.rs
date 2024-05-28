#![allow(ambiguous_associated_items)]

use miette::NamedSource;
use ty::{LintError, Named, Rule, Rules};

pub mod compiler;
pub mod ty;

/// Finds all matches for a rule in any given source token list
pub fn match_rule(rule: Rule, tokens: &Vec<Named>) -> Vec<(Rule, Vec<Named>)> {
    let mut out = Vec::new();
    if tokens.len() < rule.pattern.len() {
        return out;
    }

    match rule.test(tokens) {
        Ok(_) => {}
        Err(e) => {
            out.push((rule.clone(), e.clone()));

            let line = e.last().unwrap().span().end().line;
            let col = e.last().unwrap().span().end().column;

            let more = tokens
                .iter()
                .skip_while(|v| {
                    let span_line = v.span().start().line;
                    if span_line < line {
                        return true;
                    }

                    if span_line == line {
                        return v.span().start().column < col;
                    }

                    false
                })
                .map(|v| v.clone());

            out.extend(match_rule(rule.clone(), &more.clone().collect()));
        }
    }

    out
}

/// Tests a set of rules against a source file
pub fn test(rules: Rules, tokens: Vec<Named>, source: String, file_name: String) -> Vec<LintError> {
    let any = rules
        .rules
        .iter()
        .flat_map(|(_, v)| match_rule(v.clone(), &tokens))
        .collect::<Vec<_>>();

    let errors = any.iter().map(|(n, r)| LintError {
        window: r.clone(),
        fails: n.fails,
        rule: n.clone(),
        line: (
            source
                .lines()
                .nth(r.clone().first().unwrap().span().start().line - 1)
                .unwrap_or_default()
                .to_string(),
            // find chars before line
            source
                .lines()
                .enumerate()
                .filter(|(i, _)| *i < r.clone().first().unwrap().span().start().line - 1)
                .map(|(_, l)| l.chars().count())
                .reduce(|a, b| a + b)
                .unwrap_or_default(),
        ),
        source: NamedSource::new(&file_name, source.clone()),
    });

    errors.collect::<Vec<_>>()
}
