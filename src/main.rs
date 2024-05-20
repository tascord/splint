#![feature(proc_macro_diagnostic)]
use miette::{NamedSource, Report};
use proc_macro2::TokenTree;
use std::{fs, str::FromStr};
use ty::{LintError, Named};

use crate::ty::Rules;

pub mod ty;

pub fn main() {
    let r: Rules = serde_json::from_str(include_str!("../rules.json")).unwrap();
    std::env::args().skip(1).for_each(|loc| {
        if !loc.contains('*') {
            lint(loc, r.clone())
        } else {
            glob::glob(&loc)
                .unwrap()
                .filter_map(Result::ok)
                .for_each(|p| lint(p.into_os_string().to_str().unwrap().to_string(), r.clone()))
        }
    });
}

fn lint(loc: String, rules: Rules) {
    let input = fs::read_to_string(loc.clone()).unwrap();
    let token_tree = proc_macro2::TokenStream::from_str(&input).unwrap();
    let named = token_tree
        .into_iter()
        .map(|tt| parse(tt))
        .flatten()
        .collect::<Vec<Named>>();

    test(rules, named, input.to_string(), loc);
}

fn parse(tt: TokenTree) -> Vec<Named> {
    match tt {
        TokenTree::Group(g) => {
            let delim = Named::delim_pair(g.delimiter(), g.span_open(), g.span_close());
            let mut body = vec![delim[0].clone()];

            g.stream()
                .into_iter()
                .map(|tt| parse(tt))
                .for_each(|v| body.extend(v));

            body.push(delim[1].clone());
            body
        }
        _ => vec![tt.into()],
    }
}

fn test(r: Rules, s: Vec<Named>, source: String, file: String) {
    let any = r
        .rules
        .iter()
        .map(|(_, v)| (v, v.test(&s)))
        .filter(|(_, r)| r.is_err());

    any.clone().for_each(|(n, r)| {
        let window = r.unwrap_err();
        let re = Report::new(LintError {
            window,
            fails: n.fails,
            rule: n.clone(),
            source: NamedSource::new(&file, source.clone()),
        });

        if n.fails {
            panic!("{:?}", re);
        } else {
            println!("{:?}", re)
        }
    });
}
