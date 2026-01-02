# wa-encoder-rs

Native Rust WeakAura encoder/decoder. No Lua runtime required.

## Features

- **Encode** JSON to WeakAura import strings
- **Decode** WeakAura import strings to JSON
- **Validate** WeakAura strings and report issues
- **MCP Server** mode for Claude Desktop integration
- **CLI** for command-line usage

## Installation

```bash
cargo build --release
```

Binary will be at `target/release/wa-encoder-rs.exe`

## CLI Usage

```bash
# Encode JSON file to WA string
wa-encoder-rs encode aura.json

# Encode JSON string directly
wa-encoder-rs encode '{"d":{"id":"Test","regionType":"icon"}}'

# Decode WA string to JSON
wa-encoder-rs decode '!WA:2!...'

# Validate WA string
wa-encoder-rs validate '!WA:2!...'

# Quick round-trip test
wa-encoder-rs test

# Run as MCP server (default if no args)
wa-encoder-rs mcp
```

## MCP Server

Add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "wa-encoder": {
      "command": "X:\\path\\to\\wa-encoder-rs.exe",
      "args": []
    }
  }
}
```

### MCP Tools

| Tool | Description |
|------|-------------|
| `wa_encode` | Encode JSON to WeakAura import string |
| `wa_decode` | Decode WeakAura import string to JSON |
| `wa_validate` | Validate a WeakAura string and report issues |

## Technical Details

### WeakAura String Format

```
!WA:2!<encoded_data>
```

Where `<encoded_data>` is:
1. **LibSerialize** - Lua table serialization (big-endian for multi-byte lengths)
2. **DEFLATE** - Compression
3. **EncodeForPrint** - Custom base64 with alphabet `abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789()` (little-endian byte order)

### Key Implementation Notes

- LibSerialize uses **big-endian** for multi-byte integer lengths
- EncodeForPrint uses **little-endian** for byte packing
- String references are **1-based** (Lua convention)
- WeakAura JSON wrapper requires: `"m": "d"`, `"s": "5.17.0"`, `"v": 2000`

## Dependencies

- `flate2` - DEFLATE compression
- `serde_json` - JSON parsing
- `rmcp` - MCP server SDK
- `clap` - CLI parsing

## License

MIT
