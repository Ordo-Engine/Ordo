//! Shell-command analysis for guard policies.
//!
//! A `Bash` tool call carries one free-form `command` string, and matching it
//! with substrings is trivially bypassed (`rm -r -f`, `rm  -rf`, `/bin/rm -Rf`,
//! `git -C . push`, `sudo env X=1 rm -rf`, `bash -c '…'`). This module splits
//! the string the way a POSIX shell would — quotes, escapes, `&&`/`||`/`;`/`|`,
//! subshells, `$(…)` and backtick substitutions, `bash -c`/`eval` payloads,
//! heredocs — strips wrappers (`sudo`, `env`, `xargs`, `nohup`, `timeout`, …)
//! and exposes structured facts the policy can match exactly:
//!
//! - `programs` — every program that would run, by basename (`["git", "rm"]`)
//! - `subcommands` — `"<program> <first non-flag argument>"` (`"git push"`),
//!   skipping well-known global options that take a value (`git -C dir`)
//! - `argv` — program → all its arguments across invocations, with short-flag
//!   clusters also expanded (`-rf` adds `-r` and `-f`)
//! - `commands` — each simple command as written, whitespace-normalized
//! - `words` — every argument and redirect target that is a single token (no
//!   whitespace), for path checks — free text like a commit message is left
//!   out so it can't trip a path rule
//! - `shell_parse` — `ok`, or `error` when quoting/substitution was unbalanced
//!
//! This is analysis, not execution: nothing is expanded or evaluated. It is
//! best-effort by design (variables, aliases and functions are invisible), so
//! guard stays defense-in-depth, not a sandbox.

use std::collections::BTreeMap;

/// Nesting limit for `bash -c` / `eval` / `$(…)` payloads.
const MAX_DEPTH: usize = 4;

#[derive(Debug, Default)]
pub(crate) struct ShellFacts {
    pub programs: Vec<String>,
    pub subcommands: Vec<String>,
    pub argv: BTreeMap<String, Vec<String>>,
    pub commands: Vec<String>,
    pub words: Vec<String>,
    pub parse_ok: bool,
}

impl ShellFacts {
    /// Insert the facts into a policy-input map (overwriting same-named keys).
    pub fn insert_into(self, map: &mut serde_json::Map<String, serde_json::Value>) {
        let argv: serde_json::Map<String, serde_json::Value> =
            self.argv.into_iter().map(|(k, v)| (k, v.into())).collect();
        map.insert("programs".into(), self.programs.into());
        map.insert("subcommands".into(), self.subcommands.into());
        map.insert("argv".into(), argv.into());
        map.insert("commands".into(), self.commands.into());
        map.insert("words".into(), self.words.into());
        map.insert(
            "shell_parse".into(),
            if self.parse_ok { "ok" } else { "error" }.into(),
        );
    }
}

/// The input keys `insert_into` writes — reserved so a tool argument can't
/// impersonate them.
pub(crate) const FACT_KEYS: [&str; 6] = [
    "programs",
    "subcommands",
    "argv",
    "commands",
    "words",
    "shell_parse",
];

pub(crate) fn analyze(command: &str) -> ShellFacts {
    let mut facts = ShellFacts {
        parse_ok: true,
        ..Default::default()
    };
    analyze_into(command, 0, &mut facts);
    facts
}

fn analyze_into(src: &str, depth: usize, facts: &mut ShellFacts) {
    if depth > MAX_DEPTH {
        facts.parse_ok = false;
        return;
    }
    let mut subs = Vec::new();
    let tokens = Lexer::new(src).run(&mut subs, &mut facts.parse_ok);

    let mut words = Vec::new();
    let mut redirect_targets = Vec::new();
    let mut redirect_next = false;
    for tok in tokens.into_iter().chain(std::iter::once(Token::Sep)) {
        match tok {
            Token::Word(w) => {
                if redirect_next {
                    redirect_targets.push(w);
                    redirect_next = false;
                } else {
                    words.push(w);
                }
            }
            Token::Redir => redirect_next = true,
            Token::Sep => {
                for t in redirect_targets.drain(..) {
                    push_word(facts, t);
                }
                if !words.is_empty() {
                    simple_command(std::mem::take(&mut words), depth, facts);
                }
                redirect_next = false;
            }
        }
    }
    for sub in subs {
        analyze_into(&sub, depth + 1, facts);
    }
}

