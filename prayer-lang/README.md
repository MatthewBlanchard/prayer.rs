# prayer-lang

`prayer-lang` implements the parser, validator, normalizer, analyzer, action
renderer, and compiler for PrayerLang. PrayerLang programs are strictly linear
command lists; clients resolve dynamic policy choices before submitting them.

The typical pipeline is:

1. `AstProgram::parse` source text and receive span-aware `Diagnostic` values
   on failure.
2. Validate commands against `ValidationContext::with_defaults()` or a custom
   command catalog.
3. Analyze against an `AnalysisObservation`.
4. Compile the analyzed program into versioned `CompiledProgram` plan nodes
   containing typed `prayer_actions::Action` values and a source map.

`AstProgram::normalize` produces canonical source, `Diagnostic::render`
formats a source-labelled error, and `render_action` projects a typed action
back into PrayerLang text.

```rust
use prayer_lang::{AstProgram, ValidationContext};

let source = "go sol_central; dock; refuel;";
let program = AstProgram::parse(source).expect("valid syntax");
let diagnostics = program.validate(&ValidationContext::with_defaults());

assert!(diagnostics.is_empty());
assert_eq!(program.normalize(), "go sol_central;\ndock;\nrefuel;\n");
```

From the repository root:

```console
cargo check -p prayer-lang
cargo test -p prayer-lang
```
