use std::path::Path;

use miette::Report;
use serde_json::{json, Value};

use crate::ty::LintError;

pub fn compiler_span(c: &LintError) -> Value {
    let v = c.window.clone();

    let byte_start = v.first().unwrap().span().byte_range().start;
    let byte_end = v.last().unwrap().span().byte_range().end;

    let column_start = v.first().unwrap().span().start().column + 1;
    let column_end = v.last().unwrap().span().end().column + 1;

    let line_start = v.first().unwrap().span().start().line;
    let line_end = v.last().unwrap().span().end().line;

    let highlight_end = byte_start - c.line.1;
    let highlight_start = highlight_end - (byte_end - byte_start);

    let file_name = c.source.name();

    json!({
        "byte_end": byte_end,
        "byte_start": byte_start,
        "column_end": column_end,
        "column_start": column_start,
        "expansion": null,
        "file_name": file_name,
        "is_primary": true,
        "label": null,
        "line_end": line_end,
        "line_start": line_start,
        "suggested_replacement": null,
        "suggestion_applicability": null,
        "text": [
            {
                "highlight_end": highlight_end,
                "highlight_start": highlight_start,
                "text": c.line.0.to_string()
            }
        ]
    })
}

impl LintError {
    pub fn json_diagnostic(&self) -> Value {
        let span = compiler_span(&self);
        let level = match self.rule.fails {
            true => "error",
            false => "warning",
        };

        let absolute_file_path = Path::new(&self.source.name())
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();

        json!({
            "reason": "compiler-message",
            "package_id": "",
            "manifest_path": "",
            "target": {
                "kind": [
                    "bin"
                ],
                "crate_types": [
                    "bin"
                ],
                "name": "splint",
                "src_path": absolute_file_path,
                "edition": "2021",
                "doc": true,
                "doctest": false,
                "test": true
            },
            "message": {
                "rendered": format!("{:?}", Report::new(self.clone())),
                "$message_type": "diagnostic",
                "children": [
                    {
                        "children": [],
                        "code": null,
                        "level": "note",
                        "message": self.rule.clone().description,
                        "rendered": null,
                        "spans": []
                    },
                    {
                        "children": [],
                        "code": null,
                        "level": "help",
                        "message": self.rule.clone().help.unwrap_or("Lint failed here".to_string()),
                        "rendered": null,
                        "spans": [
                            span.clone()
                        ]
                    }
                ],
                "code": {
                    "code": "unused_imports",
                    "explanation": null
                },
                "level": level,
                "message": self.rule.name.clone(),
                "spans": [
                    span.clone()
                ]
            }
        })
    }
}
