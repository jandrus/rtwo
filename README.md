<div align="center">
  <h1> rtwo </h1>
  <h2> Ollama CLI tool in Rust. Used to query and manage ollama server </h2>
</div>


## Features

- Terminal based
- Response highlights
- Chat history
- Download/Delete models from ollama server
- Simple
- Logging

_________

## Demo

<img src="https://github.com/jandrus/rtwo/blob/main/demo/demo.gif?raw=true">

_________

## Installation

### Binary releases

You can download the pre-built binaries from the [release page](https://github.com/jandrus/rtwo/releases)

### crates.io
`rtwo` can be installed from [crates.io](https://crates.io/crates/rtwo)

```shell
cargo install rtwo
```

### Build from source

#### Prerequisites:
- [Rust](https://www.rust-lang.org/) 
- [Cargo package manager](https://doc.rust-lang.org/cargo/).

#### Build

1. Clone the repo: ```git clone PATH_TO_REPO```
2. Build: ```cargo build --release```

This will produce an binary executable file at `target/release/rtwo` that you can copy to a directory in your `$PATH`.

_________

## Configuration

rtwo can be configured using a TOML configuration file. The file is located at:
- Linux : `$HOME/.config/rtwo/rtwo.toml`
- Mac : `$HOME/Library/Application Support/rtwo/rtwo.toml`
- Windows : `{FOLDERID_RoamingAppData}\rtwo\config\rtwo.toml`.

The default configuration is:
``` toml
host = "localhost"
port = 11434
model = "llama3"
verbose = false
color = true
save = true
stream = false
```

- host:    target host for ollama server
- port:    target port for ollama server
- model:   model to query
- verbose: enable/disable verbose output from responses (See Usage)
- color:   enable/disable color output from responses
- save:    enable/disable saving responses to DB (`$HOME/.local/share/rtwo/rtwo.db`)
- stream:  enable/disable streaming response to terminal
  - NOTE: `false` is the recommended value. Setting this to true disables markdown output formatting. However, for slower ollama servers or larger models, it may be advantageous to set this value to `true`.

_________

## Usage
``` shell
  -H, --host <HOST>
          Host address for ollama server. e.g.: localhost, 192.168.1.5, etc.
          Can be set in configuration file.
          Default: localhost

  -p, --port <PORT>
          Host port for ollama server. e.g.: 11434, 1776, etc.
          Can be set in configuration file.
          Default: 11434

  -m, --model <MODEL>
          Model name to query. e.g.: mistral, llama3:70b, etc.
          NOTE: If model is not available on HOST, rtwo will not automatically download the model to the HOST. Use "pull" [-P, --pull] to download the model to the HOST.
          Can be set in configuration file.
          Default: llama3

  -v, --verbose
          Enable verbose output. Prints: model, tokens in prompt, tokens in response, and time taken after response is rendered to user.
          Example:
          	* Model: llama3:70b
          	* Tokens in prompt: 23
          	* Tokens in response: 216
          	* Time taken: 27.174
          Can be set in configuration file.
          Default: false

  -c, --color
          Enable color output.
          Can be set in configuration file.
          Default: true

  -n, --nocolor
          Disable color output.
          Can be set in configuration file [color = false].

  -s, --save
          Save conversation for recall (places conversation in DB)
          Can be set in configuration file.
          Default: true

  -i, --incogneto
          Do NOT save conversation for recall.
          Can be set in configuration file [save = false].

  -b, --batch
          Do not stream llm output (wait for full response to generate before rendering to user).
          NOTE: This allows response formatting.
          Can be set in configuration file [stream = false].

  -S, --stream
          Stream llm output (Display response as it is rendered by host)
          NOTE: This disables response formatting.
          Can be set in configuration file.
          Default: false

  -l, --list
          List previous conversations

  -L, --listmodel
          List available models on ollama server (HOST:PORT)

  -r, --restore
          Select previous conversation from local storage and pick up where you left off. This restores the context
          from a saved conversation and prints the saved output.
          Interactive

  -d, --delete
          Delete previous conversations from local storage.
          NOTE: action is irreversible.
          Interactive

  -P, --pull <MODEL>
          Pull model to ollama server for use (downloads model on HOST). e.g.: llama3.

  -D, --delete-model <MODEL>
          Delete model from ollama server (deletes model on HOST). e.g.: llama2.

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

_________

## Donate
- BTC: bc1q7y20wr2n5qt2fxe569llvz5a0qsnpsz4decplr
- XMR: 84t9GUWQVJSGxF8cbMtRBd67YDAHnTsrdWVStcdpiwcAcAnVy21U6RmLdwiQdbfsyu16UqZn6qj1gGheTMkHkYA4HbVN4zS

_________

## License

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <http://www.gnu.org/licenses/>.
