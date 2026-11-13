use llcontrol_ipc::methods::*;
use serde_json::{json, Value};

fn display_settings() -> Value {
    json!({"version":1,"revision":7,"temperature_unit":"celsius","interface_language":"en-US"})
}

fn response(method: &str) -> Value {
    match method {
        "list_devices" => {
            json!([{"id":"lcd:test","family":"galahad2-lcdv2","display_name":"LCDV2","vid":1046,"pid":29589,"serial":null,"hid_path":"/dev/hidraw-test"}])
        }
        "list_sensors" => {
            json!([{"id":"cpu:temp","label":"CPU","unit":"celsius","value":42.0,"status":"valid","observed_unix_ms":1000,"age_ms":10}])
        }
        _ => display_settings(),
    }
}

#[test]
fn empty_request_methods_reject_unknown_fields_and_validate_responses() {
    for method in [
        LIST_DEVICES.name,
        LIST_SENSORS.name,
        GET_DISPLAY_SETTINGS.name,
    ] {
        validate_request(method, json!({})).unwrap();
        validate_request(method, json!({"unexpected":true})).unwrap_err();
        validate_response(method, &response(method)).unwrap();
        validate_response(method, &Value::Null).unwrap_err();
        validate_response(method, &json!({"unexpected":true})).unwrap_err();
    }
}

#[test]
fn display_settings_write_rejects_unknown_wrapper_and_nested_fields() {
    let valid = json!({"settings":display_settings()});
    validate_request(SET_DISPLAY_SETTINGS.name, valid).unwrap();
    validate_request(
        SET_DISPLAY_SETTINGS.name,
        json!({"settings":display_settings(),"unexpected":true}),
    )
    .unwrap_err();
    let mut nested = display_settings();
    nested["unexpected"] = json!(true);
    validate_request(SET_DISPLAY_SETTINGS.name, json!({"settings":nested})).unwrap_err();
    validate_response(SET_DISPLAY_SETTINGS.name, &display_settings()).unwrap();
    validate_response(
        SET_DISPLAY_SETTINGS.name,
        &json!({"version":99,"revision":0,"temperature_unit":"celsius"}),
    )
    .unwrap_err();
}

#[test]
fn registry_is_versioned_unique_and_covers_the_generated_typescript_contract() {
    assert_eq!(RPC_CONTRACT_VERSION, 1);
    let mut names = std::collections::BTreeSet::new();
    for method in METHODS {
        assert_eq!(method.version, 1);
        assert!(names.insert(method.name));
    }
    assert_eq!(names.len(), METHODS.len());
    let typescript = typescript_contract();
    for method in METHODS {
        assert!(typescript.contains(method.name));
    }
    assert!(typescript.contains("export interface Device"));
    assert!(typescript.contains("export interface Sensor"));
    assert!(typescript.contains("export interface DisplaySettings"));
}
