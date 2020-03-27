use std::env::current_dir;
use std::fs::{create_dir_all, write};
use std::path::PathBuf;

use schemars::{schema::RootSchema, schema_for};

use queue::contract::{CountResponse, HandleMsg, InitMsg, Item, QueryMsg, SumResponse};

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();

    export_schema(&schema_for!(InitMsg), &out_dir);
    export_schema(&schema_for!(HandleMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
    export_schema(&schema_for!(Item), &out_dir);
    export_schema(&schema_for!(CountResponse), &out_dir);
    export_schema(&schema_for!(SumResponse), &out_dir);
}

/// Writes schema to file. Overwrites existing file.
/// Panics on any error writing out the schema.
fn export_schema(schema: &RootSchema, out_dir: &PathBuf) -> () {
    let title = schema
        .schema
        .metadata
        .as_ref()
        .map(|b| b.title.clone().unwrap_or("untitled".to_string()))
        .unwrap_or("unknown".to_string());
    let path = out_dir.join(format!("{}.json", title));
    let json = serde_json::to_string_pretty(schema).unwrap();
    write(&path, json + "\n").unwrap();
    println!("Created {}", path.to_str().unwrap());
}
