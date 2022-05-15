use crate::{
    application::command::{CommandOptionType, Number},
    id::{
        marker::{AttachmentMarker, ChannelMarker, GenericMarker, RoleMarker, UserMarker},
        Id,
    },
};
use serde::{
    de::{Error as DeError, IgnoredAny, MapAccess, Unexpected, Visitor},
    ser::SerializeStruct,
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::fmt::{Debug, Formatter, Result as FmtResult};

/// Data received when a user fills in a command option.
///
/// See [Discord Docs/Application Command Object].
///
/// [Discord Docs/Application Command Object]: https://discord.com/developers/docs/interactions/application-commands#application-command-object-application-command-interaction-data-option-structure
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandDataOption {
    /// [`true`] if this autocomplete option is currently highlighted.
    pub focused: bool,
    pub name: String,
    pub value: CommandOptionValue,
}

impl Serialize for CommandDataOption {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let subcommand_is_empty = matches!(
            &self.value,
            CommandOptionValue::SubCommand(o)
            | CommandOptionValue::SubCommandGroup(o)
                if o.is_empty()
        );

        let len = 2 + usize::from(!subcommand_is_empty) + usize::from(self.focused);

        let mut state = serializer.serialize_struct("CommandDataOption", len)?;

        if self.focused {
            state.serialize_field("focused", &self.focused)?;
        }

        state.serialize_field("name", &self.name)?;

        state.serialize_field("type", &self.value.kind())?;

        match &self.value {
            CommandOptionValue::Attachment(a) => state.serialize_field("value", a)?,
            CommandOptionValue::Boolean(b) => state.serialize_field("value", b)?,
            CommandOptionValue::Channel(c) => state.serialize_field("value", c)?,
            CommandOptionValue::Integer(i) => state.serialize_field("value", i)?,
            CommandOptionValue::Mentionable(m) => state.serialize_field("value", m)?,
            CommandOptionValue::Number(n) => state.serialize_field("value", n)?,
            CommandOptionValue::Role(r) => state.serialize_field("value", r)?,
            CommandOptionValue::String(s) => state.serialize_field("value", s)?,
            CommandOptionValue::User(u) => state.serialize_field("value", u)?,
            CommandOptionValue::SubCommand(s) | CommandOptionValue::SubCommandGroup(s) => {
                if !subcommand_is_empty {
                    state.serialize_field("options", s)?
                }
            }
        }

        state.end()
    }
}

impl<'de> Deserialize<'de> for CommandDataOption {
    #[allow(clippy::too_many_lines)]
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Debug, Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Fields {
            Name,
            Type,
            Value,
            Options,
            Focused,
        }

        // Id before string such that IDs will always be interpreted
        // as such, this does mean that string inputs that looks like
        // IDs will have to be caught if it is a string.
        #[derive(Debug, Deserialize)]
        #[serde(untagged)]
        enum ValueEnvelope {
            Boolean(bool),
            Integer(i64),
            Number(f64),
            Id(Id<GenericMarker>),
            String(String),
        }

