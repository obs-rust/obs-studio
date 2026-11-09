//! Shader write validation and single-call dispatch, independent of backend layout.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteError {
    NoContext,
    NullParameter,
    NullValue,
}

impl WriteError {
    pub fn reason(self) -> &'static str {
        match self {
            Self::NoContext => "called while not in a graphics context",
            Self::NullParameter => "Null 'param' parameter",
            Self::NullValue => "Null 'val' parameter",
        }
    }
}

/// Validate in C's context/parameter/value order, then forward both tokens unchanged.
/// Scalars are always `Some`; a pointer value is `None` exactly when null.
pub fn write<P, V>(
    context: bool,
    parameter: Option<P>,
    value: Option<V>,
    dispatch: impl FnOnce(P, V),
) -> Result<(), WriteError> {
    if !context {
        return Err(WriteError::NoContext);
    }
    let parameter = parameter.ok_or(WriteError::NullParameter)?;
    let value = value.ok_or(WriteError::NullValue)?;
    dispatch(parameter, value);
    Ok(())
}
