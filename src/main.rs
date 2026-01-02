//! wa-encoder-rs - WeakAura encoder/decoder MCP server + CLI

use clap::{Parser, Subcommand};
use rmcp::{
    ServerHandler, ServiceExt,
    model::{ServerInfo, ServerCapabilities, Implementation, ProtocolVersion, CallToolResult, Content},
    tool, tool_router, tool_handler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    transport::stdio,
    ErrorData as McpError,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::fs;
use wa_encoder::{decode, encode, validate};

// === MCP Parameter Structs ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EncodeReq {
    #[schemars(description = "JSON string or file path to encode")]
    pub input: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DecodeReq {
    #[schemars(description = "WeakAura import string starting with !WA:")]
    pub import_string: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ValidateReq {
    #[schemars(description = "WeakAura import string to validate")]
    pub import_string: String,
}

// === MCP Server ===

#[derive(Clone)]
pub struct WaMcp {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl WaMcp {
    pub fn new() -> Self {
        Self { tool_router: Self::tool_router() }
    }

    #[tool(description = "Encode JSON to WeakAura import string. Input can be JSON string or file path.")]
    async fn wa_encode(&self, Parameters(p): Parameters<EncodeReq>) -> Result<CallToolResult, McpError> {
        let json = if p.input.trim().starts_with('{') {
            p.input.clone()
        } else {
            // Try as file path
            match fs::read_to_string(&p.input) {
                Ok(content) => content,
                Err(e) => return Ok(CallToolResult::success(vec![Content::text(format!("Error reading file: {}", e))])),
            }
        };

        match encode(&json) {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(result)])),
            Err(e) => Ok(CallToolResult::success(vec![Content::text(format!("Encode error: {}", e))])),
        }
    }

    #[tool(description = "Decode WeakAura import string to JSON")]
    async fn wa_decode(&self, Parameters(p): Parameters<DecodeReq>) -> Result<CallToolResult, McpError> {
        match decode(&p.import_string) {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(result)])),
            Err(e) => Ok(CallToolResult::success(vec![Content::text(format!("Decode error: {}", e))])),
        }
    }

    #[tool(description = "Validate a WeakAura import string and report any issues")]
    async fn wa_validate(&self, Parameters(p): Parameters<ValidateReq>) -> Result<CallToolResult, McpError> {
        let result = validate(&p.import_string);
        let mut output = String::new();

        if result.valid {
            output.push_str("Valid WeakAura string\n");
            if !result.warnings.is_empty() {
                output.push_str("\nWarnings:\n");
                for w in &result.warnings {
                    output.push_str(&format!("  - {}\n", w));
                }
            }
        } else {
            output.push_str("Invalid WeakAura string\n\nErrors:\n");
            for e in &result.errors {
                output.push_str(&format!("  - {}\n", e));
            }
        }

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }
}

#[tool_handler]
impl ServerHandler for WaMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("WeakAura encoder/decoder - 3 tools: wa_encode, wa_decode, wa_validate".into()),
        }
    }
}

// === CLI ===

#[derive(Parser)]
#[command(name = "wa-encoder-rs")]
#[command(about = "WeakAura encoder/decoder - CLI + MCP modes")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Encode JSON file or string to WeakAura import string
    Encode {
        /// JSON file path or JSON string
        input: String,
    },
    /// Decode WeakAura import string to JSON
    Decode {
        /// WeakAura import string (or - for stdin)
        #[arg(default_value = "-")]
        input: String,
    },
    /// Validate a WeakAura import string
    Validate {
        /// WeakAura import string
        input: String,
    },
    /// Quick encode/decode test
    Test,
    /// Dump decompressed bytes (debug)
    Dump {
        /// Byte offset to start
        #[arg(default_value = "0")]
        offset: usize,
        /// Number of bytes to show
        #[arg(default_value = "20")]
        count: usize,
    },
    /// Run as MCP server (default if no args)
    Mcp,
}

fn read_stdin() -> String {
    use std::io::{self, BufRead};
    let stdin = io::stdin();
    let mut lines = String::new();
    for line in stdin.lock().lines().map_while(Result::ok) {
        lines.push_str(&line);
    }
    lines
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Encode { input }) => {
            let json = if input.starts_with('{') {
                input
            } else {
                fs::read_to_string(&input)?
            };
            match encode(&json) {
                Ok(result) => println!("{}", result),
                Err(e) => eprintln!("Encode error: {}", e),
            }
        }

        Some(Commands::Decode { input }) => {
            let wa_string = if input == "-" { read_stdin() } else { input };
            match decode(&wa_string) {
                Ok(result) => println!("{}", result),
                Err(e) => eprintln!("Decode error: {}", e),
            }
        }

        Some(Commands::Validate { input }) => {
            let result = validate(&input);
            if result.valid {
                println!("Valid WeakAura string");
                for w in &result.warnings {
                    println!("  Warning: {}", w);
                }
            } else {
                println!("Invalid WeakAura string");
                for e in &result.errors {
                    println!("  Error: {}", e);
                }
            }
        }

        Some(Commands::Test) => {
            let json = r#"{"d":{"id":"Test Aura","regionType":"icon","width":64,"height":64,"xOffset":0,"yOffset":0},"c":[],"m":"d","s":"5.17.0","v":2000}"#;
            println!("Original JSON: {}", json);

            match encode(json) {
                Ok(encoded) => {
                    println!("Encoded: {}", encoded);
                    println!("Length: {}", encoded.len());

                    match decode(&encoded) {
                        Ok(decoded) => {
                            println!("Decoded: {}", decoded);
                            println!("Round-trip successful!");
                        }
                        Err(e) => eprintln!("Decode error: {}", e),
                    }
                }
                Err(e) => eprintln!("Encode error: {}", e),
            }
        }

        Some(Commands::Dump { offset, count }) => {
            let input = read_stdin();
            let trimmed = input.trim();

            if !trimmed.starts_with("!WA:") {
                eprintln!("Invalid WA string: starts with {:?}", &trimmed[..20.min(trimmed.len())]);
                return Ok(());
            }

            let after_wa = &trimmed[4..];
            let exclaim_pos = after_wa.find('!').unwrap();
            let encoded = &after_wa[exclaim_pos + 1..];
            eprintln!("Encoded length: {}", encoded.len());

            let compressed = wa_encoder::encodeforprint::decode_for_print(encoded).unwrap();

            use flate2::read::{DeflateDecoder, ZlibDecoder};
            use std::io::Read;
            let decompressed = {
                let mut decoder = DeflateDecoder::new(&compressed[..]);
                let mut buf = Vec::new();
                match decoder.read_to_end(&mut buf) {
                    Ok(_) => buf,
                    Err(_) => {
                        let mut decoder = ZlibDecoder::new(&compressed[..]);
                        let mut buf = Vec::new();
                        decoder.read_to_end(&mut buf).unwrap();
                        buf
                    }
                }
            };

            eprintln!("Decompressed: {} bytes", decompressed.len());

            for i in offset..(offset + count).min(decompressed.len()) {
                println!("  @{}: 0x{:02x} ({})", i, decompressed[i], decompressed[i]);
            }
        }

        Some(Commands::Mcp) | None => {
            // MCP server mode (default)
            let server = WaMcp::new().serve(stdio()).await?;
            server.waiting().await?;
        }
    }

    Ok(())
}
