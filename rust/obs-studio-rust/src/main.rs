//! `obs-studio-rust`: control OBS streaming from the command line.

#![forbid(unsafe_code)]

use std::process::ExitCode;

mod mcp;

use obs_studio_rust::{Client, DEFAULT_PORT, describe_stream_status};

const USAGE: &str = "\
Usage: obs-studio-rust [OPTIONS] <COMMAND>
       obs-studio-rust --mcp

Control OBS Studio streaming over obs-websocket v5.

Commands:
  status    Show whether OBS is streaming
  start     Start streaming
  stop      Stop streaming
  toggle    Start streaming if stopped, stop it if live
  version   Show the OBS and obs-websocket versions

Options:
  --host <HOST>          obs-websocket host [default: localhost]
  --port <PORT>          obs-websocket port [default: 4455]
  --password <PASSWORD>  obs-websocket password [env: OBS_WEBSOCKET_PASSWORD]
  --json                Print response data as JSON
  --mcp                  Start libobs and serve MCP over stdin/stdout (needs a
                         build with --features bundled-native); logs go to stderr
  -h, --help             Print help
  -V, --version          Print version
";

struct Args {
    host: String,
    port: u16,
    password: Option<String>,
    json: bool,
    mcp: bool,
    command: String,
}

fn parse_args() -> Result<Option<Args>, String> {
    let mut host = "localhost".to_owned();
    let mut port = DEFAULT_PORT;
    let mut password = std::env::var("OBS_WEBSOCKET_PASSWORD")
        .ok()
        .filter(|p| !p.is_empty());
    let mut command = None;
    let mut json = false;
    let mut mcp = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("obs-studio-rust {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "--host" => host = value("--host")?,
            "--port" => {
                let raw = value("--port")?;
                port = raw.parse().map_err(|_| format!("invalid port: {raw}"))?;
            }
            "--password" => password = Some(value("--password")?),
            "--json" => json = true,
            "--mcp" => mcp = true,
            _ if arg.starts_with('-') => return Err(format!("unknown option: {arg}")),
            _ if command.is_none() => command = Some(arg),
            _ => return Err(format!("unexpected argument: {arg}")),
        }
    }
    let command = match command {
        Some(command) => command,
        None if mcp => String::new(),
        None => return Err("missing command".to_owned()),
    };
    if mcp && !command.is_empty() {
        return Err("--mcp takes no command".to_owned());
    }
    Ok(Some(Args {
        host,
        port,
        password,
        json,
        mcp,
        command,
    }))
}

fn run(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let request = match args.command.as_str() {
        "status" => "GetStreamStatus",
        "start" => "StartStream",
        "stop" => "StopStream",
        "toggle" => "ToggleStream",
        "version" => "GetVersion",
        other => return Err(format!("unknown command: {other}").into()),
    };
    let mut client = Client::connect(&args.host, args.port, args.password.as_deref())?;
    let response = client.request(request, None)?;
    if args.json {
        println!("{response}");
    } else {
        match args.command.as_str() {
            "status" => println!("{}", describe_stream_status(&response)),
            "start" => println!("stream: starting"),
            "stop" => println!("stream: stopping"),
            "toggle" => match response.get("outputActive").and_then(|v| v.as_bool()) {
                Some(true) => println!("stream: starting"),
                _ => println!("stream: stopping"),
            },
            _ => println!(
                "OBS {} (obs-websocket {}, {})",
                response["obsVersion"].as_str().unwrap_or("?"),
                response["obsWebSocketVersion"].as_str().unwrap_or("?"),
                response["platformDescription"].as_str().unwrap_or("?"),
            ),
        }
    }
    client.close()?;
    Ok(())
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(Some(args)) => args,
        Ok(None) => return ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let result = if args.mcp {
        mcp::run().map_err(Box::from)
    } else {
        run(&args)
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
