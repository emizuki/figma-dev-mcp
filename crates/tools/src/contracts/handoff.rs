use super::common::{protocol_input_wrapper, protocol_output_wrapper};
use figma_dev_mcp_protocol::domain;

protocol_input_wrapper!(GetStylesInput, domain::GetStylesInput);
protocol_output_wrapper!(GetStylesResult, domain::GetStylesResult);
protocol_input_wrapper!(GetVariablesInput, domain::GetVariablesInput);
protocol_output_wrapper!(GetVariablesResult, domain::GetVariablesResult);
protocol_input_wrapper!(GetComponentsInput, domain::GetComponentsInput);
protocol_output_wrapper!(GetComponentsResult, domain::GetComponentsResult);
protocol_input_wrapper!(GetFontsInput, domain::GetFontsInput);
protocol_output_wrapper!(GetFontsResult, domain::GetFontsResult);
protocol_input_wrapper!(GetDevModeDataInput, domain::GetDevModeDataInput);
protocol_output_wrapper!(GetDevModeDataResult, domain::GetDevModeDataResult);
