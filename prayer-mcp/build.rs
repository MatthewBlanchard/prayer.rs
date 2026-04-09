//! Build script for prayer-mcp.
//!
//! Validates that every command in the default catalog has a doc entry in
//! `COMMAND_DOCS`, then generates two files into `OUT_DIR`:
//!
//! - `dsl_reference.txt`  — full PrayerLang reference (static header + generated
//!                           commands section + static footer)
//! - `dsl_commands.json`  — commands array used by `dsl_reference_json()`

use prayer_runtime::catalog::{default_command_catalog, COMMAND_DOCS};
use prayer_runtime::dsl::ArgType;
use std::collections::HashMap;
use std::path::PathBuf;

// Static sections of the reference document.
const HEADER: &str = include_str!("src/dsl_ref_header.txt");
const FOOTER: &str = include_str!("src/dsl_ref_footer.txt");

fn main() {
    // Re-run if catalog or template files change.
    println!("cargo::rerun-if-changed=../prayer-runtime/src/catalog.rs");
    println!("cargo::rerun-if-changed=src/dsl_ref_header.txt");
    println!("cargo::rerun-if-changed=src/dsl_ref_footer.txt");

    let catalog = default_command_catalog();
    let docs: HashMap<&str, &str> = COMMAND_DOCS.iter().copied().collect();

    // Validate: every catalog command must have a non-empty doc entry.
    let mut missing: Vec<String> = catalog
        .keys()
        .filter(|name| {
            docs.get(name.as_str())
                .map(|d| d.is_empty())
                .unwrap_or(true)
        })
        .cloned()
        .collect();
    missing.sort();

    if !missing.is_empty() {
        for name in &missing {
            println!(
                "cargo::error=Command '{name}' has no entry in COMMAND_DOCS in catalog.rs. \
                 Add a doc string or the build will fail."
            );
        }
        std::process::exit(1);
    }

    let out_dir: PathBuf = std::env::var("OUT_DIR").unwrap().into();

    // Generate the commands section and JSON using COMMAND_DOCS order.
    let mut md_commands = String::from("## Default command catalog\n\nCurrent built-in commands:\n");
    let mut json_commands = String::from("[\n");
    let mut first_json = true;

    for (name, doc) in COMMAND_DOCS {
        let Some(spec) = catalog.get(*name) else {
            // COMMAND_DOCS has an entry for a command not in the catalog — skip.
            continue;
        };

        // Markdown signature line.
        let mut sig = (*name).to_string();
        for arg in &spec.args {
            if arg.required {
                sig.push_str(&format!(" <{}>", arg.name));
            } else {
                sig.push_str(&format!(" [{}]", arg.name));
            }
        }
        md_commands.push_str(&format!("\n### `{sig}`\n\n{doc}\n"));

        // JSON entry.
        if !first_json {
            json_commands.push_str(",\n");
        }
        first_json = false;

        json_commands.push_str(&format!("  {{\"name\":{},\"doc\":{},\"args\":[", json_str(name), json_str(doc)));
        for (i, arg) in spec.args.iter().enumerate() {
            if i > 0 {
                json_commands.push(',');
            }
            let type_str = arg_type_str(arg.kind);
            if arg.required {
                json_commands.push_str(&format!(
                    "{{\"name\":{},\"type\":{}}}",
                    json_str(&arg.name),
                    json_str(type_str)
                ));
            } else {
                json_commands.push_str(&format!(
                    "{{\"name\":{},\"type\":{},\"optional\":true}}",
                    json_str(&arg.name),
                    json_str(type_str)
                ));
            }
        }
        json_commands.push_str("]}");
    }
    json_commands.push_str("\n]");

    // Write dsl_reference.txt
    let reference = format!("{HEADER}\n{md_commands}\n{FOOTER}");
    std::fs::write(out_dir.join("dsl_reference.txt"), reference)
        .expect("failed to write dsl_reference.txt");

    // Write dsl_commands.json
    std::fs::write(out_dir.join("dsl_commands.json"), json_commands)
        .expect("failed to write dsl_commands.json");
}

fn json_str(s: &str) -> String {
    // Minimal JSON string escaping sufficient for doc strings and identifiers.
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn arg_type_str(kind: ArgType) -> &'static str {
    kind.as_str()
}
