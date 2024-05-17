use std::{collections::HashMap, str::FromStr};

use proc_macro2::TokenTree;
use ty::Named;

use crate::ty::{Needle, Rule, Rules};

const INPUT: &str = include_str!("../input.rs");
// const RULES: &str = include_str!("../rules.toml");

pub mod ty;

pub fn main() {
    let token_tree = proc_macro2::TokenStream::from_str(INPUT).unwrap();
    let named = token_tree
        .into_iter()
        .map(|tt| parse(tt))
        .flatten()
        .collect::<Vec<Named>>();

    let mut m = HashMap::<String, Rule>::new();
    m.insert(
        "Disallow Unwrap".to_string(),
        Rule(vec![
            Needle("Ident".to_string(), None),
            Needle("Punct".to_string(), Some(".".to_string())),
            Needle("Ident".to_string(), Some("unwrap".to_string())),
            Needle("Delim".to_string(), Some("(".to_string())),
            Needle("Delim".to_string(), Some(")".to_string())),
        ]),
    );

    let r = Rules { rules: m };
    test(r, named);
    // println!("{:?}", toml::from_str::<Rules>(RULES).unwrap());
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

fn test(r: Rules, s: Vec<Named>) {
    let any = r
        .rules
        .iter()
        .map(|(n, v)| (n, v.test(&s)))
        .filter(|(_, r)| r.is_err());

    any.clone().for_each(|(n, r)| {
        println!("Lint {} failed:", n);
        r.unwrap_err()
            .iter()
            .for_each(|e| spanned_er);
    });

    if any.clone().count() == 0 {
        println!("Looks good.")
    } else {
        panic!("Some lints failed.")
    }
}
