use figma_dev_mcp_broker::PLUGIN_PROTOCOL_VERSION;
use figma_dev_mcp_protocol::domain::{
    CapabilitySet, ConnectionId, DisplayText, PageId, PageSummary, ProtocolVersion,
};
use figma_dev_mcp_protocol::wire::Hello;

pub fn hello(connection_id: &str, file_name: &str) -> Hello {
    Hello {
        // Taken from the broker rather than written out again: a harness that
        // can disagree with the server it drives is the drift this file once had.
        protocol_version: ProtocolVersion::try_from(PLUGIN_PROTOCOL_VERSION).unwrap(),
        connection_id: ConnectionId::try_from(connection_id).unwrap(),
        display_name: DisplayText::try_from(file_name).unwrap(),
        file_key: None,
        file_name: DisplayText::try_from(file_name).unwrap(),
        current_page: PageSummary {
            id: PageId::try_from("0:1").unwrap(),
            name: DisplayText::try_from("Page 1").unwrap(),
        },
        editor_type: DisplayText::try_from("dev").unwrap(),
        plugin_version: DisplayText::try_from("0.1.0").unwrap(),
        capabilities: CapabilitySet::default(),
    }
}
