use serde_json::{Value, json};

/// Normalize schemas for strict outputs while retaining nullable fields and references.
pub fn strict_schema(mut schema: Value) -> Value {
    fn visit(value: &mut Value) {
        match value {
            Value::Object(object) => {
                for key in ["default", "format", "$schema"] { object.remove(key); }
                if object.get("type").is_some_and(|v|v=="integer") && object.get("minimum").is_some_and(|v|v==0) { object.remove("minimum"); }
                if let Some(alternatives) = object.remove("oneOf") { object.insert("anyOf".into(), alternatives); }
                if object.contains_key("properties") || object.get("type").is_some_and(|value| value == "object") {
                    object.insert("additionalProperties".into(), json!(false));
                    let required = object.get("properties").and_then(Value::as_object).map(|properties| properties.keys().cloned().collect::<Vec<_>>()).unwrap_or_default();
                    object.insert("required".into(), json!(required));
                }
                for key in ["properties", "$defs", "definitions", "patternProperties"] {
                    if let Some(children)=object.get_mut(key).and_then(Value::as_object_mut) { for child in children.values_mut() { visit(child); } }
                }
                for key in ["items", "anyOf", "allOf", "not", "additionalProperties", "prefixItems"] { if let Some(child)=object.get_mut(key) { visit(child); } }
            }
            Value::Array(values) => for child in values { visit(child); },
            _ => {},
        }
    }
    visit(&mut schema); schema
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn agents_schema_preserves_property_names() {
        let schema=strict_schema(json!({"type":"object","properties":{"default":{"type":"string","default":"old"},"format":{"type":"string"},"oneOf":{"type":"integer"}}}));
        assert_eq!(schema["properties"].as_object().unwrap().len(),3);
        assert!(schema["properties"]["default"].get("default").is_none());
    }
    #[test] fn agents_strict_nullable_tagged_schema() {
        let schema = strict_schema(serde_json::to_value(schemars::schema_for!(crate::viz::Scene)).unwrap());
        fn check(value: &Value) { if let Some(object) = value.as_object() { assert!(!object.contains_key("oneOf")); if let Some(properties) = object.get("properties").and_then(Value::as_object) { assert_eq!(object["additionalProperties"], false); assert_eq!(object["required"].as_array().unwrap().len(),properties.len()); } for child in object.values() { check(child); } } else if let Some(values) = value.as_array() { for child in values { check(child); } } }
        check(&schema);
        let scene=json!({"title":"Tree","input":"[]","frames":[{"caption":"Empty tree","line":null,"structures":[{"kind":"tree","label":"root","root":null,"nodes":[{"id":"n","value":"1","left":null,"right":null,"tone":"default"}]}]}]});
        assert!(serde_json::from_value::<crate::viz::Scene>(scene).is_ok());
        #[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)] #[serde(tag="kind")] enum Nullable { Answer { note: Option<String> } }
        let strict = strict_schema(serde_json::to_value(schemars::schema_for!(Nullable)).unwrap());
        assert!(strict.to_string().contains("null"));
        assert!(serde_json::from_value::<Nullable>(json!({"kind":"Answer","note":null})).is_ok());
    }
}
