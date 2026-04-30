//! Service topic data plumbing (Pulsar producer + consumer) and helpers.
//!
//! Mirrors the DFC Web "服务请求" page (`CmdPage.razor`):
//! sends `SvrReqRecord` payloads to `thing_service-BZ-REQUEST-<id>` and
//! consumes `SvrRespRecord` payloads from `thing_service-BZ-RESPONSE-<id>`.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use chrono::Local;
use crossbeam_channel::{Receiver, Sender};
use futures::StreamExt;
use prost::Message;
use tokio::sync::watch;

use super::config_view::{decode_framed_iothub_message, format_clock_time};
use crate::proto::iothub::{AnyValue, EventRecord, EventRecordList, SvrReqRecord, SvrRespRecord};
use crate::states::ServiceResponseRow;

/// Context key for an `SvrReqRecord` carried inside an `EventRecord`.
pub const SVR_REQ_KEY: &str = "svrReq";
/// Context key for an `SvrRespRecord` carried inside an `EventRecord`.
pub const SVR_RESP_KEY: &str = "svrResp";

/// Preset service request types (label, IMR). The last entry maps to "自定义"
/// and signals the form to use the manual IMR input field.
pub const REQUEST_TYPES: &[(&str, &str)] = &[
    ("启动", "WindTurbine/SERVICE/WTUR/Start"),
    ("停机", "WindTurbine/SERVICE/WTUR/Stop"),
    ("复位", "WindTurbine/SERVICE/WTUR/Reset"),
    ("维护", "WindTurbine/SERVICE/WTUR/Maintain"),
    ("左偏航", "WindTurbine/SERVICE/WTUR/LeftYaw"),
    ("右偏航", "WindTurbine/SERVICE/WTUR/RightYaw"),
    ("停止偏航", "WindTurbine/SERVICE/WTUR/StopYaw"),
    ("自动偏航", "WindTurbine/SERVICE/WTUR/AutoYaw"),
    ("有功控制", "WindTurbine/SERVICE/WTUR/PwrAtControl"),
    ("无功控制", "WindTurbine/SERVICE/WTUR/PwrReactControl"),
    ("功率因数控制", "WindTurbine/SERVICE/WTUR/PFControl"),
    ("停机模式字", "WindTurbine/SERVICE/WTUR/StopByModel"),
    ("人工置数(IoT)", "WindTurbine/SERVICE/BASE/SetValueToIoT"),
    (
        "人工置数(Device)",
        "WindTurbine/SERVICE/BASE/SetValueToDevice",
    ),
    ("锁定", "WindTurbine/SERVICE/BASE/LockService"),
    ("解锁", "WindTurbine/SERVICE/BASE/UnlockService"),
    ("挂牌", "WindTurbine/SERVICE/BASE/PutUpShield"),
    ("摘牌", "WindTurbine/SERVICE/BASE/PutDownShield"),
    ("PLC校时", "WindTurbine/SERVICE/WTUR/PlcTiming"),
    ("自定义", ""),
];

/// Index of the "自定义" entry — used as the default selection.
pub const CUSTOM_TYPE_INDEX: usize = REQUEST_TYPES.len() - 1;

pub(super) fn normalize_pulsar_service_url(raw: &str) -> Option<String> {
    let cleaned = raw.trim().trim_matches('"').trim_matches('\'').trim();
    if cleaned.is_empty() {
        return None;
    }

    (!pulsar_service_url_candidates(cleaned).is_empty()).then(|| cleaned.to_string())
}

pub(super) fn pulsar_service_url_candidates(raw: &str) -> Vec<String> {
    let cleaned = raw.trim().trim_matches('"').trim_matches('\'').trim();
    if cleaned.is_empty() {
        return Vec::new();
    }

    if let Some((scheme, rest)) = cleaned.split_once("://") {
        return pulsar_service_url_candidates_with_scheme(rest, &format!("{scheme}://"));
    }

    pulsar_service_url_candidates_with_scheme(cleaned, "pulsar://")
}

fn pulsar_service_url_candidates_with_scheme(raw: &str, scheme: &str) -> Vec<String> {
    let (authority_list, path) = match raw.split_once('/') {
        Some((authority, path)) => (authority, Some(path)),
        None => (raw, None),
    };

    if path.is_some_and(|path| !path.is_empty()) {
        return Vec::new();
    }

    authority_list
        .split(',')
        .map(str::trim)
        .filter(|authority| !authority.is_empty())
        .filter_map(|authority| normalize_pulsar_authority(authority, scheme))
        .collect()
}