fn push_unique(v: &mut Vec<String>, s: String) {
    if !v.contains(&s) {
        v.push(s);
    }
}

fn is_assignment(w: &str) -> bool {
    match w.split_once('=') {
        Some((name, _)) => {
            let mut chars = name.chars();
            matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
        None => false,
    }
}

fn basename(w: &str) -> &str {
    w.rsplit('/').next().unwrap_or(w)
}

/// Index of the first argument after `start` that is not an option, skipping
/// the values of options listed in `with_value`. `--` ends option parsing.
fn skip_options(words: &[String], start: usize, with_value: &[&str]) -> usize {
    let mut i = start;
    while i < words.len() {
        let w = words[i].as_str();
        if w == "--" {
            return i + 1;
        }
        if !w.starts_with('-') || w == "-" {
            break;
        }
        i += if with_value.contains(&w) { 2 } else { 1 };
    }
    i.min(words.len())
}

/// Process one simple command (already split from its neighbours).
fn simple_command(mut words: Vec<String>, depth: usize, facts: &mut ShellFacts) {
    facts.commands.push(words.join(" "));
    // Unwrap wrappers (`sudo env X=1 nice -n 5 rm …`) one layer at a time.
    for _ in 0..16 {
        let skip = words
            .iter()
            .take_while(|w| is_assignment(w) || is_keyword(w))
            .count();
        words.drain(..skip);
        let Some(first) = words.first() else { return };
        if matches!(first.as_str(), "for" | "case" | "select" | "function") {
            // Loop/case headers aren't commands; their bodies are separate
            // simple commands after the `;`/newline split.
            for w in words.drain(1..) {
                push_word(facts, w);
            }
            return;
        }
        let prog = basename(first).to_string();
        let rest = match prog.as_str() {
            "sudo" => skip_options(
                &words,
                1,
                &["-u", "-g", "-C", "-D", "-h", "-p", "-r", "-t", "-U", "-T"],
            ),
            "doas" => skip_options(&words, 1, &["-u", "-C"]),
            "env" => skip_options(&words, 1, &["-u", "-C", "-S", "--unset", "--chdir"]),
            "nohup" | "exec" | "builtin" | "unbuffer" | "time" => skip_options(&words, 1, &[]),
            "command" => {
                if words.get(1).is_some_and(|w| w == "-v" || w == "-V") {
                    // `command -v x` only looks `x` up.
                    record(&prog, &words[1..], facts);
                    return;
                }
                skip_options(&words, 1, &[])
            }
            "nice" => skip_options(&words, 1, &["-n", "--adjustment"]),
            "ionice" => skip_options(&words, 1, &["-c", "-n", "-p", "--class", "--classdata"]),
            "stdbuf" => skip_options(&words, 1, &["-i", "-o", "-e"]),
            "timeout" => {
                let i = skip_options(&words, 1, &["-s", "-k", "--signal", "--kill-after"]);
                (i + 1).min(words.len()) // the duration
            }
            "watch" => skip_options(&words, 1, &["-n", "--interval", "-d"]),
            "xargs" => skip_options(
                &words,
                1,
                &[
                    "-I",
                    "-n",
                    "-P",
                    "-d",
                    "-L",
                    "-s",
                    "-a",
                    "-E",
                    "--max-args",
                    "--max-procs",
                ],
            ),
            "bash" | "sh" | "zsh" | "dash" | "ksh" | "fish" => {
                record(&prog, &words[1..], facts);
                if let Some(script) = shell_c_payload(&words) {
                    analyze_into(&script, depth + 1, facts);
                }
                return;
            }
            "eval" => {
                record(&prog, &words[1..], facts);
                analyze_into(&words[1..].join(" "), depth + 1, facts);
                return;
            }
            _ => {
                record(&prog, &words[1..], facts);
                return;
            }
        };
        // A wrapper: record it with its own options, then unwrap.
        record(&prog, &words[1..rest], facts);
        if prog == "env" {
            // `env -S 'rm -rf x'` splits its string argument into a command.
            if let Some(pos) = words[..rest].iter().position(|w| w == "-S") {
                if let Some(s) = words.get(pos + 1) {
                    analyze_into(s, depth + 1, facts);
                }
            }
        }
        words.drain(..rest);
        if words.is_empty() {
            return;
        }
    }
    facts.parse_ok = false;
}

fn is_keyword(w: &str) -> bool {
    matches!(
        w,
        "{" | "}"
            | "!"
            | "if"
            | "then"
            | "else"
            | "elif"
            | "fi"
            | "do"
            | "done"
            | "while"
            | "until"
            | "esac"
    )
}

/// The script string of `sh -c '…'` / `bash -lc '…'`, if any.
fn shell_c_payload(words: &[String]) -> Option<String> {
    let mut i = 1;
    while i < words.len() {
        let w = &words[i];
        if !w.starts_with('-') || w == "--" {
            return None;
        }
        if w == "-c" || (!w.starts_with("--") && w.contains('c')) {
            return words.get(i + 1).cloned();
        }
        i += if w == "-o" || w == "+o" { 2 } else { 1 };
    }
    None
}

/// Global options (before the subcommand) that take a separate value.
fn global_value_options(prog: &str) -> &'static [&'static str] {
    match prog {
        "git" => &["-C", "-c", "--git-dir", "--work-tree", "--namespace"],
        "kubectl" | "helm" | "oc" => &[
            "-n",
            "--namespace",
            "--context",
            "--kubeconfig",
            "--cluster",
            "-s",
            "--server",
        ],
        "docker" | "podman" => &["-H", "--host", "--context", "-c", "--config", "-l"],
        "npm" | "pnpm" | "yarn" => &[
            "-C",
            "--prefix",
            "--dir",
            "--cwd",
            "-w",
            "--workspace",
            "--filter",
            "-F",
        ],
        "cargo" => &["-C", "--config", "-Z"],
        _ => &[],
    }
}

