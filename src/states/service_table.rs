//! Service topic state
//!
//! Holds request records (sent via the `thing_service-BZ-REQUEST` topic) and
//! response records (received from `thing_service-BZ-RESPONSE`) plus pagination.

use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct ServiceRequestRow {
    pub uid: u64,
    pub device: String,
    pub imr: String,
    pub request_time: String,
    pub timeout_ms: u32,
    pub is_test: bool,
    pub requester: String,
    pub args_json: String,
    pub uuid: String,
    pub response_time: String,
    pub response_code_hex: String,
    pub responser: String,
    pub summary: String,
}

#[derive(Clone, Debug)]
pub struct ServiceResponseRow {
    pub uid: u64,
    pub request_uuid: String,
    pub response_uuid: String,
    pub response_time: String,
    pub response_code_hex: String,
    pub responser: String,
    pub receive_time: String,
    pub summary: String,
}

#[derive(Clone, Debug, Default)]
pub enum ServiceTableLoadState {
    #[default]
    Idle,
    Loading,
    Ready,
    Error(Arc<str>),
}

#[derive(Clone)]
pub struct ServiceTableState {
    topic_path: Option<String>,
    requests: VecDeque<ServiceRequestRow>,
    responses: VecDeque<ServiceResponseRow>,
    load_state: ServiceTableLoadState,
    req_page_size: usize,
    req_page_index: usize,
    resp_page_size: usize,
    resp_page_index: usize,
    max_rows: usize,
}

impl ServiceTableState {
    pub fn new() -> Self {
        Self {
            topic_path: None,
            requests: VecDeque::new(),
            responses: VecDeque::new(),
            load_state: ServiceTableLoadState::Idle,
            req_page_size: 10,
            req_page_index: 0,
            resp_page_size: 10,
            resp_page_index: 0,
            max_rows: 10_000,
        }
    }

    pub fn topic_path(&self) -> Option<&str> {
        self.topic_path.as_deref()
    }

    pub fn load_state(&self) -> &ServiceTableLoadState {
        &self.load_state
    }

    pub fn requests_len(&self) -> usize {
        self.requests.len()
    }

    pub fn responses_len(&self) -> usize {
        self.responses.len()
    }

    pub fn req_page_size(&self) -> usize {
        self.req_page_size
    }

    pub fn req_page_index(&self) -> usize {
        self.req_page_index
    }

    pub fn resp_page_size(&self) -> usize {
        self.resp_page_size
    }

    pub fn resp_page_index(&self) -> usize {
        self.resp_page_index
    }

    pub fn req_total_pages(&self) -> usize {
        page_count(self.requests.len(), self.req_page_size)
    }

    pub fn resp_total_pages(&self) -> usize {
        page_count(self.responses.len(), self.resp_page_size)
    }

    pub fn set_req_page_size(&mut self, size: usize) {
        self.req_page_size = size.max(1);
        self.req_page_index = 0;
    }

    pub fn set_req_page_index(&mut self, index: usize) {
        self.req_page_index = index.min(self.req_total_pages().saturating_sub(1));
    }

    pub fn set_resp_page_size(&mut self, size: usize) {
        self.resp_page_size = size.max(1);
        self.resp_page_index = 0;
    }

    pub fn set_resp_page_index(&mut self, index: usize) {
        self.resp_page_index = index.min(self.resp_total_pages().saturating_sub(1));
    }

    pub fn reset_for_topic(&mut self, topic_path: Option<String>) {
        self.topic_path = topic_path;
        self.requests.clear();
        self.responses.clear();
        self.req_page_index = 0;
        self.resp_page_index = 0;
        self.load_state = if self.topic_path.is_some() {
            ServiceTableLoadState::Loading
        } else {
            ServiceTableLoadState::Idle
        };
    }

    pub fn set_error(&mut self, message: impl Into<Arc<str>>) {
        self.load_state = ServiceTableLoadState::Error(message.into());
    }

    pub fn mark_ready(&mut self) {
        if self.topic_path.is_some() {
            self.load_state = ServiceTableLoadState::Ready;
        }
    }

    pub fn push_request_front(&mut self, row: ServiceRequestRow) {
        self.requests.push_front(row);
        while self.requests.len() > self.max_rows {
            self.requests.pop_back();
        }
        self.req_page_index = self
            .req_page_index
            .min(self.req_total_pages().saturating_sub(1));
        self.mark_ready();
    }

    pub fn push_response_front(&mut self, row: ServiceResponseRow) {
        self.apply_response_to_request(&row);
        self.responses.push_front(row);
        while self.responses.len() > self.max_rows {
            self.responses.pop_back();
        }
        self.resp_page_index = self
            .resp_page_index
            .min(self.resp_total_pages().saturating_sub(1));
        self.mark_ready();
    }

