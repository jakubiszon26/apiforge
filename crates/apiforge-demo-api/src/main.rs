//! Runs the local demo API on `127.0.0.1` (port 4318 unless `--port` is given).

use apiforge_demo_api::RequestLog;
use std::{
    io,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    process::ExitCode,
};

const USAGE: &str = "Usage: apiforge-demo-api [--port <port>] [--request-log <file>]

Serves the API Forge demo shop and support API on http://127.0.0.1:<port>/demo.
The default port is 4318.
--request-log appends one JSON line per request to <file>: the time, method, path
with query, status, JSON body up to 64 KiB, header names and the value of X-Client.";

#[derive(Debug, PartialEq)]
struct Options {
    port: u16,
    request_log: Option<PathBuf>,
}

fn main() -> ExitCode {
    let options = match parse_options(std::env::args().skip(1)) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let log = match options.request_log.as_deref().map(RequestLog::open) {
        None => None,
        Some(Ok(log)) => Some(log),
        Some(Err(error)) => {
            let path = options.request_log.unwrap_or_default();
            eprintln!(
                "The request log {} cannot be opened; the demo API cannot start: {error}",
                path.display()
            );
            return ExitCode::FAILURE;
        }
    };
    let port = options.port;
    match run(SocketAddr::from((Ipv4Addr::LOCALHOST, port)), log) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
            eprintln!("Port {port} is already in use; the demo API cannot start.");
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("Demo API failed: {error}");
            ExitCode::FAILURE
        }
    }
}

/// The options, or `None` when help was requested.
fn parse_options(mut args: impl Iterator<Item = String>) -> Result<Option<Options>, String> {
    let mut options = Options {
        port: apiforge_demo_api::DEFAULT_PORT,
        request_log: None,
    };
    while let Some(arg) = args.next() {
        let (name, value) = match arg.split_once('=') {
            Some((name, value)) => (name, Some(value.to_owned())),
            None => (arg.as_str(), None),
        };
        match (name, &value) {
            ("-h" | "--help", None) => return Ok(None),
            ("--port" | "--request-log", _) => {}
            _ => return Err(format!("Unknown argument: {arg}")),
        }
        let value = value
            .or_else(|| args.next())
            .ok_or_else(|| format!("{name} needs a value"))?;
        if name == "--port" {
            options.port = value
                .parse()
                .map_err(|_| format!("Invalid port: {value}"))?;
        } else {
            options.request_log = Some(PathBuf::from(value));
        }
    }
    Ok(Some(options))
}

fn run(addr: SocketAddr, log: Option<RequestLog>) -> io::Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()?;
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        println!("API Forge demo API: http://{}/demo", listener.local_addr()?);
        match log {
            Some(log) => apiforge_demo_api::serve_logged(listener, log).await,
            None => apiforge_demo_api::serve(listener).await,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Option<Options>, String> {
        parse_options(args.iter().map(|arg| (*arg).to_owned()))
    }

    fn options(port: u16, request_log: Option<&str>) -> Result<Option<Options>, String> {
        Ok(Some(Options {
            port,
            request_log: request_log.map(PathBuf::from),
        }))
    }

    #[test]
    fn arguments_select_the_port() {
        assert_eq!(parse(&[]), options(4318, None));
        assert_eq!(parse(&["--port", "8080"]), options(8080, None));
        assert_eq!(parse(&["--port=0"]), options(0, None));
        assert_eq!(parse(&["--help"]), Ok(None));
        assert!(parse(&["--port"]).is_err());
        assert!(parse(&["--port", "70000"]).is_err());
        assert!(parse(&["--host", "0.0.0.0"]).is_err());
        assert!(parse(&["--help=yes"]).is_err());
    }

    #[test]
    fn arguments_select_the_request_log() {
        assert_eq!(
            parse(&["--request-log", "a=b.jsonl", "--port", "80"]),
            options(80, Some("a=b.jsonl"))
        );
        assert_eq!(
            parse(&["--request-log=a=b.jsonl"]),
            options(4318, Some("a=b.jsonl"))
        );
        assert!(parse(&["--request-log"]).is_err());
    }
}