fn record(prog: &str, args: &[String], facts: &mut ShellFacts) {
    push_unique(&mut facts.programs, prog.to_string());

    let sub_at = skip_options(args, 0, global_value_options(prog));
    if let Some(sub) = args.get(sub_at) {
        push_unique(&mut facts.subcommands, format!("{prog} {sub}"));
    }

    let argv = facts.argv.entry(prog.to_string()).or_default();
    for a in args {
        push_unique(argv, a.clone());
        let is_cluster = a.len() > 2
            && a.starts_with('-')
            && !a.starts_with("--")
            && a[1..].chars().all(|c| c.is_ascii_alphabetic());
        if is_cluster {
            for c in a[1..].chars() {
                push_unique(argv, format!("-{c}"));
            }
        }
    }
    for a in args {
        push_word(facts, a.clone());
    }
}

fn push_word(facts: &mut ShellFacts, w: String) {
    if !w.is_empty() && !w.contains(char::is_whitespace) {
        push_unique(&mut facts.words, w);
    }
}

#[derive(Debug, PartialEq)]
enum Token {
    Word(String),
    /// A command boundary: `;` `&` `&&` `||` `|` `|&` `(` `)` newline.
    Sep,
    /// A redirection operator; the next word is its target.
    Redir,
}

struct Lexer {
    chars: Vec<char>,
    i: usize,
    /// Heredoc delimiters (and whether `<<-`) waiting for the next newline.
    heredocs: Vec<(String, bool)>,
}

impl Lexer {
    fn new(src: &str) -> Self {
        Lexer {
            chars: src.chars().collect(),
            i: 0,
            heredocs: Vec::new(),
        }
    }

    fn peek(&self, off: usize) -> Option<char> {
        self.chars.get(self.i + off).copied()
    }

