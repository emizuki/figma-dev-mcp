use super::common::{protocol_input_wrapper, protocol_output_wrapper};
use figma_dev_mcp_protocol::domain;

protocol_input_wrapper!(GetReactionsInput, domain::GetReactionsInput);
protocol_output_wrapper!(GetReactionsResult, domain::GetReactionsResult);
protocol_input_wrapper!(GetMotionInput, domain::GetMotionInput);
protocol_output_wrapper!(GetMotionResult, domain::GetMotionResult);
