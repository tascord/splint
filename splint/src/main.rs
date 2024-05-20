use clap::Parser;
use itertools::Itertools;
use miette::{miette, NamedSource, Report};
use owo_colors::OwoColorize;
use proc_macro2::TokenTree;
use std::{fs, io::Error, str::FromStr, time::Instant};
use ty::{LintError, Named};

use crate::ty::Rules;
pub mod ty;

const RULES_FILES: [&str; 2] = ["splint.json", ".splint.json"];

#[derive(Parser, Debug)]
#[command(
    version = "1.0.0",
    about = "A simple linter to avoid pain in your codebases"
)]
struct Args {
    #[arg(short = 'r', help = "The rules to lint against (.json)")]
    rules: Option<String>,
    #[arg(name = "FILES", help = "The files to lint")]
    files: Vec<String>,
    #[arg(short = 'q', default_value = "false", help = "Quiet mode")]
    quiet: bool,
}

pub fn main() -> miette::Result<()> {
    let args = Args::parse();
    let rules_path = args.rules.unwrap_or_else(|| {
        let path = std::env::current_dir().unwrap();
        RULES_FILES
            .iter()
            .map(|f| path.join(f))
            .find(|f| f.exists())
            .unwrap_or_else(|| {
                eprintln!("{:?}", miette!("Couldn't find rules file in current directory. You can specify one with -r"));
                std::process::exit(1);
            })
            .to_str()
            .unwrap()
            .to_string()
    });

    let r: Rules = serde_json::from_str(
        fs::read_to_string(rules_path)
            .map_err(|e| miette!("Couldn't get rules: {:?}", e))?
            .as_str(),
    )
    .map_err(|e| miette!("Couldn't parse rules: {:?}", e))?;

    let files = args.files.iter().flat_map(|loc| {
        if !loc.contains('*') {
            vec![loc.to_string()]
        } else {
            glob::glob(&loc)
                .unwrap()
                .filter_map(Result::ok)
                .map(|p| p.into_os_string().to_str().unwrap().to_string())
                .collect::<Vec<_>>()
        }
    });

    let s = Instant::now();
    let (total, fails) = files
        .map(|loc| lint(loc, r.clone()))
        .try_collect::<(usize, usize), Vec<_>, std::io::Error>()
        .map_err(|e| miette!("Couldn't read source file: {:?}", e))?
        .into_iter()
        .reduce(|a, b| (a.0 + b.0, a.1 + b.1))
        .unwrap();

    if !args.quiet {
        println!(
            "{}, {}",
            format!("{fails} fails").red(),
            format!("{} warnings", total - fails).yellow()
        );
        println!(
            "Finished linting {} files in {}ms",
            total,
            s.elapsed().as_millis(),
        );
    }

    if fails > 0 {
        std::process::exit(1);
    }

    Ok(())
}

pub fn lint(loc: String, rules: Rules) -> Result<(usize, usize), Error> {
    let input = fs::read_to_string(loc.clone())?;
    let token_tree = proc_macro2::TokenStream::from_str(&input).unwrap();
    let named = token_tree
        .into_iter()
        .map(|tt| parse(tt))
        .flatten()
        .collect::<Vec<Named>>();

    Ok(test(rules, named, input.to_string(), loc))
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

fn test(r: Rules, s: Vec<Named>, source: String, file: String) -> (usize, usize) {
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

        eprintln!("{re:?}");
    });

    (any.clone().count(), any.filter(|a| a.0.fails).count())
}