        impl ValueEnvelope {
            fn as_unexpected(&self) -> Unexpected<'_> {
                match self {
                    Self::Boolean(b) => Unexpected::Bool(*b),
                    Self::Integer(i) => Unexpected::Signed(*i),
                    Self::Number(f) => Unexpected::Float(*f),
                    Self::Id(_) => Unexpected::Other("ID"),
                    Self::String(s) => Unexpected::Str(s),
                }
            }
        }

        struct CommandDataOptionVisitor;

        impl<'de> Visitor<'de> for CommandDataOptionVisitor {
            type Value = CommandDataOption;

            fn expecting(&self, formatter: &mut Formatter<'_>) -> FmtResult {
                formatter.write_str("CommandDataOption")
            }

            #[allow(clippy::too_many_lines)]
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut name_opt = None;
                let mut kind_opt = None;
                let mut options = Vec::new();
                let mut value_opt = None;
                let mut focused = None;

                loop {
                    let key = match map.next_key() {
                        Ok(Some(key)) => key,
                        Ok(None) => break,
                        Err(why) => {
                            map.next_value::<IgnoredAny>()?;

                            tracing::trace!("ran into an unknown key: {why:?}");

                            continue;
                        }
                    };

                    match key {
                        Fields::Name => {
                            if name_opt.is_some() {
                                return Err(DeError::duplicate_field("name"));
                            }

                            name_opt = Some(map.next_value()?);
                        }
                        Fields::Type => {
                            if kind_opt.is_some() {
                                return Err(DeError::duplicate_field("type"));
                            }

                            kind_opt = Some(map.next_value()?);
                        }
                        Fields::Value => {
                            if value_opt.is_some() {
                                return Err(DeError::duplicate_field("value"));
                            }

                            value_opt = Some(map.next_value()?);
                        }
                        Fields::Options => {
                            if !options.is_empty() {
                                return Err(DeError::duplicate_field("options"));
                            }

                            options = map.next_value()?;
                        }
                        Fields::Focused => {
                            if focused.is_some() {
                                return Err(DeError::duplicate_field("focused"));
                            }

                            focused = map.next_value()?;
                        }
                    }
                }

                let name = name_opt.ok_or_else(|| DeError::missing_field("name"))?;
                let kind = kind_opt.ok_or_else(|| DeError::missing_field("type"))?;

                let value = match kind {
                    CommandOptionType::Attachment => {
                        let val = value_opt.ok_or_else(|| DeError::missing_field("value"))?;

                        if let ValueEnvelope::Id(id) = val {
                            CommandOptionValue::Attachment(id.cast())
                        } else {
                            return Err(DeError::invalid_type(
                                val.as_unexpected(),
                                &"attachment id",
                            ));
                        }
                    }
                    CommandOptionType::Boolean => {
                        let val = value_opt.ok_or_else(|| DeError::missing_field("value"))?;

                        if let ValueEnvelope::Boolean(b) = val {
                            CommandOptionValue::Boolean(b)
                        } else {
                            return Err(DeError::invalid_type(val.as_unexpected(), &"boolean"));
                        }
                    }
                    CommandOptionType::Channel => {
                        let val = value_opt.ok_or_else(|| DeError::missing_field("value"))?;

                        if let ValueEnvelope::Id(id) = val {
                            CommandOptionValue::Channel(id.cast())
                        } else {
                            return Err(DeError::invalid_type(val.as_unexpected(), &"channel id"));
                        }
                    }
                    CommandOptionType::Integer => {
                        let val = value_opt.ok_or_else(|| DeError::missing_field("value"))?;

                        if let ValueEnvelope::Integer(i) = val {
                            CommandOptionValue::Integer(i)
                        } else {
                            return Err(DeError::invalid_type(val.as_unexpected(), &"integer"));
                        }
                    }
                    CommandOptionType::Mentionable => {
                        let val = value_opt.ok_or_else(|| DeError::missing_field("value"))?;

                        if let ValueEnvelope::Id(id) = val {
                            CommandOptionValue::Mentionable(id)
                        } else {
                            return Err(DeError::invalid_type(
                                val.as_unexpected(),
                                &"mentionable id",
                            ));
                        }
                    }
                    CommandOptionType::Number => {
                        let val = value_opt.ok_or_else(|| DeError::missing_field("value"))?;

                        match val {
                            ValueEnvelope::Integer(i) => {
                                // As json allows sending floating
                                // points without the tailing decimals
                                // it may be interpreted as a integer
                                // but it is safe to cast as there can
                                // not occur any loss.
                                #[allow(clippy::cast_precision_loss)]
                                CommandOptionValue::Number(Number(i as f64))
                            }
                            ValueEnvelope::Number(f) => CommandOptionValue::Number(Number(f)),
                            other => {
                                return Err(DeError::invalid_type(
                                    other.as_unexpected(),
                                    &"number",
                                ));
                            }
                        }
                    }
                    CommandOptionType::Role => {
                        let val = value_opt.ok_or_else(|| DeError::missing_field("value"))?;

                        if let ValueEnvelope::Id(id) = val {
                            CommandOptionValue::Role(id.cast())
                        } else {
                            return Err(DeError::invalid_type(val.as_unexpected(), &"role id"));
                        }
                    }

                    CommandOptionType::String => {
                        let val = value_opt.ok_or_else(|| DeError::missing_field("value"))?;

                        match val {
                            ValueEnvelope::String(s) => CommandOptionValue::String(s),
                            ValueEnvelope::Id(id) => {
                                CommandOptionValue::String(id.get().to_string())
                            }
                            other => {
                                return Err(DeError::invalid_type(
                                    other.as_unexpected(),
                                    &"string",
                                ));
                            }
                        }
                    }
                    CommandOptionType::SubCommand => CommandOptionValue::SubCommand(options),
                    CommandOptionType::SubCommandGroup => {
                        CommandOptionValue::SubCommandGroup(options)
                    }
                    CommandOptionType::User => {
                        let val = value_opt.ok_or_else(|| DeError::missing_field("value"))?;

                        if let ValueEnvelope::Id(id) = val {
                            CommandOptionValue::User(id.cast())
                        } else {
                            return Err(DeError::invalid_type(val.as_unexpected(), &"user id"));
                        }
                    }
                };

                Ok(CommandDataOption {
                    name,
                    value,
                    focused: focused.unwrap_or_default(),
                })
            }
        }

        deserializer.deserialize_map(CommandDataOptionVisitor)
    }
}