    fn run(mut self, subs: &mut Vec<String>, ok: &mut bool) -> Vec<Token> {
        let mut out = Vec::new();
        while let Some(c) = self.peek(0) {
            match c {
                ' ' | '\t' | '\r' => self.i += 1,
                '\n' => {
                    self.i += 1;
                    out.push(Token::Sep);
                    self.skip_heredoc_bodies();
                }
                ';' | ')' => {
                    self.i += 1;
                    out.push(Token::Sep);
                }
                '(' => {
                    self.i += 1;
                    out.push(Token::Sep);
                }
                '&' => {
                    if self.peek(1) == Some('>') {
                        self.i += if self.peek(2) == Some('>') { 3 } else { 2 };
                        out.push(Token::Redir);
                    } else {
                        self.i += if self.peek(1) == Some('&') { 2 } else { 1 };
                        out.push(Token::Sep);
                    }
                }
                '|' => {
                    self.i += if matches!(self.peek(1), Some('|') | Some('&')) {
                        2
                    } else {
                        1
                    };
                    out.push(Token::Sep);
                }
                '<' | '>' if self.peek(1) == Some('(') => {
                    // Process substitution: its body is a separate command.
                    self.i += 2;
                    subs.push(self.read_balanced(ok));
                }
                '<' => {
                    if self.peek(1) == Some('<') && self.peek(2) != Some('<') {
                        let dash = self.peek(2) == Some('-');
                        self.i += if dash { 3 } else { 2 };
                        self.skip_blanks();
                        let delim = self.read_word(subs, ok);
                        if !delim.is_empty() {
                            self.heredocs.push((delim, dash));
                        }
                    } else {
                        while matches!(self.peek(0), Some('<') | Some('>') | Some('&')) {
                            self.i += 1;
                        }
                        out.push(Token::Redir);
                    }
                }
                '>' => {
                    self.i += 1;
                    while matches!(self.peek(0), Some('>') | Some('&') | Some('|')) {
                        self.i += 1;
                    }
                    if self.peek(0) == Some('-') {
                        // `>&-` closes a descriptor; no target follows.
                        self.i += 1;
                        continue;
                    }
                    out.push(Token::Redir);
                }
                '#' => {
                    while self.peek(0).is_some_and(|c| c != '\n') {
                        self.i += 1;
                    }
                }
                _ => {
                    let w = self.read_word(subs, ok);
                    let is_fd = !w.is_empty()
                        && w.chars().all(|c| c.is_ascii_digit())
                        && matches!(self.peek(0), Some('<') | Some('>'));
                    if !is_fd {
                        out.push(Token::Word(w));
                    }
                }
            }
        }
        out
    }

    fn skip_blanks(&mut self) {
        while matches!(self.peek(0), Some(' ') | Some('\t')) {
            self.i += 1;
        }
    }

    /// After a newline, consume the bodies of any pending heredocs.
    fn skip_heredoc_bodies(&mut self) {
        for (delim, dash) in std::mem::take(&mut self.heredocs) {
            loop {
                if self.i >= self.chars.len() {
                    return;
                }
                let start = self.i;
                while self.peek(0).is_some_and(|c| c != '\n') {
                    self.i += 1;
                }
                let line: String = self.chars[start..self.i].iter().collect();
                if self.peek(0) == Some('\n') {
                    self.i += 1;
                }
                let line = if dash {
                    line.trim_start_matches('\t')
                } else {
                    line.as_str()
                };
                if line == delim {
                    break;
                }
            }
        }
    }

    fn is_word_end(c: char) -> bool {
        matches!(
            c,
            ' ' | '\t' | '\r' | '\n' | ';' | '&' | '|' | '<' | '>' | '(' | ')'
        )
    }

    /// Read one shell word, removing quotes and escapes. Substitutions are
    /// kept verbatim in the word and their bodies pushed onto `subs`.
    fn read_word(&mut self, subs: &mut Vec<String>, ok: &mut bool) -> String {
        let mut w = String::new();
        while let Some(c) = self.peek(0) {
            if Self::is_word_end(c) {
                break;
            }
            self.i += 1;
            match c {
                '\\' => match self.peek(0) {
                    Some('\n') => self.i += 1,
                    Some(n) => {
                        w.push(n);
                        self.i += 1;
                    }
                    None => {}
                },
                '\'' => loop {
                    match self.peek(0) {
                        Some('\'') => {
                            self.i += 1;
                            break;
                        }
                        Some(n) => {
                            w.push(n);
                            self.i += 1;
                        }
                        None => {
                            *ok = false;
                            break;
                        }
                    }
                },
                '"' => loop {
                    match self.peek(0) {
                        Some('"') => {
                            self.i += 1;
                            break;
                        }
                        Some('\\') => {
                            self.i += 1;
                            match self.peek(0) {
                                Some(n @ ('$' | '`' | '"' | '\\')) => {
                                    w.push(n);
                                    self.i += 1;
                                }
                                Some('\n') => self.i += 1,
                                _ => w.push('\\'),
                            }
                        }
                        Some('$') if self.peek(1) == Some('(') => {
                            self.i += 1;
                            self.read_dollar_paren(&mut w, subs, ok);
                        }
                        Some('`') => {
                            self.i += 1;
                            self.read_backtick(&mut w, subs, ok);
                        }
                        Some(n) => {
                            w.push(n);
                            self.i += 1;
                        }
                        None => {
                            *ok = false;
                            break;
                        }
                    }
                },
                '$' if self.peek(0) == Some('(') => self.read_dollar_paren(&mut w, subs, ok),
                '$' if self.peek(0) == Some('{') => {
                    w.push('$');
                    while let Some(n) = self.peek(0) {
                        w.push(n);
                        self.i += 1;
                        if n == '}' {
                            break;
                        }
                    }
                }
                '`' => self.read_backtick(&mut w, subs, ok),
                _ => w.push(c),
            }
        }
        w
    }