fn normalize_pulsar_authority(authority: &str, scheme: &str) -> Option<String> {
    if authority.is_empty()
        || authority.contains('{')
        || authority.contains('}')
        || authority.contains('/')
        || authority.contains(';')
        || authority.contains(',')
        || authority.chars().any(char::is_whitespace)
    {
        return None;
    }

    Some(format!("{scheme}{authority}"))
}

pub(super) async fn build_pulsar_client_with_fallbacks(
    service_urls: &[String],
    token: Option<&str>,
) -> Result<(pulsar::Pulsar<pulsar::TokioExecutor>, String), String> {
    if service_urls.is_empty() {
        return Err("无法解析 Pulsar service URL".to_string());
    }

    let mut errors = Vec::new();

    for service_url in service_urls {
        let mut builder = pulsar::Pulsar::builder(service_url.clone(), pulsar::TokioExecutor);
        if let Some(token) = token {
            builder = builder.with_auth(pulsar::Authentication {
                name: "token".to_string(),
                data: token.as_bytes().to_vec(),
            });
        }

        match builder.build().await {
            Ok(client) => return Ok((client, service_url.clone())),
            Err(err) => {
                tracing::warn!(
                    service_url = %service_url,
                    "Failed to connect Pulsar client: {}",
                    err
                );
                errors.push(format!("{service_url}: {err}"));
            }
        }
    }

    Err(errors.join(" | "))
}

/// Events emitted by the background stream loop into the GPUI side.
#[derive(Debug)]
pub enum ServiceStreamEvent {
    Response(ServiceResponseRow),
    Error(String),
}

/// Publish job sent from the form submit handler to the producer task.
#[derive(Debug, Clone)]
pub struct ServicePublishRequest {
    pub device: String,
    pub record: SvrReqRecord,
}

/// Build the Pulsar payload for one service request.
///
/// Layout: `[0x20, 0x02, 0x00] || EventRecordList { event_array: [EventRecord {
///   src: device, context: { "svrReq": AnyValue { anyV.value: SvrReqRecord encoded } }
/// }] }`
pub fn build_service_request_payload(device: &str, req: &SvrReqRecord) -> Vec<u8> {
    let req_bytes = req.encode_to_vec();

    let mut context = std::collections::HashMap::new();
    context.insert(
        SVR_REQ_KEY.to_string(),
        AnyValue {
            v: Some(crate::proto::iothub::any_value::V::AnyV(prost_types::Any {
                type_url: String::new(),
                value: req_bytes,
            })),
        },
    );

    let event = EventRecord {
        src: device.to_string(),
        context,
        ..Default::default()
    };

    let list = EventRecordList {
        event_array: vec![event],
    };

    let mut payload = Vec::with_capacity(3 + list.encoded_len());
    payload.extend_from_slice(&[0x20, 0x02, 0x00]);
    // Encoding into a Vec is infallible (prost only errors on insufficient buffer length).
    let _ = list.encode(&mut payload);
    payload
}

fn embedded_iothub_message_bytes(value: &AnyValue) -> Option<&[u8]> {
    match value.v.as_ref() {
        Some(crate::proto::iothub::any_value::V::AnyV(any)) => Some(any.value.as_slice()),
        Some(crate::proto::iothub::any_value::V::BytesV(bytes)) => Some(bytes.as_slice()),
        _ => None,
    }
}

