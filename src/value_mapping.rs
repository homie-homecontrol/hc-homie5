use crate::value_condition::ValueCondition;
use serde::Deserialize;
use std::ops::Deref;

#[derive(Copy, Clone)]
pub enum MappingResult<FROM, TO>
where
    FROM: PartialEq + PartialOrd + std::fmt::Debug,
    TO: PartialEq + PartialOrd + std::fmt::Debug,
{
    Mapped(TO),
    Unmapped(FROM),
}

impl<FROM, TO> MappingResult<FROM, TO>
where
    FROM: PartialEq + PartialOrd + std::fmt::Debug,
    TO: PartialEq + PartialOrd + std::fmt::Debug,
{
    #[allow(dead_code)]
    pub fn is_unmapped(&self) -> bool {
        match self {
            Self::Mapped(_) => false,
            Self::Unmapped(_) => true,
        }
    }
    pub fn is_mapped(&self) -> bool {
        match self {
            Self::Mapped(_) => true,
            Self::Unmapped(_) => false,
        }
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn into_option(self) -> Option<TO> {
        match self {
            MappingResult::Mapped(v) => Some(v),
            MappingResult::Unmapped(_) => None,
        }
    }

    #[allow(dead_code)]
    pub fn into_option_wrap(self) -> MappingResult<Option<FROM>, Option<TO>> {
        match self {
            MappingResult::Mapped(v) => MappingResult::Mapped(Some(v)),
            MappingResult::Unmapped(v) => MappingResult::Unmapped(Some(v)),
        }
    }

    #[allow(dead_code)]
    pub fn as_ref(&self) -> MappingResult<&FROM, &TO> {
        match *self {
            MappingResult::Mapped(ref v) => MappingResult::Mapped(v),
            MappingResult::Unmapped(ref v) => MappingResult::Unmapped(v),
        }
    }
}

impl<FROM, TO> MappingResult<&FROM, &TO>
where
    FROM: PartialEq + PartialOrd + std::fmt::Debug + Clone,
    TO: PartialEq + PartialOrd + std::fmt::Debug + Clone,
{
    pub fn cloned(self) -> MappingResult<FROM, TO> {
        match self {
            MappingResult::Mapped(v) => MappingResult::Mapped(v.clone()),
            MappingResult::Unmapped(v) => MappingResult::Unmapped(v.clone()),
        }
    }
}

impl<FROM, TO> MappingResult<FROM, TO>
where
    FROM: PartialEq + PartialOrd + std::fmt::Debug,
    TO: PartialEq + PartialOrd + std::fmt::Debug,
    FROM: Into<TO>,
{
    /// Converts `Unmapped(FROM)` into `TO` if `FROM` can be converted into `TO`.
    pub fn unwrap(self) -> TO {
        match self {
            MappingResult::Mapped(v) => v,
            MappingResult::Unmapped(v) => v.into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ValueMapping<FROM, TO>
where
    FROM: PartialEq + PartialOrd + std::fmt::Debug,
    TO: PartialEq + PartialOrd + std::fmt::Debug,
{
    #[serde(default = "default_none")]
    pub from: Option<ValueCondition<FROM>>,
    pub to: TO,
}

// Helper function to provide a default value for `Option` fields
fn default_none<FROM>() -> Option<ValueCondition<FROM>>
where
    FROM: PartialEq + PartialOrd + std::fmt::Debug,
{
    None
}

impl<FROM, TO> ValueMapping<FROM, TO>
where
    FROM: PartialEq + PartialOrd + std::fmt::Debug,
    TO: PartialEq + PartialOrd + std::fmt::Debug,
{
    pub fn map_to<'a>(&'a self, value: &'a FROM) -> MappingResult<&'a FROM, &'a TO> {
        if self.from.is_none() {
            return MappingResult::Mapped(&self.to);
        }
        if let Some(true) = self.from.as_ref().map(|cond| cond.evaluate(value)) {
            return MappingResult::Mapped(&self.to);
        }
        MappingResult::Unmapped(value)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ValueMappingList<FROM, TO>(pub Vec<ValueMapping<FROM, TO>>)
where
    FROM: PartialEq + PartialOrd + std::fmt::Debug,
    TO: PartialEq + PartialOrd + std::fmt::Debug;

impl<FROM, TO> ValueMappingList<FROM, TO>
where
    FROM: PartialEq + PartialOrd + std::fmt::Debug,
    TO: PartialEq + PartialOrd + std::fmt::Debug,
{
    pub fn map_to<'a>(&'a self, value: &'a FROM) -> MappingResult<&'a FROM, &'a TO> {
        self.0
            .iter()
            .map(|mapping| mapping.map_to(value))
            .find(|to_value| to_value.is_mapped())
            .unwrap_or(MappingResult::Unmapped(value))
    }
}

impl<FROM, TO> Default for ValueMappingList<FROM, TO>
where
    FROM: PartialEq + PartialOrd + std::fmt::Debug,
    TO: PartialEq + PartialOrd + std::fmt::Debug,
{
    fn default() -> Self {
        Self(vec![])
    }
}

impl<FROM, TO> Deref for ValueMappingList<FROM, TO>
where
    FROM: PartialEq + PartialOrd + std::fmt::Debug,
    TO: PartialEq + PartialOrd + std::fmt::Debug,
{
    type Target = Vec<ValueMapping<FROM, TO>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct ValueMappingIO<IN, OUT>
where
    IN: PartialEq + PartialOrd + std::fmt::Debug,
    OUT: PartialEq + PartialOrd + std::fmt::Debug,
{
    #[serde(default)]
    pub input: ValueMappingList<OUT, IN>,
    #[serde(default)]
    pub output: ValueMappingList<IN, OUT>,
}

#[allow(dead_code)]
impl<IN, OUT> ValueMappingIO<IN, OUT>
where
    IN: PartialEq + PartialOrd + std::fmt::Debug,
    OUT: PartialEq + PartialOrd + std::fmt::Debug,
{
    pub fn map_input<'a>(&'a self, value: &'a OUT) -> MappingResult<&'a OUT, &'a IN> {
        self.input.map_to(value)
    }

    pub fn map_ouput<'a>(&'a self, value: &'a IN) -> MappingResult<&'a IN, &'a OUT> {
        self.output.map_to(value)
    }
}
