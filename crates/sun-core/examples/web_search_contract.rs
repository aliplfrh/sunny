//! Test bridge: validate actual frontend activation arguments with the Rust parser.
//! Never opens a browser or performs network access.
use serde::Deserialize;
use std::io::{self, Read};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    query: String,
    engine_id: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let request: Request = serde_json::from_str(&input)?;
    let engine = sun_core::websearch::engine_by_id(&request.engine_id);
    let (_, url) = sun_core::websearch::prepare(&request.query, &engine)
        .map_err(|error| format!("{error:?}"))?;
    println!("{url}");
    Ok(())
}