    fn apply_response_to_request(&mut self, response: &ServiceResponseRow) {
        if response.request_uuid.is_empty() {
            return;
        }
        if let Some(req) = self
            .requests
            .iter_mut()
            .find(|r| r.uuid == response.request_uuid)
        {
            req.response_time = response.response_time.clone();
            req.response_code_hex = response.response_code_hex.clone();
            req.responser = response.responser.clone();
            if !response.summary.is_empty() {
                req.summary = response.summary.clone();
            }
        }
    }

    pub fn clear_records(&mut self) {
        self.requests.clear();
        self.responses.clear();
        self.req_page_index = 0;
        self.resp_page_index = 0;
    }

    pub fn req_page_range(&self) -> (usize, usize) {
        page_range(self.requests.len(), self.req_page_size, self.req_page_index)
    }

    pub fn resp_page_range(&self) -> (usize, usize) {
        page_range(
            self.responses.len(),
            self.resp_page_size,
            self.resp_page_index,
        )
    }

    pub fn req_page_rows_owned(&self) -> Vec<ServiceRequestRow> {
        let (start, end) = self.req_page_range();
        if start == end {
            return Vec::new();
        }
        self.requests
            .iter()
            .skip(start)
            .take(end - start)
            .cloned()
            .collect()
    }

    pub fn resp_page_rows_owned(&self) -> Vec<ServiceResponseRow> {
        let (start, end) = self.resp_page_range();
        if start == end {
            return Vec::new();
        }
        self.responses
            .iter()
            .skip(start)
            .take(end - start)
            .cloned()
            .collect()
    }
}

impl Default for ServiceTableState {
    fn default() -> Self {
        Self::new()
    }
}

fn page_count(total: usize, page_size: usize) -> usize {
    if page_size == 0 {
        return 1;
    }
    ((total + page_size - 1) / page_size).max(1)
}

fn page_range(total: usize, page_size: usize, page_index: usize) -> (usize, usize) {
    if total == 0 || page_size == 0 {
        return (0, 0);
    }
    let start = page_index * page_size;
    if start >= total {
        return (0, 0);
    }
    let end = (start + page_size).min(total);
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request(uuid: &str) -> ServiceRequestRow {
        ServiceRequestRow {
            uid: 1,
            device: "dev-1".to_string(),
            imr: "WindTurbine/SERVICE/WTUR/Start".to_string(),
            request_time: "2026-04-14 10:00:00.000".to_string(),
            timeout_ms: 5000,
            is_test: false,
            requester: "V8Test".to_string(),
            args_json: String::new(),
            uuid: uuid.to_string(),
            response_time: String::new(),
            response_code_hex: String::new(),
            responser: String::new(),
            summary: String::new(),
        }
    }

    fn sample_response(req_uuid: &str) -> ServiceResponseRow {
        ServiceResponseRow {
            uid: 2,
            request_uuid: req_uuid.to_string(),
            response_uuid: "evt-1".to_string(),
            response_time: "2026-04-14 10:00:01.500".to_string(),
            response_code_hex: "0x00000000".to_string(),
            responser: "device".to_string(),
            receive_time: "2026-04-14 10:00:01.520".to_string(),
            summary: "ok".to_string(),
        }
    }

    #[test]
    fn push_response_writes_back_to_matching_request() {
        let mut state = ServiceTableState::new();
        state.reset_for_topic(Some("svc".to_string()));
        state.push_request_front(sample_request("uuid-1"));
        state.push_response_front(sample_response("uuid-1"));

        let req = &state.requests[0];
        assert_eq!(req.response_code_hex, "0x00000000");
        assert_eq!(req.responser, "device");
        assert_eq!(req.summary, "ok");
        assert_eq!(state.responses.len(), 1);
    }

    #[test]
    fn unmatched_response_still_appended() {
        let mut state = ServiceTableState::new();
        state.reset_for_topic(Some("svc".to_string()));
        state.push_request_front(sample_request("uuid-1"));
        state.push_response_front(sample_response("uuid-other"));

        assert!(state.requests[0].response_code_hex.is_empty());
        assert_eq!(state.responses.len(), 1);
    }

    #[test]
    fn clear_records_keeps_topic_and_state() {
        let mut state = ServiceTableState::new();
        state.reset_for_topic(Some("svc".to_string()));
        state.push_request_front(sample_request("uuid-1"));
        state.push_response_front(sample_response("uuid-1"));

        state.clear_records();
        assert_eq!(state.requests.len(), 0);
        assert_eq!(state.responses.len(), 0);
        assert_eq!(state.topic_path(), Some("svc"));
        assert!(matches!(state.load_state, ServiceTableLoadState::Ready));
    }

    #[test]
    fn pagination_caps_index_and_returns_slice() {
        let mut state = ServiceTableState::new();
        state.set_req_page_size(2);
        state.reset_for_topic(Some("svc".to_string()));
        for i in 0..5 {
            state.push_request_front(sample_request(&format!("uuid-{i}")));
        }
        assert_eq!(state.req_total_pages(), 3);
        state.set_req_page_index(99);
        assert_eq!(state.req_page_index(), 2);
    }
}
