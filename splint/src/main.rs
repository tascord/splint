use clap::Parser;
use itertools::Itertools;
use miette::{bail, miette, NamedSource, Report};
use owo_colors::OwoColorize;
use proc_macro2::TokenTree;
use std::{fs, io::Error, process::Command, str::FromStr, time::Instant};
use ty::{LintError, Named, Rule};

use crate::ty::Rules;
mod compiler;
pub mod ty;

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
    #[arg(
        short = 'q',
        default_value = "false",
        help = "Quiet mode",
        required = false
    )]
    quiet: bool,
    #[arg(short = 'a', default_value = "false", help = "RustAnalyzer mode")]
    analyze: bool,
}

pub fn main() {
    let args: Args = Args::parse();
    match cli(args.clone()) {
        Ok(a) => {
            if args.analyze {
                a.into_iter()
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
                let fail = a.iter().any(|a| a.fails);
                a.into_iter().map(Report::new).for_each(|r| {
                    eprintln!("{r:?}");
                });

                if fail {
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            if !args.quiet {
                eprintln!("{e:?}");
                std::process::exit(1);
            }
        }
    }
}

fn cli(args: Args) -> miette::Result<Vec<LintError>> {
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

    let s = Instant::now();
    let (violations, total, fails) = files
        .map(|loc| lint(loc, r.clone()))
        .try_collect::<(Vec<LintError>, usize), Vec<_>, std::io::Error>()
        .map_err(|e| miette!("Couldn't read source file: {:?}", e))?
        .into_iter()
        .fold((Vec::new(), 0, 0), |mut a, b| {
            (
                {
                    a.0.extend(b.0.clone());
                    a.0
                },
                a.1 + b.0.len(),
                a.2 + b.1,
            )
        });

    if !(args.quiet || args.analyze) {
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

    Ok(violations)
}

pub fn lint(loc: String, rules: Rules) -> Result<(Vec<LintError>, usize), Error> {
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

fn walk(r: Rule, s: &Vec<Named>) -> Vec<(Rule, Vec<Named>)> {
    let mut out = Vec::new();
    if s.len() < r.pattern.len() {
        return out;
    }

    match r.test(s) {
        Ok(_) => {}
        Err(e) => {
            out.push((r.clone(), e.clone()));

            let line = e.last().unwrap().span().end().line;
            let col = e.last().unwrap().span().end().column;

            let more = s
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

            out.extend(walk(r.clone(), &more.clone().collect()));
        }
    }

    out
}

fn test(r: Rules, s: Vec<Named>, source: String, file: String) -> (Vec<LintError>, usize) {
    let any = r
        .rules
        .iter()
        .flat_map(|(_, v)| walk(v.clone(), &s))
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
        source: NamedSource::new(&file, source.clone()),
    });

    (
        errors.collect::<Vec<_>>(),
        any.iter().filter(|a| a.0.fails).count(),
    )
}
