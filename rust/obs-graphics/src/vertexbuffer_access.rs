//! Context-first validation and dispatch for opaque vertex buffers.

/// Validation failures preserve the C wrapper's diagnostic order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    NoContext,
    NullBuffer,
}

/// A live context's device operations. Handles and results stay opaque.
pub trait Backend<P, R> {
    fn flush(&mut self, buffer: P);
    fn get_data(&mut self, buffer: P) -> R;
}

/// Flush exactly once after validating context and buffer, in that order.
pub fn flush<P, R, B: Backend<P, R>>(
    context: Option<&mut B>,
    buffer: Option<P>,
) -> Result<(), Error> {
    let context = context.ok_or(Error::NoContext)?;
    let buffer = buffer.ok_or(Error::NullBuffer)?;
    context.flush(buffer);
    Ok(())
}

/// Return the backend's result unchanged, including its empty-result value.
pub fn get_data<P, R, B: Backend<P, R>>(
    context: Option<&mut B>,
    buffer: Option<P>,
) -> Result<R, Error> {
    let context = context.ok_or(Error::NoContext)?;
    let buffer = buffer.ok_or(Error::NullBuffer)?;
    Ok(context.get_data(buffer))
}
