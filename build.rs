use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use rand::distributions::{Alphanumeric, DistString};
use std::fs::{remove_file, File};
use std::io::{Read, Write};
use std::process::Command;

#[cfg(debug_assertions)]
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

#[cfg(not(debug_assertions))]
fn main() {}

// this is actually disgusting

const STRING_DELIMITERS: &str = "\"'`";
const SKIP_PATTERNS: [&str; 6] = ["atob(", "btoa(", "http://", "https://", ".js", ".css"];

const ROTATE_KEY: u8 = 13;
const XOR_KEY: u8 = 0x7B;

struct StringToken {
    content: String,
    span: std::ops::Range<usize>,
}

impl StringToken {
    #[inline]
    fn should_skip(&self) -> bool {
        self.content.len() < 3
            || SKIP_PATTERNS
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

    decoder_function_name: String,
    decoder_function: String,
}

impl StringEncoder {
    fn new(source: String) -> Self {
        let chars = source.chars().collect();

        let decoder_function_name = format!(
            "_{}",
            Alphanumeric.sample_string(&mut rand::thread_rng(), 8)
        );
        
        let decoder_function = format!(
            "function {decoder_function_name}(s){{return atob(s).split('').map(c=>String.fromCharCode((c.charCodeAt(0)^{XOR_KEY}))).map(c=>String.fromCharCode((c.charCodeAt(0)+256-{ROTATE_KEY})%256)).join('')}}"
        );

        Self {
            output: source.clone(),
            source,
            chars,
            position: 0,
            offset: 0,

            decoder_function_name,
            decoder_function,
        }
    }

    pub fn process_file(input: &mut File, output: &mut File) -> std::io::Result<()> {
        let mut source = String::new();
        input.read_to_string(&mut source)?;

        let mut encoder = Self::new(source);
        encoder.process_strings();

        encoder
            .output
            .insert_str(encoder.output.len(), &encoder.decoder_function);

        output.write_all(encoder.output.as_bytes())
    }

    fn process_strings(&mut self) {
        while let Some(token) = self.next() {
            if !token.should_skip() {
                self.obfuscate_token(&token);
            }
        }
    }

    fn obfuscate_token(&mut self, token: &StringToken) {
        let replacement = {
            let content = &token.content[1..token.content.len() - 1];

            let xored = content
                .as_bytes()
                .iter()
                .map(|&b| b.wrapping_add(ROTATE_KEY))
                .map(|b| b ^ XOR_KEY)
                .collect::<Vec<_>>();

            let encoded_byte_delimited = BASE64_STANDARD
                .encode(xored)
                .bytes()
                .map(|b| b.to_string())
                .collect::<Vec<String>>()
                .join(",");

            format!(
                "{}(String.fromCharCode({encoded_byte_delimited}))",
                self.decoder_function_name
            )
        };

        {
            let adjusted_start = token.span.start + self.offset;
            let adjusted_end = token.span.end + self.offset;
            self.output
                .replace_range(adjusted_start..=adjusted_end, &replacement);
        }

        let old_len = token.span.end - token.span.start + 1;
        if replacement.len() > old_len {
            self.offset += replacement.len() - old_len;
        } else {
            self.offset -= old_len - replacement.len();
        }
    }
}

impl Iterator for StringEncoder {
    type Item = StringToken;

    fn next(&mut self) -> Option<Self::Item> {
        while self.position < self.chars.len() {
            let current_char = self.chars[self.position];

            if !STRING_DELIMITERS.contains(current_char) {
                self.position += 1;
                continue;
            }

            let start = self.position;
            let mut escaped = false;
            
            self.position += 1;

            while self.position < self.chars.len() {
                match (self.chars[self.position], escaped) {
                    ('\\', false) => {
                        escaped = true;
                        self.position += 1;
                    }
                    (c, false) if c == current_char => {
                        let token = StringToken {
                            content: self.source[start..=self.position].to_string(),
                            span: start..self.position,
                        };
                        self.position += 1;
                        return Some(token);
                    }
                    (_, _) => {
                        escaped = false;
                        self.position += 1;
                    }
                }
            }

            break;
        }
        None
    }
}
