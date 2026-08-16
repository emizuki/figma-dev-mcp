macro_rules! protocol_input_wrapper {
    ($name:ident, $inner:path) => {
        #[derive(
            Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
        )]
        #[serde(transparent)]
        pub struct $name($inner);

        impl $name {
            pub(crate) fn into_protocol(self) -> $inner {
                self.0
            }
        }
    };
}

pub(crate) use protocol_input_wrapper;

macro_rules! protocol_schema_wrapper {
    ($name:ident, $inner:path) => {
        #[derive(
            Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
        )]
        #[serde(transparent)]
        pub struct $name($inner);
    };
}

pub(crate) use protocol_schema_wrapper;

macro_rules! protocol_output_wrapper {
    ($name:ident, $inner:path) => {
        #[derive(
            Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
        )]
        #[serde(transparent)]
        pub struct $name($inner);

        impl $name {
            pub(crate) fn from_protocol(value: $inner) -> Self {
                Self(value)
            }
        }
    };
}

pub(crate) use protocol_output_wrapper;

macro_rules! protocol_object_output_wrapper {
    ($name:ident, $inner:path) => {
        #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
        #[serde(transparent)]
        pub struct $name($inner);

        impl $name {
            pub(crate) fn from_protocol(value: $inner) -> Self {
                Self(value)
            }
        }

        impl schemars::JsonSchema for $name {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                stringify!($name).into()
            }

            fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
                let mut schema = <$inner as schemars::JsonSchema>::json_schema(generator);
                schema
                    .ensure_object()
                    .insert("type".to_owned(), "object".into());
                schema
            }
        }
    };
}

pub(crate) use protocol_object_output_wrapper;
