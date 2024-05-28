use std::path::Path;
use std::str::FromStr;

use miette::Report;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use to_and_fro::{default, ToAndFro};

use crate::ty::LintError;

#[derive(ToAndFro, Clone)]
#[serde]
enum SuggestionApplicability {
    MachineApplicable,
    HasPlaceholders,
    MaybeIncorrect,
    Unspecified,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CompilerSpan {
    /// Byte offset of the start of the span, relative to the start of the file
    byte_start: usize,
    /// Byte offset of the end of the span, relative to the start of the file
    byte_end: usize,

    /// Column offset of the start of the span, in the source file
    column_start: usize,
    /// Column offset of the end of the span, in the source file
    column_end: usize,

    /// Line number of the start of the span, in the source file
    line_end: usize,
    /// Line number of the end of the span, in the source file
    line_start: usize,

    /// I'm not sure what this does right now
    expansion: Option<()>,

    /// Name of the span's source file
    file_name: String,
    /// Whether the span is primary
    is_primary: bool,

    /// Label for the span
    label: Option<String>,

    /// Text replacement of the highlight
    suggested_replacement: Option<String>,
    /// Applicability of the suggestion
    suggestion_applicability: Option<SuggestionApplicability>,

    /// Text of the span
    text: Vec<CompilerSpanText>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CompilerSpanText {
    /// Byte offset of the start of the highlight, relative to the start of the span
    highlight_start: usize,
    /// Byte offset of the end of the highlight, relative to the start of the span
    highlight_end: usize,
    /// Line of source code the span occurs in
    text: String,
}

#[derive(ToAndFro, Clone)]
#[serde]
#[casing("kebab")]
#[default("CompilerMessage")]
pub enum CompilerMessageReason {
    CompilerMessage,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CompilerMessage {
    reason: CompilerMessageReason,
    /// Package ID, can be empty
    package_id: String,
    /// Path to the Cargo.toml, can be empty
    manifest_path: String,
    /// Package target details
    target: CompilerMessageTarget,
    /// Assosciated message
    message: CompilerMessageInner,
}

#[derive(ToAndFro, Clone)]
#[serde]
#[casing("kebab")]
pub enum CompilerMessageLevel {
    Warning,
    Note,
    Help,
    ErrorNote,
    Error,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CompilerMessageChild {
    /// Not sure what this is, but it's always empty
    children: Vec<()>,
    /// Code of the message (e.g. E0001, or a lint name)
    code: Option<String>,
    /// Level of severity of the message
    level: CompilerMessageLevel,
    /// Message text
    message: String,
    /// Rendered message text
    rendered: Option<String>,
    /// Spans the message relates to
    spans: Vec<CompilerSpan>,
}

#[derive(ToAndFro, Clone)]
#[serde]
#[casing("kebab")]
#[default("Diagnostic")]
pub enum CompilerMessageType {
    Diagnostic,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CompilerMessageInner {
    /// Rendered message text
    rendered: Option<String>,
    #[serde(rename = "$message_type")]
    message_type: CompilerMessageType,
    /// Children of the message
    children: Vec<CompilerMessageChild>,
    /// Code of the message (e.g. E0001, or a lint name)
    code: CompilerMessageCode,
    /// Level of severity of the message
    level: CompilerMessageLevel,
    /// Message text
    message: String,
    /// Spans the message relates to
    spans: Vec<CompilerSpan>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CompilerMessageCode {
    /// unused_imports, etc. Not sure if this follows above convention
    code: String,
    /// Short description of the code
    explanation: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CompilerMessageTarget {
    /// lib, bin, etc.
    kind: Vec<String>,
    /// lib, bin, etc.
    crate_types: Vec<String>,
    /// Name of the target
    name: String,
    /// Absolute path to the source file
    src_path: String,
    /// Rust edition of target
    edition: String,
    /// Whether the current profile supports benchmarks, for cargo check this is true
    doc: bool,
    /// Whether the current profile supports doctests, for cargo check this is false
    doctest: bool,
    /// Whether the current profile supports tests, for cargo check this is true
    test: bool,
}

impl Into<CompilerSpan> for &LintError {
    fn into(self) -> CompilerSpan {
        let v = self.window.clone();

        let byte_start = v.first().unwrap().span().byte_range().start;
        let byte_end = v.last().unwrap().span().byte_range().end;

        let column_start = v.first().unwrap().span().start().column + 1;
        let column_end = v.last().unwrap().span().end().column + 1;

        let line_start = v.first().unwrap().span().start().line;
        let line_end = v.last().unwrap().span().end().line;

        let highlight_end = byte_start - self.line.1;
        let highlight_start = highlight_end - (byte_end - byte_start);

        let file_name = self.source.name();

        CompilerSpan {
            byte_end: byte_end,
            byte_start: byte_start,
            column_end: column_end,
            column_start: column_start,
            expansion: None,
            file_name: file_name.to_string(),
            is_primary: true,
            label: None,
            line_end: line_end,
            line_start: line_start,
            suggested_replacement: self.rule.replace.clone(),
            suggestion_applicability: None,
            text: vec![CompilerSpanText {
                highlight_end: highlight_end,
                highlight_start: highlight_start,
                text: self.line.0.to_string(),
            }],
        }
    }
}

impl LintError {
    pub fn json_diagnostic(&self) -> CompilerMessage {
        let span: CompilerSpan = self.into();
        let level = match self.rule.fails {
            true => CompilerMessageLevel::Error,
            false => CompilerMessageLevel::Warning,
        };

        let absolute_file_path = Path::new(&self.source.name())
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();

        let mut children = vec![CompilerMessageChild {
            children: vec![],
            code: None,
            level: level.clone(),
            message: self.rule.clone().description,
            rendered: None,
            spans: vec![],
        }];

        if let Some(help) = self.rule.help.clone() {
            children.push(CompilerMessageChild {
                children: vec![],
                code: None,
                level: CompilerMessageLevel::Help,
                message: help,
                rendered: None,
                spans: vec![span.clone()],
            });
        }

        CompilerMessage {
            reason: CompilerMessageReason::CompilerMessage,
            package_id: String::new(),
            manifest_path: String::new(),

            target: CompilerMessageTarget {
                kind: vec!["bin".to_string()],
                crate_types: vec!["bin".to_string()],
                name: "splint".to_string(),
                src_path: absolute_file_path,
                edition: "2021".to_string(),
                doc: true,
                doctest: false,
                test: true,
            },

            message: CompilerMessageInner {
                message_type: CompilerMessageType::Diagnostic,

                children,
                level: level.clone(),
                rendered: Some(format!("{:?}", Report::new(self.clone()))),
                message: self.rule.name.clone(),
                spans: vec![span.clone()],

                code: CompilerMessageCode {
                    code: self.rule.name.clone(),
                    explanation: None,
                },
            },
        }
    }
}
