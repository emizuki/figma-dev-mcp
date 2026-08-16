#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ToolName {
    GetComponents,
    GetDesignContext,
    GetDevModeData,
    GetFonts,
    GetMetadata,
    GetMotion,
    GetNodes,
    GetReactions,
    GetScreenshot,
    GetSelection,
    GetStyles,
    GetVariables,
    ListFiles,
    SearchNodes,
}

impl ToolName {
    pub(crate) const ALL: [Self; 14] = [
        Self::GetComponents,
        Self::GetDesignContext,
        Self::GetDevModeData,
        Self::GetFonts,
        Self::GetMetadata,
        Self::GetMotion,
        Self::GetNodes,
        Self::GetReactions,
        Self::GetScreenshot,
        Self::GetSelection,
        Self::GetStyles,
        Self::GetVariables,
        Self::ListFiles,
        Self::SearchNodes,
    ];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::GetComponents => "get_components",
            Self::GetDesignContext => "get_design_context",
            Self::GetDevModeData => "get_dev_mode_data",
            Self::GetFonts => "get_fonts",
            Self::GetMetadata => "get_metadata",
            Self::GetMotion => "get_motion",
            Self::GetNodes => "get_nodes",
            Self::GetReactions => "get_reactions",
            Self::GetScreenshot => "get_screenshot",
            Self::GetSelection => "get_selection",
            Self::GetStyles => "get_styles",
            Self::GetVariables => "get_variables",
            Self::ListFiles => "list_files",
            Self::SearchNodes => "search_nodes",
        }
    }
}

impl TryFrom<&str> for ToolName {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::ALL
            .into_iter()
            .find(|name| name.as_str() == value)
            .ok_or(())
    }
}
