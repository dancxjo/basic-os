use crate::Value;
use alloc::string::String;
use core::str::FromStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexDirection {
    Row,
    Column,
}

impl FlexDirection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Row => "row",
            Self::Column => "column",
        }
    }

    pub fn to_value(self) -> Value {
        Value::Text(String::from(self.as_str()))
    }

    pub fn from_value(value: &Value) -> Option<Self> {
        value_as_str(value).and_then(|s| Self::from_str(s).ok())
    }
}

impl Default for FlexDirection {
    fn default() -> Self {
        Self::Column
    }
}

impl FromStr for FlexDirection {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "row" => Ok(Self::Row),
            "column" => Ok(Self::Column),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JustifyContent {
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
}

impl JustifyContent {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
            Self::SpaceBetween => "space-between",
            Self::SpaceAround => "space-around",
        }
    }

    pub fn to_value(self) -> Value {
        Value::Text(String::from(self.as_str()))
    }

    pub fn from_value(value: &Value) -> Option<Self> {
        value_as_str(value).and_then(|s| Self::from_str(s).ok())
    }
}

impl Default for JustifyContent {
    fn default() -> Self {
        Self::Start
    }
}

impl FromStr for JustifyContent {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "start" | "flex-start" => Ok(Self::Start),
            "center" => Ok(Self::Center),
            "end" | "flex-end" => Ok(Self::End),
            "space-between" => Ok(Self::SpaceBetween),
            "space-around" => Ok(Self::SpaceAround),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignItems {
    Start,
    Center,
    End,
    Stretch,
}

impl AlignItems {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
            Self::Stretch => "stretch",
        }
    }

    pub fn to_value(self) -> Value {
        Value::Text(String::from(self.as_str()))
    }

    pub fn from_value(value: &Value) -> Option<Self> {
        value_as_str(value).and_then(|s| Self::from_str(s).ok())
    }
}

impl Default for AlignItems {
    fn default() -> Self {
        Self::Stretch
    }
}

impl FromStr for AlignItems {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "start" | "flex-start" => Ok(Self::Start),
            "center" => Ok(Self::Center),
            "end" | "flex-end" => Ok(Self::End),
            "stretch" => Ok(Self::Stretch),
            _ => Err(()),
        }
    }
}

fn value_as_str(value: &Value) -> Option<&str> {
    match value {
        Value::Text(s) => Some(s.as_str()),
        Value::Bytes(b) => core::str::from_utf8(b).ok(),
        _ => None,
    }
}
