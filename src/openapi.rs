use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub struct OpenApiDocument {
    pub openapi: String,
    pub info: Info,
    pub servers: Vec<Server>,
    pub paths: HashMap<String, PathItem>,
}

#[derive(Serialize, Deserialize)]
pub struct Info {
    pub title: String,
    pub description: String,
    pub version: String,
    pub contact: Contact,
}

#[derive(Serialize, Deserialize)]
pub struct Contact {
    pub name: String,
    pub email: String,
    pub url: String,
}

#[derive(Serialize, Deserialize)]
pub struct Server {
    pub url: String,
    pub description: String,
}

#[derive(Serialize, Deserialize)]
pub struct Operation {
    pub summary: String,
    pub description: String,
    pub operation_id: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub parameters: Vec<Parameter>,
}

#[derive(Serialize, Deserialize)]
pub struct PathItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub get: Option<Operation>,
}

#[derive(Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "in")]
    pub location: String,
    pub description: String,
    pub required: bool,
    pub schema: Schema,
}

#[derive(Serialize, Deserialize)]
struct RequestBody {
    required: bool,
    content: HashMap<String, MediaType>,
}

#[derive(Serialize, Deserialize)]
pub struct MediaType {
    pub schema: Schema,
}

#[derive(Serialize, Deserialize)]
pub struct Response {
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<HashMap<String, MediaType>>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum Schema {
    Reference {
        #[serde(rename = "$ref")]
        reference: String,
    },
    ArraySchema {
        #[serde(rename = "type")]
        schema_type: String,
        items: Box<Schema>,
    },
    ObjectSchema {
        #[serde(rename = "type")]
        schema_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        format: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        default: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        minimum: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        maximum: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        min_length: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_length: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        read_only: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        properties: Option<HashMap<String, Box<Schema>>>,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        required: Vec<String>,
    },
}

#[derive(Serialize, Deserialize)]
struct Components {
    schemas: HashMap<String, Schema>,
}