pub fn parse_service_response_rows(payload: &[u8], uid: &AtomicU64) -> Vec<ServiceResponseRow> {
    let Some((summary, list)) = decode_framed_iothub_message::<EventRecordList>(payload) else {
        tracing::warn!(
            payload_len = payload.len(),
            "failed to decode service response EventRecordList"
        );
        return Vec::new();
    };

    let receive_time = Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string();
    let mut rows = Vec::new();

    for event in list.event_array {
        let Some(bytes) = event
            .context
            .get(SVR_RESP_KEY)
            .and_then(embedded_iothub_message_bytes)
        else {
            tracing::debug!(
                event_uuid = %event.evt_uuid,
                src = %event.src,
                "service response event has no svrResp Any payload"
            );
            continue;
        };

        let Ok(svr_resp) = SvrRespRecord::decode(bytes) else {
            tracing::warn!(
                event_uuid = %event.evt_uuid,
                bytes_len = bytes.len(),
                "failed to decode SvrRespRecord from service response"
            );
            continue;
        };

        rows.push(ServiceResponseRow {
            uid: uid.fetch_add(1, Ordering::Relaxed),
            request_uuid: svr_resp.req_serial_uuid,
            response_uuid: event.evt_uuid,
            response_time: format_clock_time(svr_resp.resp_date_time.as_ref()),
            response_code_hex: format_response_code_hex(svr_resp.resp_code),
            responser: if svr_resp.responser.is_empty() {
                svr_resp.requester
            } else {
                svr_resp.responser
            },
            receive_time: receive_time.clone(),
            summary: summary.clone(),
        });
    }

    rows
}

pub fn format_response_code_hex(code: u32) -> String {
    format!("{code:08X}")
}

/// Build a `ClockTime` from the current local time (seconds precision).
pub fn now_clock_time() -> crate::proto::iothub::ClockTime {
    let now = chrono::Local::now();
    crate::proto::iothub::ClockTime {
        t: now.timestamp().clamp(0, u32::MAX as i64) as u32,
        zone_info: 0,
    }
}

/// Convert a `serde_json::Value` to an `AnyValue` following the DFC mapping:
/// bool -> boolV, integer -> sint64V, float -> doubleV, string/compound -> jsonV.
pub fn json_value_to_any_value(value: &serde_json::Value) -> AnyValue {
    use crate::proto::iothub::any_value::V;

    let v = match value {
        serde_json::Value::Bool(b) => V::BoolV(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                V::Sint64V(i)
            } else if let Some(f) = n.as_f64() {
                V::DoubleV(f)
            } else {
                V::StringV(n.to_string())
            }
        }
        serde_json::Value::String(s) => V::JsonV(s.clone()),
        serde_json::Value::Null => V::JsonV("null".to_string()),
        other => V::JsonV(other.to_string()),
    };

    AnyValue { v: Some(v) }
}

