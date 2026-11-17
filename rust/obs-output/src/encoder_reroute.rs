//! Create-only encoder replacement. The environment retains plugin ownership.
pub trait Environment {
    type Candidate;
    type Output;
    fn encoder_present(&self) -> bool;
    fn id_present(&self) -> bool;
    fn null_parameter(&mut self, encoder: bool);
    fn allowed(&self) -> bool;
    fn lookup(&self) -> Option<Self::Candidate>;
    fn kinds(&self, candidate: &Self::Candidate) -> (i32, i32);
    fn codecs_equal(&self, candidate: &Self::Candidate) -> bool;
    /// Install the complete active table before invoking its create callback.
    fn install_create(&mut self, candidate: Self::Candidate) -> Self::Output;
}

pub fn create<E: Environment>(env: &mut E) -> Option<E::Output> {
    if !env.encoder_present() {
        env.null_parameter(true);
        return None;
    }
    if !env.id_present() {
        env.null_parameter(false);
        return None;
    }
    if !env.allowed() {
        return None;
    }
    let candidate = env.lookup()?;
    let (original, replacement) = env.kinds(&candidate);
    if original != replacement || !env.codecs_equal(&candidate) {
        return None;
    }
    Some(env.install_create(candidate))
}
