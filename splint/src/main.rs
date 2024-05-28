use clap::Parser;
use itertools::Itertools;
use miette::{bail, miette, Report};
use owo_colors::OwoColorize;
use proc_macro2::TokenTree;
use std::{fs, io::Error, process::Command, str::FromStr, time::Instant};
use ty::{LintError, Named};

use crate::ty::Rules;
use splint::*;

const RULES_FILES: [&str; 4] = ["splint.json", ".splint.json", "splint.toml", ".splint.toml"];

#[derive(Parser, Debug, Clone)]
#[command(
    version = "1.0.0",
    about = "A simple linter to avoid pain in your codebases",
    ignore_errors = true
)]
struct Args {
    #[arg(short = 'r', help = "The rules to lint against (json|toml)")]
    rules: Option<String>,
    #[arg(name = "FILES", help = "The files to lint")]
    files: Vec<String>,
    #[arg(short = 'q', default_value = "false", help = "Quiet mode")]
    quiet: bool,
    #[arg(short = 'a', default_value = "false", help = "RustAnalyzer mode")]
    analyze: bool,
}

pub fn main() {
    let args: Args = Args::parse();
    match cli(args.clone()) {
        Ok((violations, file_count, ms)) => {
            if args.analyze {
                violations
                    .into_iter()
                    .map(|e| serde_json::to_string(&e.json_diagnostic()).unwrap())
                    .for_each(|f| {
                        println!("{}", f);
                    });

                Command::new(env!("CARGO"))
                    .arg("check")
                    .arg("--quiet")
                    .arg("--workspace")
                    .arg("--message-format=json")
                    .arg("--all-targets")
                    .status()
                    .unwrap();

                std::process::exit(0);
            } else {
                let fails = violations.iter().filter(|a| a.fails);
                if !args.quiet {
                    violations
                        .clone()
                        .into_iter()
                        .map(Report::new)
                        .for_each(|r| {
                            eprintln!("{r:?}");
                        });

                    println!(
                        "{}, {}",
                        format!("{} fails", fails.clone().count()).red(),
                        format!("{} warnings", violations.len()).yellow()
                    );
                    println!("Finished linting {} files in {}ms", file_count, ms);
                }

                if fails.count() > 0 {
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("{e:?}");
            std::process::exit(1);
        }
    }
}

fn cli(args: Args) -> miette::Result<(Vec<LintError>, usize, u128)> {
    let rules_path = args.rules.unwrap_or_else(|| {
        let path = std::env::current_dir().unwrap();
        RULES_FILES
            .iter()
            .map(|f| path.join(f))
            .find(|f| f.exists())
            .unwrap_or_else(|| {
                if !args.quiet {
                    eprintln!("{:?}", miette!("Couldn't find rules file in current directory. You can specify one with -r"));
                }
                std::process::exit(1);
            })
            .to_str()
            .unwrap()
            .to_string()
    });

    let r: Rules = {
        let content = fs::read_to_string(rules_path.clone())
            .map_err(|e| miette!("Couldn't read rules: {:?}", e))?;
        if rules_path.ends_with(".toml") {
            toml::from_str(&content).map_err(|e| miette!("Couldn't parse rules: {:?}", e))?
        } else {
            serde_json::from_str(&content).map_err(|e| miette!("Couldn't parse rules: {:?}", e))?
        }
    };

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

    if files.clone().count() == 0 {
        bail!(miette!("No files provided."))
    }

    let s: Instant = Instant::now();
    let violations = files
        .clone()
        .map(|f| lint(f, r.clone()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| miette!("Error linting files: {:?}", e))?
        .into_iter()
        .flatten()
        .collect_vec();

    Ok((violations, files.count(), s.elapsed().as_millis()))
}

pub fn lint(loc: String, rules: Rules) -> Result<Vec<LintError>, Error> {
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
