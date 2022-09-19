use std::collections::HashMap;

use cosmwasm_schema::{generate_api, QueryResponses, IDL_VERSION};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {
    pub admin: String,
    pub cap: u128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Mint { amount: u128 },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema, QueryResponses)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    #[returns(u128)]
    Balance { account: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SudoMsg {
    SetAdmin { new_admin: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct MigrateMsg {
    pub admin: String,
    pub cap: u128,
}

#[test]
fn test_basic_structure() {
    let api_str = generate_api! {
        name: "test",
        version: "0.1.0",
        instantiate: InstantiateMsg,
        query: QueryMsg,
        execute: ExecuteMsg,
        sudo: SudoMsg,
        migrate: MigrateMsg,
    }
    .render()
    .to_string()
    .unwrap();

    let api_json: HashMap<String, Value> = serde_json::from_str(&api_str).unwrap();
    assert_eq!(api_json.get("idl_version").unwrap(), IDL_VERSION);
    assert_eq!(api_json.get("contract_name").unwrap(), "test");
    assert_eq!(api_json.get("contract_version").unwrap(), "0.1.0");
    assert_eq!(
        api_json.get("instantiate").unwrap().get("title").unwrap(),
        "InstantiateMsg"
    );
    assert_eq!(
        api_json.get("execute").unwrap().get("title").unwrap(),
        "ExecuteMsg"
    );
    assert_eq!(
        api_json.get("query").unwrap().get("title").unwrap(),
        "QueryMsg"
    );
    assert_eq!(
        api_json.get("migrate").unwrap().get("title").unwrap(),
        "MigrateMsg"
    );
    assert_eq!(
        api_json.get("sudo").unwrap().get("title").unwrap(),
        "SudoMsg"
    );
}

#[test]
fn test_query_responses() {
    let api_str = generate_api! {
        instantiate: InstantiateMsg,
        query: QueryMsg,
    }
    .render()
    .to_string()
    .unwrap();

    let api: Value = serde_json::from_str(&api_str).unwrap();
    let queries = api
        .get("query")
        .unwrap()
        .get("oneOf")
        .unwrap()
        .as_array()
        .unwrap();

    // Find the "balance" query in the queries schema
    assert_eq!(queries.len(), 1);
    assert_eq!(
        queries[0].get("required").unwrap().get(0).unwrap(),
        "balance"
    );

    // Find the "balance" query in responses
    api.get("responses").unwrap().get("balance").unwrap();
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema, QueryResponses)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsgWithGenerics<T: std::fmt::Debug>
where
    T: JsonSchema,
{
    #[returns(u128)]
    QueryData { data: T },
}

#[test]
fn test_query_responses_generics() {
    let api_str = generate_api! {
        instantiate: InstantiateMsg,
        query: QueryMsgWithGenerics<u32>,
    }
    .render()
    .to_string()
    .unwrap();

    let api: Value = serde_json::from_str(&api_str).unwrap();
    let queries = api
        .get("query")
        .unwrap()
        .get("oneOf")
        .unwrap()
        .as_array()
        .unwrap();

    // Find the "balance" query in the queries schema
    assert_eq!(queries.len(), 1);
    assert_eq!(
        queries[0].get("required").unwrap().get(0).unwrap(),
        "query_data"
    );

    // Find the "balance" query in responses
    api.get("responses").unwrap().get("query_data").unwrap();
}