    /// At `(` right after a `$`: read `$(…)` (a command substitution, pushed
    /// onto `subs`) or `$((…))` (arithmetic, not a command).
    fn read_dollar_paren(&mut self, w: &mut String, subs: &mut Vec<String>, ok: &mut bool) {
        let arithmetic = self.peek(1) == Some('(');
        self.i += 1;
        let body = self.read_balanced(ok);
        w.push_str("$(");
        w.push_str(&body);
        w.push(')');
        if !arithmetic {
            subs.push(body);
        }
    }

    fn read_backtick(&mut self, w: &mut String, subs: &mut Vec<String>, ok: &mut bool) {
        let mut body = String::new();
        loop {
            match self.peek(0) {
                Some('`') => {
                    self.i += 1;
                    break;
                }
                Some('\\') if matches!(self.peek(1), Some('`') | Some('\\') | Some('$')) => {
                    body.push(self.chars[self.i + 1]);
                    self.i += 2;
                }
                Some(n) => {
                    body.push(n);
                    self.i += 1;
                }
                None => {
                    *ok = false;
                    break;
                }
            }
        }
        w.push('`');
        w.push_str(&body);
        w.push('`');
        subs.push(body);
    }

    /// Just past an opening `(`: read up to the matching `)`, honouring
    /// quotes and escapes, and return the body.
    fn read_balanced(&mut self, ok: &mut bool) -> String {
        let start = self.i;
        let mut depth = 1usize;
        while let Some(c) = self.peek(0) {
            match c {
                '\\' => self.i += 1,
                '\'' => {
                    self.i += 1;
                    while self.peek(0).is_some_and(|c| c != '\'') {
                        self.i += 1;
                    }
                }
                '"' => {
                    self.i += 1;
                    while let Some(n) = self.peek(0) {
                        if n == '\\' {
                            self.i += 1;
                        } else if n == '"' {
                            break;
                        }
                        self.i += 1;
                    }
                }
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        let body = self.chars[start..self.i].iter().collect();
                        self.i += 1;
                        return body;
                    }
                }
                _ => {}
            }
            self.i += 1;
        }
        *ok = false;
        self.i = self.chars.len();
        self.chars[start.min(self.chars.len())..].iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn programs(cmd: &str) -> Vec<String> {
        analyze(cmd).programs
    }

    fn argv(cmd: &str, prog: &str) -> Vec<String> {
        analyze(cmd).argv.remove(prog).unwrap_or_default()
    }

    #[test]
    fn splits_compound_commands() {
        assert_eq!(
            programs("git status && rm -rf x; ls | grep y || echo z & cat w"),
            ["git", "rm", "ls", "grep", "echo", "cat"]
        );
        assert_eq!(programs("(cd a && rm -r b)"), ["cd", "rm"]);
        assert_eq!(programs("ls\nrm -r x"), ["ls", "rm"]);
    }

    #[test]
    fn flags_are_expanded_regardless_of_spelling() {
        for cmd in [
            "rm -rf build",
            "rm  -rf build",
            "rm -r -f build",
            "rm -fr build",
            "/bin/rm -rf build",
            "\\rm -rf build",
            "'rm' -rf build",
        ] {
            let a = argv(cmd, "rm");
            assert!(a.contains(&"-r".to_string()), "{cmd}: {a:?}");
            assert!(a.contains(&"-f".to_string()), "{cmd}: {a:?}");
        }
        assert!(argv("rm -Rf x", "rm").contains(&"-R".to_string()));
        assert!(argv("rm --recursive x", "rm").contains(&"--recursive".to_string()));
        // Long single-dash options are kept whole too.
        assert!(argv("find . -delete", "find").contains(&"-delete".to_string()));
    }

    #[test]
    fn wrappers_are_unwrapped() {
        for cmd in [
            "sudo rm -rf /",
            "sudo -u root rm -rf /",
            "env FOO=1 rm -rf /",
            "FOO=1 rm -rf /",
            "nohup rm -rf / &",
            "timeout 5 rm -rf /",
            "nice -n 10 rm -rf /",
            "find . -name x | xargs rm -rf",
            "xargs -I {} rm -rf {}",
            "command rm -rf /",
            "env -S 'rm -rf /'",
        ] {
            assert!(programs(cmd).contains(&"rm".to_string()), "{cmd}");
            assert!(argv(cmd, "rm").contains(&"-r".to_string()), "{cmd}");
        }
        assert!(programs("sudo rm x").contains(&"sudo".to_string()));
    }

    #[test]
    fn nested_payloads_are_analyzed() {
        for cmd in [
            "bash -c 'rm -rf /'",
            "sh -lc \"cd x && rm -rf y\"",
            "eval rm -rf /",
            "echo $(rm -rf /)",
            "echo \"$(rm -rf /)\"",
            "echo `rm -rf /`",
            "diff <(rm -rf /) b",
        ] {
            assert!(programs(cmd).contains(&"rm".to_string()), "{cmd}");
        }
        // Arithmetic is not a command.
        assert_eq!(programs("echo $((1 + 2))"), ["echo"]);
    }

    #[test]
    fn subcommands_skip_global_value_options() {
        let f = analyze("git -C repo push origin main");
        assert_eq!(f.subcommands, ["git push"]);
        assert_eq!(analyze("git -c a=b push").subcommands, ["git push"]);
        assert_eq!(
            analyze("kubectl -n prod delete pod x").subcommands,
            ["kubectl delete"]
        );
        assert!(analyze("npm --prefix web publish")
            .subcommands
            .contains(&"npm publish".to_string()));
    }

    #[test]
    fn quoted_text_is_one_argument_not_a_command() {
        let f = analyze("git commit -m 'rm -rf everything; git push'");
        assert_eq!(f.programs, ["git"]);
        assert_eq!(f.subcommands, ["git commit"]);
        let f = analyze("echo \"a && rm -rf b\"");
        assert_eq!(f.programs, ["echo"]);
        // Free text isn't a path: it stays out of `words`.
        let f = analyze("git commit -m 'stop tracking .env' -- a.txt");
        assert!(!f.words.iter().any(|w| w.contains(".env")), "{:?}", f.words);
        assert!(f.words.contains(&"a.txt".to_string()));
    }

    #[test]
    fn heredoc_bodies_are_data() {
        let f = analyze("cat <<'EOF' > notes.md\nrm -rf /\nEOF\nls");
        assert_eq!(f.programs, ["cat", "ls"]);
        assert!(f.words.contains(&"notes.md".to_string()));
    }

    #[test]
    fn redirect_targets_are_words_not_arguments() {
        let f = analyze("cat < .env 2>/dev/null");
        assert_eq!(f.programs, ["cat"]);
        assert!(f.words.contains(&".env".to_string()));
        assert!(f.argv["cat"].is_empty());
        assert!(analyze("echo x > out.txt")
            .words
            .contains(&"out.txt".to_string()));
    }

    #[test]
    fn comments_and_keywords_are_skipped() {
        assert_eq!(programs("ls # rm -rf /"), ["ls"]);
        assert_eq!(programs("if true; then rm -rf x; fi"), ["true", "rm"]);
        assert_eq!(programs("for f in *.tmp; do rm -r \"$f\"; done"), ["rm"]);
    }

    #[test]
    fn unbalanced_input_is_flagged_but_still_analyzed() {
        let f = analyze("rm -rf 'x");
        assert!(!f.parse_ok);
        assert!(f.programs.contains(&"rm".to_string()));
        assert!(analyze("ls -la").parse_ok);
    }

    #[test]
    fn commands_are_whitespace_normalized() {
        assert_eq!(
            analyze("git   status  &&  ls   -la").commands,
            ["git status", "ls -la"]
        );
    }

    #[test]
    fn deep_nesting_is_bounded() {
        let mut cmd = "rm -rf /".to_string();
        for _ in 0..6 {
            cmd = format!("bash -c {}", shell_quote(&cmd));
        }
        let f = analyze(&cmd);
        assert!(!f.parse_ok);
    }

    fn shell_quote(s: &str) -> String {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}