pub async fn run_service_topic_stream(
    service_url: String,
    request_topic: String,
    response_topic: String,
    token: Option<String>,
    mut stop: watch::Receiver<bool>,
    publish_rx: Receiver<ServicePublishRequest>,
    tx: Sender<ServiceStreamEvent>,
    uid: Arc<AtomicU64>,
) {
    let service_urls = pulsar_service_url_candidates(&service_url);
    if service_urls.is_empty() {
        let _ = tx.send(ServiceStreamEvent::Error(format!(
            "Pulsar 连接失败: 无法解析 service URL: {service_url}"
        )));
        return;
    }

    let (client, connected_service_url): (pulsar::Pulsar<_>, String) =
        match build_pulsar_client_with_fallbacks(&service_urls, token.as_deref()).await {
            Ok(client) => client,
            Err(e) => {
                let _ = tx.send(ServiceStreamEvent::Error(format!("Pulsar 连接失败: {e}")));
                return;
            }
        };

    tracing::info!(
        service_url = %connected_service_url,
        request_topic = %request_topic,
        response_topic = %response_topic,
        "connected service topic stream"
    );

    let mut consumer: pulsar::Consumer<Vec<u8>, _> = match client
        .consumer()
        .with_topic(&response_topic)
        .with_subscription(format!("dfc-gui-svc-{}", uuid::Uuid::new_v4()))
        .with_subscription_type(pulsar::SubType::Shared)
        .with_consumer_name(format!("dfc-gui-svc-consumer-{}", uuid::Uuid::new_v4()))
        .with_options(
            pulsar::ConsumerOptions::default()
                .durable(false)
                .with_receiver_queue_size(1000),
        )
        .build()
        .await
    {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(ServiceStreamEvent::Error(format!(
                "创建响应 Consumer 失败: {e}"
            )));
            return;
        }
    };

    let mut producer = match client
        .producer()
        .with_topic(&request_topic)
        .with_name(format!("dfc-gui-svc-producer-{}", uuid::Uuid::new_v4()))
        .build()
        .await
    {
        Ok(p) => p,
        Err(e) => {
            let _ = tx.send(ServiceStreamEvent::Error(format!(
                "创建请求 Producer 失败: {e}"
            )));
            return;
        }
    };

    loop {
        if *stop.borrow() {
            return;
        }

        while let Ok(req) = publish_rx.try_recv() {
            let payload = build_service_request_payload(&req.device, &req.record);
            if let Err(e) = producer.send_non_blocking(payload).await {
                let _ = tx.send(ServiceStreamEvent::Error(format!(
                    "发送请求失败 ({}): {e}",
                    req.device
                )));
                tracing::error!(
                    device = %req.device,
                    req_uuid = %req.record.req_serial_uuid,
                    imr = %req.record.imr,
                    "failed to send service request"
                );
            } else {
                tracing::debug!(
                    device = %req.device,
                    req_uuid = %req.record.req_serial_uuid,
                    imr = %req.record.imr,
                    "queued service request to Pulsar"
                );
            }
        }

        tokio::select! {
            _ = stop.changed() => {
                if *stop.borrow() {
                    return;
                }
            }
            msg = consumer.next() => {
                match msg {
                    Some(Ok(message)) => {
                        let payload = message.payload.data.clone();
                        let _ = consumer.ack(&message).await;
                        let rows = parse_service_response_rows(&payload, &uid);
                        for row in rows {
                            let _ = tx.send(ServiceStreamEvent::Response(row));
                        }
                    }
                    Some(Err(e)) => {
                        let _ = tx.send(ServiceStreamEvent::Error(format!("读取响应失败: {e}")));
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                    None => {
                        let _ = tx.send(ServiceStreamEvent::Error(
                            "响应数据流意外结束".to_string(),
                        ));
                        return;
                    }
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(80)) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::iothub::ClockTime;
    use std::collections::HashMap;

    fn sample_req() -> SvrReqRecord {
        let mut args = HashMap::new();
        args.insert(
            "value".to_string(),
            AnyValue {
                v: Some(crate::proto::iothub::any_value::V::Sint64V(42)),
            },
        );
        SvrReqRecord {
            req_serial_uuid: "uuid-1".to_string(),
            req_date_time: Some(ClockTime {
                t: 1_700_000_000,
                zone_info: 8 * 3600,
            }),
            time_out: 5000,
            requester: "V8Test".to_string(),
            imr: "WindTurbine/SERVICE/WTUR/Start".to_string(),
            args,
            is_test_request: false,
        }
    }

    #[test]
    fn build_payload_has_dfc_framing_and_round_trips() {
        let req = sample_req();
        let payload = build_service_request_payload("dev-1", &req);

        assert_eq!(&payload[..3], &[0x20, 0x02, 0x00]);

        let list = EventRecordList::decode(&payload[3..]).expect("decode list");
        assert_eq!(list.event_array.len(), 1);
        let event = &list.event_array[0];
        assert_eq!(event.src, "dev-1");

        let svr_req_bytes = match event
            .context
            .get("svrReq")
            .and_then(|v| v.v.as_ref())
            .expect("svrReq present")
        {
            crate::proto::iothub::any_value::V::AnyV(any) => any.value.clone(),
            other => panic!("unexpected variant: {other:?}"),
        };

        let decoded = SvrReqRecord::decode(svr_req_bytes.as_slice()).expect("decode req");
        assert_eq!(decoded.req_serial_uuid, "uuid-1");
        assert_eq!(decoded.imr, "WindTurbine/SERVICE/WTUR/Start");
        assert_eq!(decoded.time_out, 5000);
        assert_eq!(decoded.requester, "V8Test");
        assert_eq!(decoded.args.len(), 1);
    }

    #[test]
    fn pulsar_service_url_candidates_expand_multi_broker_list() {
        assert_eq!(
            pulsar_service_url_candidates(
                "pulsar://10.10.4.101:6650,10.10.4.102:6650,10.10.4.103:6650"
            ),
            vec![
                "pulsar://10.10.4.101:6650".to_string(),
                "pulsar://10.10.4.102:6650".to_string(),
                "pulsar://10.10.4.103:6650".to_string(),
            ]
        );
    }

    #[test]
    fn pulsar_service_url_candidates_reject_bus_style_addresses() {
        assert!(pulsar_service_url_candidates("10.10.4.101:15000;10.10.4.102:15000").is_empty());
    }

    #[test]
    fn parse_response_extracts_svr_resp() {
        let resp = SvrRespRecord {
            req_serial_uuid: "uuid-1".to_string(),
            resp_code: 0x80010000,
            resp_date_time: Some(ClockTime {
                t: 1_700_000_010,
                zone_info: 0,
            }),
            requester: "V8Test".to_string(),
            imr: "WindTurbine/SERVICE/WTUR/Start".to_string(),
            args: HashMap::new(),
            responser: "device".to_string(),
        };

        let mut context = HashMap::new();
        context.insert(
            "svrResp".to_string(),
            AnyValue {
                v: Some(crate::proto::iothub::any_value::V::AnyV(prost_types::Any {
                    type_url: String::new(),
                    value: resp.encode_to_vec(),
                })),
            },
        );

        let event = EventRecord {
            evt_uuid: "evt-99".to_string(),
            src: "dev-1".to_string(),
            context,
            ..Default::default()
        };

        let list = EventRecordList {
            event_array: vec![event],
        };
        let mut payload = vec![0x20, 0x02, 0x00];
        list.encode(&mut payload)
            .expect("event record list should encode");

        let uid = AtomicU64::new(1);
        let rows = parse_service_response_rows(&payload, &uid);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].request_uuid, "uuid-1");
        assert_eq!(rows[0].response_uuid, "evt-99");
        assert_eq!(rows[0].response_code_hex, "80010000");
        assert_eq!(rows[0].responser, "device");
    }

    #[test]
    fn parse_response_keeps_legacy_bytes_payload_compatibility() {
        let resp = SvrRespRecord {
            req_serial_uuid: "uuid-legacy".to_string(),
            resp_code: 0x8000_0000,
            resp_date_time: None,
            requester: "V8Test".to_string(),
            imr: "WindTurbine/SERVICE/WTUR/Start".to_string(),
            args: HashMap::new(),
            responser: String::new(),
        };

        let mut context = HashMap::new();
        context.insert(
            "svrResp".to_string(),
            AnyValue {
                v: Some(crate::proto::iothub::any_value::V::BytesV(
                    resp.encode_to_vec(),
                )),
            },
        );

        let event = EventRecord {
            evt_uuid: "evt-legacy".to_string(),
            context,
            ..Default::default()
        };

        let list = EventRecordList {
            event_array: vec![event],
        };
        let mut payload = vec![0x20, 0x02, 0x00];
        list.encode(&mut payload)
            .expect("event record list should encode");

        let uid = AtomicU64::new(1);
        let rows = parse_service_response_rows(&payload, &uid);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].request_uuid, "uuid-legacy");
        assert_eq!(rows[0].response_uuid, "evt-legacy");
        assert_eq!(rows[0].responser, "V8Test");
    }

    #[test]
    fn format_response_code_hex_pads_eight_digits() {
        assert_eq!(format_response_code_hex(0), "00000000");
        assert_eq!(format_response_code_hex(0x80010000), "80010000");
        assert_eq!(format_response_code_hex(0xFF), "000000FF");
    }

    #[test]
    fn json_value_to_any_value_maps_basic_types() {
        use crate::proto::iothub::any_value::V;

        let bool_av = json_value_to_any_value(&serde_json::json!(true));
        assert!(matches!(bool_av.v, Some(V::BoolV(true))));

        let int_av = json_value_to_any_value(&serde_json::json!(42));
        assert!(matches!(int_av.v, Some(V::Sint64V(42))));

        let float_av = json_value_to_any_value(&serde_json::json!(std::f64::consts::PI));
        assert!(matches!(float_av.v, Some(V::DoubleV(_))));

        let str_av = json_value_to_any_value(&serde_json::json!("hi"));
        if let Some(V::JsonV(s)) = str_av.v {
            assert_eq!(s, "hi");
        } else {
            panic!("expected jsonV");
        }

        let obj_av = json_value_to_any_value(&serde_json::json!({"a": 1}));
        assert!(matches!(obj_av.v, Some(V::JsonV(_))));

        let null_av = json_value_to_any_value(&serde_json::Value::Null);
        if let Some(V::JsonV(s)) = null_av.v {
            assert_eq!(s, "null");
        } else {
            panic!("expected jsonV");
        }
    }
}
