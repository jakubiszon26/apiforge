//! Runs the local demo API on `127.0.0.1` (port 4318 unless `--port` is given).

use std::{
    io,
    net::{Ipv4Addr, SocketAddr},
    process::ExitCode,
};

const USAGE: &str = "Usage: apiforge-demo-api [--port <port>]

Serves the API Forge demo shop and support API on http://127.0.0.1:<port>/demo.
The default port is 4318.";

fn main() -> ExitCode {
    let port = match parse_port(std::env::args().skip(1)) {
        Ok(Some(port)) => port,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(SocketAddr::from((Ipv4Addr::LOCALHOST, port))) {
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

/// The requested port, or `None` when help was requested.
fn parse_port(mut args: impl Iterator<Item = String>) -> Result<Option<u16>, String> {
    let mut port = apiforge_demo_api::DEFAULT_PORT;
    while let Some(arg) = args.next() {
        let value = match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--port" => args.next().ok_or("--port needs a value")?,
            other => match other.strip_prefix("--port=") {
                Some(value) => value.to_owned(),
                None => return Err(format!("Unknown argument: {other}")),
            },
        };
        port = value
            .parse()
            .map_err(|_| format!("Invalid port: {value}"))?;
    }
    Ok(Some(port))
}

fn run(addr: SocketAddr) -> io::Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()?;
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        println!("API Forge demo API: http://{}/demo", listener.local_addr()?);
        apiforge_demo_api::serve(listener).await
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Option<u16>, String> {
        parse_port(args.iter().map(|arg| (*arg).to_owned()))
    }

    #[test]
    fn arguments_select_the_port() {
        assert_eq!(parse(&[]), Ok(Some(4318)));
        assert_eq!(parse(&["--port", "8080"]), Ok(Some(8080)));
        assert_eq!(parse(&["--port=0"]), Ok(Some(0)));
        assert_eq!(parse(&["--help"]), Ok(None));
        assert!(parse(&["--port"]).is_err());
        assert!(parse(&["--port", "70000"]).is_err());
        assert!(parse(&["--host", "0.0.0.0"]).is_err());
    }
}
