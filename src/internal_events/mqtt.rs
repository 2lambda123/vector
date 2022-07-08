use metrics::counter;
use vector_core::internal_event::InternalEvent;

#[derive(Debug)]
pub struct MqttEventsReceived {
    pub byte_size: usize,
    pub count: usize,
}

impl InternalEvent for MqttEventsReceived {
    fn emit(self) {
        trace!(
            message = "Received events.",
            self.count,
            internal_log_rate_secs = 10
        );

        counter!("component_received_events_total", self.count as u64);
        counter!("events_in_total", self.count as u64);
        counter!("processed_bytes_total", self.byte_size as u64);
    }
}

#[derive(Debug)]
pub struct MqttConnectionError {
    pub error: rumqttc::ConnectionError,
}

impl InternalEvent for MqttConnectionError {
    fn emit(self) {
        error!(message = "Connection error.", error = ?self.error);
    }
}

#[derive(Debug)]
pub struct MqttClientError {
    pub error: rumqttc::ClientError,
}

impl InternalEvent for MqttClientError {
    fn emit(self) {
        error!(message = "Client error.", error = ?self.error);
    }
}
