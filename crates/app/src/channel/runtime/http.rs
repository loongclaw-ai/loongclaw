use crate::config::LoongClawConfig;
use super::super::core::http::ChannelOutboundHttpPolicy;

pub fn outbound_http_policy_from_config(
    config: &LoongClawConfig,
) -> ChannelOutboundHttpPolicy {
    ChannelOutboundHttpPolicy {
        allow_private_hosts: config.outbound_http.allow_private_hosts,
    }
}
