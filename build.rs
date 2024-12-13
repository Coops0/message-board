use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use std::fs::{remove_file, File};
use std::io::{Read, Write};
use std::process::Command;

fn main() {
    // println!("cargo:rerun-if-changed=templates/");
    // println!("cargo:rerun-if-changed=assets/user-script.js");

    let ts_status = Command::new("./tailwindcss")
        .args(["-i", "assets/index.css", "-o", "assets/ts.css", "-m"])
        .status()
        .unwrap();

    assert!(ts_status.success());

    {
        let mut input = File::open("assets/user-script.js").unwrap();
        let mut output = File::create("assets/user-script.obf.js").unwrap();

        StringEncoder::process_file(&mut input, &mut output).unwrap();
    };

    let obf_status = Command::new("terser")
        .args([
            "assets/user-script.obf.js",
            "-c",
            "-m",
            "--toplevel",
            "-o",
            "assets/user-script.min.js",
        ])
        .status()
        .unwrap();
    
    assert!(obf_status.success());
    
    remove_file("assets/user-script.obf.js").unwrap();
}

const STRING_DELIMITERS: &[char] = &['"', '\'', '`'];
const SKIP_PATTERNS: &[&str] = &["atob(", "btoa(", "http://", "https://", ".js", ".css"];

struct StringToken {
    content: String,
    start: usize,
    end: usize,
}

impl StringToken {
    fn should_skip(&self) -> bool {
        if self.content.len() < 3 {
            return true;
        }

        SKIP_PATTERNS
            .iter()
            .any(|&pattern| self.content.contains(pattern))
    }
}

struct StringEncoder {
    source: String,
    chars: Vec<char>,
    position: usize,
    output: String,
    offset: usize,
}

impl StringEncoder {
    fn new(source: String) -> Self {
        let chars: Vec<char> = source.chars().collect();
        Self {
            source: source.clone(),
            chars,
            position: 0,
            output: source,
            offset: 0,
        }
    }

    fn process_file(input: &mut File, output: &mut File) -> std::io::Result<()> {
        let mut source = String::new();
        input.read_to_string(&mut source)?;

        let mut obfuscator = Self::new(source);
        obfuscator.process_strings();

        output.write_all(obfuscator.output.as_bytes())?;
        output.flush()?;
        Ok(())
    }

    fn process_strings(&mut self) {
        while self.position < self.chars.len() {
            if let Some(token) = self.next() {
                if !token.should_skip() {
                    self.obfuscate_token(&token);
                }
            }

            self.position += 1;
        }
    }

    fn obfuscate_token(&mut self, token: &StringToken) {
        let content = &token.content[1..token.content.len() - 1];
        let encoded = BASE64_STANDARD.encode(content);

        let replacement_code = format!("atob(\"{encoded}\")");

        self.output.replace_range(
            token.start + self.offset..=token.end + self.offset,
            &replacement_code,
        );

        self.offset += replacement_code.len() - (token.end - token.start + 1);
    }
}

impl Iterator for StringEncoder {
    type Item = StringToken;

    fn next(&mut self) -> Option<Self::Item> {
        let current_char = self.chars[self.position];

        if !STRING_DELIMITERS.contains(&current_char) {
            return None;
        }

        let start = self.position;
        let mut escaped = false;
        self.position += 1;

        while self.position < self.chars.len() {
            let c = self.chars[self.position];

            if c == '\\' && !escaped {
                escaped = true;
                self.position += 1;
                continue;
            }

            if c == current_char && !escaped {
                return Some(StringToken {
                    content: self.source[start..=self.position].to_string(),
                    start,
                    end: self.position,
                });
            }

            escaped = false;
            self.position += 1;
        }

        None
    }
}