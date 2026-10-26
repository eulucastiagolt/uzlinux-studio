pub mod inbound;
pub mod outbound;
pub mod registration;

pub use inbound::*;
pub use outbound::*;
pub use registration::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_set_title() {
        let json = r#"{
            "event": "setTitle",
            "context": "Keypad.0.0",
            "payload": {
                "title": "My Title",
                "target": 0,
                "state": 0
            }
        }"#;

        let parsed: InboundEvent = serde_json::from_str(json).expect("Should parse setTitle");
        match parsed {
            InboundEvent::SetTitle(msg) => {
                assert_eq!(msg.context, "Keypad.0.0");
                assert_eq!(msg.payload.title, "My Title");
                assert_eq!(msg.payload.state, Some(0));
            }
            _ => panic!("Expected SetTitle variant"),
        }
    }

    #[test]
    fn test_parse_device_registered() {
        let json = r#"{
            "event": "device_registered",
            "device": "e9-02C53A015U3671380",
            "name": "Ulanzi D200",
            "size": [5, 3],
            "type": 0
        }"#;

        let parsed: InboundEvent = serde_json::from_str(json).expect("Should parse device_registered");
        match parsed {
            InboundEvent::DeviceRegistered(msg) => {
                assert_eq!(msg.device, "e9-02C53A015U3671380");
                assert_eq!(msg.name, Some("Ulanzi D200".to_string()));
                assert_eq!(msg.size, Some(vec![5, 3]));
            }
            _ => panic!("Expected DeviceRegistered variant"),
        }
    }

    #[test]
    fn test_serialize_key_down() {
        let event = OutboundEvent::KeyDown(KeyEventMessage {
            action: "com.amansprojects.starterpack.switchprofile".to_string(),
            context: "Keypad.0.0".to_string(),
            device: "e9-02C53A015U3671380".to_string(),
            payload: KeyEventPayload {
                settings: serde_json::json!({ "profile": "Live" }),
                coordinates: Coordinates { column: 0, row: 0 },
                state: 0,
                user_desired_state: 0,
                is_in_multi_action: false,
                controller: "Keypad".to_string(),
            },
        });

        let serialized = serde_json::to_string(&event).expect("Should serialize KeyDown");
        assert!(serialized.contains(r#""event":"keyDown""#));
        assert!(serialized.contains(r#""context":"Keypad.0.0""#));
        assert!(serialized.contains(r#""isInMultiAction":false"#));
        assert!(serialized.contains(r#""userDesiredState":0"#));
        assert!(serialized.contains(r#""controller":"Keypad""#));
    }

    #[test]
    fn test_serialize_will_appear() {
        let event = OutboundEvent::WillAppear(AppearanceMessage {
            action: "me.amankhanna.oampris.previous".to_string(),
            context: "Keypad.0.0".to_string(),
            device: "e9-02C53A015U3671380".to_string(),
            payload: AppearancePayload {
                settings: serde_json::json!({}),
                coordinates: Coordinates { column: 0, row: 0 },
                state: 0,
                is_in_multi_action: false,
                controller: "Keypad".to_string(),
            },
        });

        let serialized = serde_json::to_string(&event).expect("Should serialize WillAppear");
        assert!(serialized.contains(r#""event":"willAppear""#));
        assert!(serialized.contains(r#""isInMultiAction":false"#));
        assert!(serialized.contains(r#""controller":"Keypad""#));
    }
}