/// Value of a [`CommandDataOption`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandOptionValue {
    Attachment(Id<AttachmentMarker>),
    Boolean(bool),
    Channel(Id<ChannelMarker>),
    Integer(i64),
    Mentionable(Id<GenericMarker>),
    Number(Number),
    Role(Id<RoleMarker>),
    String(String),
    SubCommand(Vec<CommandDataOption>),
    SubCommandGroup(Vec<CommandDataOption>),
    User(Id<UserMarker>),
}

impl CommandOptionValue {
    pub const fn kind(&self) -> CommandOptionType {
        match self {
            CommandOptionValue::Attachment(_) => CommandOptionType::Attachment,
            CommandOptionValue::Boolean(_) => CommandOptionType::Boolean,
            CommandOptionValue::Channel(_) => CommandOptionType::Channel,
            CommandOptionValue::Integer(_) => CommandOptionType::Integer,
            CommandOptionValue::Mentionable(_) => CommandOptionType::Mentionable,
            CommandOptionValue::Number(_) => CommandOptionType::Number,
            CommandOptionValue::Role(_) => CommandOptionType::Role,
            CommandOptionValue::String(_) => CommandOptionType::String,
            CommandOptionValue::SubCommand(_) => CommandOptionType::SubCommand,
            CommandOptionValue::SubCommandGroup(_) => CommandOptionType::SubCommandGroup,
            CommandOptionValue::User(_) => CommandOptionType::User,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        application::{
            command::{CommandOptionType, CommandType, Number},
            interaction::application_command::{
                CommandData, CommandDataOption, CommandOptionValue,
            },
        },
        id::Id,
    };
    use serde_test::Token;

    #[test]
    fn no_options() {
        let value = CommandData {
            id: Id::new(1),
            name: "permissions".to_owned(),
            kind: CommandType::ChatInput,
            options: Vec::new(),
            resolved: None,
            target_id: None,
        };
        serde_test::assert_tokens(
            &value,
            &[
                Token::Struct {
                    name: "CommandData",
                    len: 3,
                },
                Token::Str("id"),
                Token::NewtypeStruct { name: "Id" },
                Token::Str("1"),
                Token::Str("name"),
                Token::Str("permissions"),
                Token::Str("type"),
                Token::U8(CommandType::ChatInput as u8),
                Token::StructEnd,
            ],
        )
    }

    #[test]
    fn subcommand_without_option() {
        let value = CommandData {
            id: Id::new(1),
            name: "photo".to_owned(),
            kind: CommandType::ChatInput,
            options: Vec::from([CommandDataOption {
                focused: false,
                name: "cat".to_owned(),
                value: CommandOptionValue::SubCommand(Vec::new()),
            }]),
            resolved: None,
            target_id: None,
        };

        serde_test::assert_tokens(
            &value,
            &[
                Token::Struct {
                    name: "CommandData",
                    len: 4,
                },
                Token::Str("id"),
                Token::NewtypeStruct { name: "Id" },
                Token::Str("1"),
                Token::Str("name"),
                Token::Str("photo"),
                Token::Str("type"),
                Token::U8(CommandType::ChatInput as u8),
                Token::Str("options"),
                Token::Seq { len: Some(1) },
                Token::Struct {
                    name: "CommandDataOption",
                    len: 2,
                },
                Token::Str("name"),
                Token::Str("cat"),
                Token::Str("type"),
                Token::U8(CommandOptionType::SubCommand as u8),
                Token::StructEnd,
                Token::SeqEnd,
                Token::StructEnd,
            ],
        );
    }

    #[test]
    fn numbers() {
        let value = CommandDataOption {
            focused: false,
            name: "opt".to_string(),
            value: CommandOptionValue::Number(Number(5.0)),
        };

        serde_test::assert_de_tokens(
            &value,
            &[
                Token::Struct {
                    name: "CommandDataOption",
                    len: 3,
                },
                Token::Str("name"),
                Token::Str("opt"),
                Token::Str("type"),
                Token::U8(CommandOptionType::Number as u8),
                Token::Str("value"),
                Token::I64(5),
                Token::StructEnd,
            ],
        );
    }
}