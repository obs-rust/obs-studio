//! Safe core of `libobs/graphics/effect-parser-writer.c.h`: the effect
//! parser's shader writer.
//!
//! For each pass, the compile step calls `ep_makeshaderstring()`
//! ([`make_shader_string`]) on the pass's vertex or pixel call, the tokens
//! of `Name(args)`. It writes the declarations of the params the call's
//! tokens name ([`write_param`]), then, when `Name` is one of the parser's
//! functions, that function ([`write_func`]) with every param, sampler,
//! struct and function it depends on written before it, and a `main()` that
//! returns the call ([`write_main`]). Each uniform param it declares is
//! handed to [`ShaderOut::push_used`] (the C pushes a copy of its name into
//! `used_params`). A call with no token array, or with only whitespace
//! tokens or `NULL` before its end, writes nothing and leaves the `written`
//! flags as they were; every other call clears them all afterwards
//! ([`reset_written`]).
//!
//! The parser is a [`WriterParser`], read one field at a time where the C
//! reads it and under the same condition, with each `written` flag set and
//! cleared where the C sets and clears it, and the call is a
//! [`CallTokens`], read a token at a time: the shim reads and writes the C
//! items in place, so a call that returns early reads nothing of the
//! parser. [`ShaderSource`] is the plain parser, the four collections the
//! writer reads, each item with its `written` flag, in the C's order.
//! Text goes to a [`ShaderOut`] one piece at a time, in the order and with
//! the pieces the C appends (`dstr_cat`, `dstr_cat_dstr`, `dstr_catf`), so
//! a C string the shim appends them to grows as the C's does;
//! [`ShaderString`] is the plain one. A `dstr_cat` piece that is empty or starts with a NUL byte
//! appends nothing, as `dstr_ncat` appends nothing for it; a
//! `dstr_cat_dstr` one (a function's body, the call string) appends all of
//! its bytes unless it is empty.
//!
//! The lookups are the C's: the first item whose name equals the one asked
//! for (`strcmp`), or, for a call token, whose name the token's text equals
//! (`strref_cmp`). [`find_func`] and [`find_param`] are this file's
//! `ep_getfunc` and `ep_getparam`; [`find_struct`], [`find_sampler`],
//! [`find_func_token`] and [`find_param_token`] are
//! `effect-parser-funcs.c`'s `ep_getstruct`, `ep_getsampler`,
//! `ep_getfunc_strref` and `ep_getparam_strref`, which the function parser
//! there keeps in C until its own port. `ep_getannotation` has no caller and
//! is not ported.
//!
//! Characterized, not endorsed, and kept (`test_effect_parser_writer`): a
//! `const` param is declared without its value; a sampler a function names
//! twice adds a blank line each time; a uniform the call names that is
//! already marked written is not handed to `push_used`; `main()` drops a
//! param whose struct's first member is unmapped, though the call still
//! passes it.
//!
//! The C reads its dependency lookups without a NULL check, because the
//! parser records only names it found: a name with no item crashes the C.
//! The port skips it instead (the dependency, its sampler's line break, or
//! the struct), so no parser reaches a difference the C survives.
//!
//! The C's `ep_write_main_params()` also collects the mapped params' names
//! into a string `ep_write_main()` frees unused, and `ep_write_main()`
//! returns a copy of the call string; neither reaches the output, so the
//! port keeps neither.
//!
//! One intentional difference (#3768): `main()` gets `", "` between its
//! mapped params. The C writes each straight after the one before it (the
//! `", "` it adds goes only into that unused name string), so a function
//! with two or more mapped params got a `main()` that does not compile:
//! `float4 main(float4 pos : POSITIONfloat2 uv : TEXCOORD0)`. Every shader
//! with at most one mapped param comes out the same.

/// `enum ep_var_type`: the input modifier of a struct member or a
/// function param. `EP_VAR_NONE` is [`VarType::In`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VarType {
    /// `EP_VAR_IN` (`EP_VAR_NONE`), and any value the C writes no
    /// modifier for.
    #[default]
    In,
    InOut,
    Out,
    Uniform,
}

/// `struct ep_var`: a struct member or a function param.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WriterVar<'a> {
    pub type_name: &'a [u8],
    pub name: &'a [u8],
    /// `NULL` is `None`.
    pub mapping: Option<&'a [u8]>,
    pub var_type: VarType,
}

/// What the writer reads of a `struct ep_param`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WriterParam<'a> {
    pub type_name: &'a [u8],
    pub name: &'a [u8],
    pub is_const: bool,
    pub is_uniform: bool,
    /// Written as `[%u]` when not 0.
    pub array_count: i32,
    pub written: bool,
}

/// What the writer reads of a `struct ep_struct`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WriterStruct<'a> {
    pub name: &'a [u8],
    pub vars: Vec<WriterVar<'a>>,
    pub written: bool,
}

impl WriterStruct<'_> {
    /// `ep_struct_mapped` (`effect-parser.h`): its first member has a
    /// mapping.
    #[must_use]
    pub fn mapped(&self) -> bool {
        self.vars.first().is_some_and(|var| var.mapping.is_some())
    }
}

/// What the writer reads of a `struct ep_sampler`: a state for each value
/// (the C reads `states.array[i]` for each of `values.num`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WriterSampler<'a> {
    pub name: &'a [u8],
    pub states: Vec<&'a [u8]>,
    pub values: Vec<&'a [u8]>,
    pub written: bool,
}

/// What the writer reads of a `struct ep_func`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WriterFunc<'a> {
    pub name: &'a [u8],
    pub ret_type: &'a [u8],
    /// `NULL` is `None`.
    pub mapping: Option<&'a [u8]>,
    /// The body, `{` to `}`, as the function parser kept it: `len` bytes,
    /// written unchanged.
    pub contents: &'a [u8],
    pub param_vars: Vec<WriterVar<'a>>,
    pub func_deps: Vec<&'a [u8]>,
    pub struct_deps: Vec<&'a [u8]>,
    pub param_deps: Vec<&'a [u8]>,
    pub sampler_deps: Vec<&'a [u8]>,
    pub written: bool,
}

/// The parser's `params`, `structs`, `funcs` and `samplers`, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShaderSource<'a> {
    pub params: Vec<WriterParam<'a>>,
    pub structs: Vec<WriterStruct<'a>>,
    pub funcs: Vec<WriterFunc<'a>>,
    pub samplers: Vec<WriterSampler<'a>>,
}

/// A token of a pass call before its end token: its text (`str`, `len`
/// bytes; empty for a `NULL` one) and the byte the C tests for whitespace
/// (`*str.array`, which is the lexer's next byte when `len` is 0).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CallToken<'a> {
    pub text: &'a [u8],
    pub lead: u8,
}

/// Where the writer's text and used param names go.
pub trait ShaderOut {
    /// `dstr_ncat` of a piece onto the shader string: nothing when it is
    /// empty or starts with a NUL byte.
    fn cat(&mut self, piece: &[u8]);
    /// `dstr_cat_dstr` of a string: all of it, whatever its first byte,
    /// unless it is empty.
    fn cat_dstr(&mut self, piece: &[u8]);
    /// `da_push_back(*used_params, &new)` of a `dstr_init_copy` of a
    /// uniform param's name.
    fn push_used(&mut self, name: &[u8]);
}

/// A shader string and its used params, as plain bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShaderString {
    pub text: Vec<u8>,
    pub used: Vec<Vec<u8>>,
}

impl ShaderOut for ShaderString {
    fn cat(&mut self, piece: &[u8]) {
        if piece.first().is_some_and(|&b| b != 0) {
            self.text.extend_from_slice(piece);
        }
    }

    fn cat_dstr(&mut self, piece: &[u8]) {
        self.text.extend_from_slice(piece);
    }

    fn push_used(&mut self, name: &[u8]) {
        self.used.push(name.to_vec());
    }
}

/// Which of a parser's items a var belongs to: a struct's members or a
/// function's params.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarsOf {
    Struct(usize),
    Func(usize),
}

/// One of a function's four dependency lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deps {
    /// `func_deps`.
    Funcs,
    /// `struct_deps`.
    Structs,
    /// `param_deps`.
    Params,
    /// `sampler_deps`.
    Samplers,
}

/// The parser as the writer reads it, one field at a time, where the C
/// reads it: the shim reads and writes the C items in place, so each read
/// and each `written` flag write happens under the C's condition and at its
/// place. Indices are within the counts the trait gives.
pub trait WriterParser {
    /// `ep->params.num`.
    fn param_count(&self) -> usize;
    fn param_name(&self, i: usize) -> &[u8];
    fn param_type(&self, i: usize) -> &[u8];
    fn param_is_const(&self, i: usize) -> bool;
    fn param_is_uniform(&self, i: usize) -> bool;
    fn param_array_count(&self, i: usize) -> i32;
    fn param_written(&self, i: usize) -> bool;
    fn set_param_written(&mut self, i: usize, written: bool);

    /// `ep->structs.num`.
    fn struct_count(&self) -> usize;
    fn struct_name(&self, i: usize) -> &[u8];
    fn struct_written(&self, i: usize) -> bool;
    fn set_struct_written(&mut self, i: usize, written: bool);

    /// `ep->samplers.num`.
    fn sampler_count(&self) -> usize;
    fn sampler_name(&self, i: usize) -> &[u8];
    /// `values.num`.
    fn sampler_value_count(&self, i: usize) -> usize;
    /// `states.array[j]`, empty where the sampler has no state `j` (the C
    /// reads past its states).
    fn sampler_state(&self, i: usize, j: usize) -> &[u8];
    fn sampler_value(&self, i: usize, j: usize) -> &[u8];
    fn sampler_written(&self, i: usize) -> bool;
    fn set_sampler_written(&mut self, i: usize, written: bool);

    /// `ep->funcs.num`.
    fn func_count(&self) -> usize;
    fn func_name(&self, i: usize) -> &[u8];
    fn func_ret_type(&self, i: usize) -> &[u8];
    fn func_mapping(&self, i: usize) -> Option<&[u8]>;
    /// The body, `len` bytes of `contents`.
    fn func_contents(&self, i: usize) -> &[u8];
    /// The `num` of a dependency list.
    fn dep_count(&self, i: usize, deps: Deps) -> usize;
    fn dep(&self, i: usize, deps: Deps, k: usize) -> &[u8];
    fn func_written(&self, i: usize) -> bool;
    fn set_func_written(&mut self, i: usize, written: bool);

    /// The `num` of a struct's `vars` or a function's `param_vars`.
    fn var_count(&self, of: VarsOf) -> usize;
    fn var_type(&self, of: VarsOf, j: usize) -> VarType;
    fn var_type_name(&self, of: VarsOf, j: usize) -> &[u8];
    fn var_name(&self, of: VarsOf, j: usize) -> &[u8];
    fn var_mapping(&self, of: VarsOf, j: usize) -> Option<&[u8]>;
}

impl<'a> ShaderSource<'a> {
    fn vars(&self, of: VarsOf) -> &[WriterVar<'a>] {
        match of {
            VarsOf::Struct(i) => &self.structs[i].vars,
            VarsOf::Func(i) => &self.funcs[i].param_vars,
        }
    }
}

impl WriterParser for ShaderSource<'_> {
    fn param_count(&self) -> usize {
        self.params.len()
    }
    fn param_name(&self, i: usize) -> &[u8] {
        self.params[i].name
    }
    fn param_type(&self, i: usize) -> &[u8] {
        self.params[i].type_name
    }
    fn param_is_const(&self, i: usize) -> bool {
        self.params[i].is_const
    }
    fn param_is_uniform(&self, i: usize) -> bool {
        self.params[i].is_uniform
    }
    fn param_array_count(&self, i: usize) -> i32 {
        self.params[i].array_count
    }
    fn param_written(&self, i: usize) -> bool {
        self.params[i].written
    }
    fn set_param_written(&mut self, i: usize, written: bool) {
        self.params[i].written = written;
    }

    fn struct_count(&self) -> usize {
        self.structs.len()
    }
    fn struct_name(&self, i: usize) -> &[u8] {
        self.structs[i].name
    }
    fn struct_written(&self, i: usize) -> bool {
        self.structs[i].written
    }
    fn set_struct_written(&mut self, i: usize, written: bool) {
        self.structs[i].written = written;
    }

    fn sampler_count(&self) -> usize {
        self.samplers.len()
    }
    fn sampler_name(&self, i: usize) -> &[u8] {
        self.samplers[i].name
    }
    fn sampler_value_count(&self, i: usize) -> usize {
        self.samplers[i].values.len()
    }
    fn sampler_state(&self, i: usize, j: usize) -> &[u8] {
        self.samplers[i].states.get(j).copied().unwrap_or_default()
    }
    fn sampler_value(&self, i: usize, j: usize) -> &[u8] {
        self.samplers[i].values[j]
    }
    fn sampler_written(&self, i: usize) -> bool {
        self.samplers[i].written
    }
    fn set_sampler_written(&mut self, i: usize, written: bool) {
        self.samplers[i].written = written;
    }

    fn func_count(&self) -> usize {
        self.funcs.len()
    }
    fn func_name(&self, i: usize) -> &[u8] {
        self.funcs[i].name
    }
    fn func_ret_type(&self, i: usize) -> &[u8] {
        self.funcs[i].ret_type
    }
    fn func_mapping(&self, i: usize) -> Option<&[u8]> {
        self.funcs[i].mapping
    }
    fn func_contents(&self, i: usize) -> &[u8] {
        self.funcs[i].contents
    }
    fn dep_count(&self, i: usize, deps: Deps) -> usize {
        self.dep_list(i, deps).len()
    }
    fn dep(&self, i: usize, deps: Deps, k: usize) -> &[u8] {
        self.dep_list(i, deps)[k]
    }
    fn func_written(&self, i: usize) -> bool {
        self.funcs[i].written
    }
    fn set_func_written(&mut self, i: usize, written: bool) {
        self.funcs[i].written = written;
    }

    fn var_count(&self, of: VarsOf) -> usize {
        self.vars(of).len()
    }
    fn var_type(&self, of: VarsOf, j: usize) -> VarType {
        self.vars(of)[j].var_type
    }
    fn var_type_name(&self, of: VarsOf, j: usize) -> &[u8] {
        self.vars(of)[j].type_name
    }
    fn var_name(&self, of: VarsOf, j: usize) -> &[u8] {
        self.vars(of)[j].name
    }
    fn var_mapping(&self, of: VarsOf, j: usize) -> Option<&[u8]> {
        self.vars(of)[j].mapping
    }
}

impl<'a> ShaderSource<'a> {
    fn dep_list(&self, i: usize, deps: Deps) -> &[&'a [u8]] {
        let f = &self.funcs[i];
        match deps {
            Deps::Funcs => &f.func_deps,
            Deps::Structs => &f.struct_deps,
            Deps::Params => &f.param_deps,
            Deps::Samplers => &f.sampler_deps,
        }
    }
}

/// A pass call's token array, read a token at a time where the C reads it.
/// `k` counts from the first token; the C walks on until the end token.
pub trait CallTokens {
    /// `token->type == CFTOKEN_NONE`: token `k` is the end token.
    fn is_end(&self, k: usize) -> bool;
    /// `*token->str.array`, the byte the C tests for whitespace.
    fn lead(&self, k: usize) -> u8;
    /// `token->str`, `len` bytes.
    fn text(&self, k: usize) -> &[u8];
}

/// The tokens before the end token.
impl CallTokens for [CallToken<'_>] {
    fn is_end(&self, k: usize) -> bool {
        k >= self.len()
    }
    fn lead(&self, k: usize) -> u8 {
        self[k].lead
    }
    fn text(&self, k: usize) -> &[u8] {
        self[k].text
    }
}

/// `strcmp(a, b) == 0` of two C strings.
fn same_name(a: &[u8], b: &[u8]) -> bool {
    a == b
}

/// `strref_cmp(token, name) == 0`, with `name` a C string (no NUL).
fn token_names(text: &[u8], name: &[u8]) -> bool {
    // strref_is_empty: no bytes, or a NUL first.
    if text.first().is_none_or(|&b| b == 0) {
        return name.is_empty();
    }
    let mut i = 0;
    loop {
        let c1 = text.get(i).copied().unwrap_or(0);
        let c2 = name.get(i).copied().unwrap_or(0);
        if c1 != c2 {
            return false;
        }
        // `i++ < str1->len && *str2++`
        if !(i < text.len() && i < name.len()) {
            return true;
        }
        i += 1;
    }
}

/// `is_whitespace` (`util/lexer.h`).
fn is_whitespace(c: u8) -> bool {
    matches!(c, b' ' | b'\r' | b'\t' | b'\n')
}

/// `ep_getfunc`: the first function named `name`.
#[must_use]
pub fn find_func<P: WriterParser + ?Sized>(src: &P, name: &[u8]) -> Option<usize> {
    (0..src.func_count()).find(|&i| same_name(name, src.func_name(i)))
}

/// `ep_getparam`: the first param named `name`.
#[must_use]
pub fn find_param<P: WriterParser + ?Sized>(src: &P, name: &[u8]) -> Option<usize> {
    (0..src.param_count()).find(|&i| same_name(name, src.param_name(i)))
}

/// `ep_getstruct`: the first struct named `name`.
#[must_use]
pub fn find_struct<P: WriterParser + ?Sized>(src: &P, name: &[u8]) -> Option<usize> {
    (0..src.struct_count()).find(|&i| same_name(name, src.struct_name(i)))
}

/// `ep_getsampler`: the first sampler named `name`.
#[must_use]
pub fn find_sampler<P: WriterParser + ?Sized>(src: &P, name: &[u8]) -> Option<usize> {
    (0..src.sampler_count()).find(|&i| same_name(name, src.sampler_name(i)))
}

/// `ep_getfunc_strref`: the first function whose name the token's text is.
#[must_use]
pub fn find_func_token<P: WriterParser + ?Sized>(src: &P, text: &[u8]) -> Option<usize> {
    (0..src.func_count()).find(|&i| token_names(text, src.func_name(i)))
}

/// `ep_getparam_strref`: the first param whose name the token's text is.
#[must_use]
pub fn find_param_token<P: WriterParser + ?Sized>(src: &P, text: &[u8]) -> Option<usize> {
    (0..src.param_count()).find(|&i| token_names(text, src.param_name(i)))
}

/// `ep_struct_mapped` (`effect-parser.h`): the struct's first member has a
/// mapping.
#[must_use]
pub fn struct_mapped<P: WriterParser + ?Sized>(src: &P, st: usize) -> bool {
    let of = VarsOf::Struct(st);
    if src.var_count(of) > 0 {
        return src.var_mapping(of, 0).is_some();
    }
    false
}

/// `ep_write_param`: the declaration of param `i`, once.
pub fn write_param<P: WriterParser + ?Sized>(src: &mut P, out: &mut impl ShaderOut, i: usize) {
    if src.param_written(i) {
        return;
    }

    if src.param_is_const(i) {
        out.cat(b"const ");
    } else if src.param_is_uniform(i) {
        out.push_used(src.param_name(i));
        out.cat(b"uniform ");
    }

    out.cat(src.param_type(i));
    out.cat(b" ");
    out.cat(src.param_name(i));

    if src.param_array_count(i) != 0 {
        // dstr_catf("[%u]") of the int: its bits as unsigned.
        #[allow(clippy::cast_sign_loss)]
        let count = src.param_array_count(i) as u32;
        out.cat(format!("[{count}]").as_bytes());
    }

    out.cat(b";\n");

    src.set_param_written(i, true);
}

/// `ep_write_func_param_deps`.
fn write_func_param_deps<P: WriterParser + ?Sized>(
    src: &mut P,
    out: &mut impl ShaderOut,
    func: usize,
) {
    let mut k = 0;
    while k < src.dep_count(func, Deps::Params) {
        if let Some(param) = find_param(src, src.dep(func, Deps::Params, k)) {
            write_param(src, out, param);
        }
        k += 1;
    }

    if src.dep_count(func, Deps::Params) != 0 {
        out.cat(b"\n\n");
    }
}

/// `ep_write_sampler`: the `sampler_state` block of sampler `i`, once.
pub fn write_sampler<P: WriterParser + ?Sized>(src: &mut P, out: &mut impl ShaderOut, i: usize) {
    if src.sampler_written(i) {
        return;
    }

    out.cat(b"sampler_state ");
    out.cat(src.sampler_name(i));
    out.cat(b" {");

    let mut j = 0;
    while j < src.sampler_value_count(i) {
        out.cat(b"\n\t");
        out.cat(src.sampler_state(i, j));
        out.cat(b" = ");
        out.cat(src.sampler_value(i, j));
        out.cat(b";\n");
        j += 1;
    }

    out.cat(b"\n};\n");
    src.set_sampler_written(i, true);
}

/// `ep_write_func_sampler_deps`.
fn write_func_sampler_deps<P: WriterParser + ?Sized>(
    src: &mut P,
    out: &mut impl ShaderOut,
    func: usize,
) {
    let mut k = 0;
    while k < src.dep_count(func, Deps::Samplers) {
        if let Some(sampler) = find_sampler(src, src.dep(func, Deps::Samplers, k)) {
            write_sampler(src, out, sampler);
            out.cat(b"\n");
        }
        k += 1;
    }
}

/// `ep_write_var`: `[modifier ]type name[ : MAPPING]`.
pub fn write_var<P: WriterParser + ?Sized>(
    src: &P,
    out: &mut impl ShaderOut,
    of: VarsOf,
    j: usize,
) {
    match src.var_type(of, j) {
        VarType::InOut => out.cat(b"inout "),
        VarType::Out => out.cat(b"out "),
        VarType::Uniform => out.cat(b"uniform "),
        // The "in" input modifier is implied by default, so leave it blank
        // in that case.
        VarType::In => {}
    }

    out.cat(src.var_type_name(of, j));
    out.cat(b" ");
    out.cat(src.var_name(of, j));

    if let Some(mapping) = src.var_mapping(of, j) {
        out.cat(b" : ");
        out.cat(mapping);
    }
}

/// `ep_write_struct`: the declaration of struct `i`, once.
pub fn write_struct<P: WriterParser + ?Sized>(src: &mut P, out: &mut impl ShaderOut, i: usize) {
    if src.struct_written(i) {
        return;
    }

    out.cat(b"struct ");
    out.cat(src.struct_name(i));
    out.cat(b" {");

    let of = VarsOf::Struct(i);
    let mut j = 0;
    while j < src.var_count(of) {
        out.cat(b"\n\t");
        write_var(src, out, of, j);
        out.cat(b";");
        j += 1;
    }

    out.cat(b"\n};\n");
    src.set_struct_written(i, true);
}

/// `ep_write_func_struct_deps`.
fn write_func_struct_deps<P: WriterParser + ?Sized>(
    src: &mut P,
    out: &mut impl ShaderOut,
    func: usize,
) {
    let mut k = 0;
    while k < src.dep_count(func, Deps::Structs) {
        if let Some(st) = find_struct(src, src.dep(func, Deps::Structs, k))
            && !src.struct_written(st)
        {
            write_struct(src, out, st);
            out.cat(b"\n");
            src.set_struct_written(st, true);
        }
        k += 1;
    }
}

/// `ep_write_func_func_deps`.
fn write_func_func_deps<P: WriterParser + ?Sized>(
    src: &mut P,
    out: &mut impl ShaderOut,
    func: usize,
) {
    let mut k = 0;
    while k < src.dep_count(func, Deps::Funcs) {
        if let Some(func_dep) = find_func(src, src.dep(func, Deps::Funcs, k))
            && !src.func_written(func_dep)
        {
            write_func(src, out, func_dep);
            out.cat(b"\n\n");
        }
        k += 1;
    }
}

/// `ep_write_func`: function `i` after what it depends on: its params,
/// samplers, structs and functions, each written once.
pub fn write_func<P: WriterParser + ?Sized>(src: &mut P, out: &mut impl ShaderOut, i: usize) {
    src.set_func_written(i, true);

    write_func_param_deps(src, out, i);
    write_func_sampler_deps(src, out, i);
    write_func_struct_deps(src, out, i);
    write_func_func_deps(src, out, i);

    /* ------------------------------------ */

    out.cat(src.func_ret_type(i));
    out.cat(b" ");
    out.cat(src.func_name(i));
    out.cat(b"(");

    let of = VarsOf::Func(i);
    let mut j = 0;
    while j < src.var_count(of) {
        if j != 0 {
            out.cat(b", ");
        }
        write_var(src, out, of, j);
        j += 1;
    }

    out.cat(b")\n");
    out.cat_dstr(src.func_contents(i));
    out.cat(b"\n");
}

/// `ep_write_main_params`: the params of function `i` that are mapped, on
/// their own or as a struct whose first member is, as `main()`'s, with
/// `", "` between them. The C writes them with nothing between them
/// (#3768).
fn write_main_params<P: WriterParser + ?Sized>(src: &P, out: &mut impl ShaderOut, i: usize) {
    let of = VarsOf::Func(i);
    let mut first = true;
    let mut j = 0;
    while j < src.var_count(of) {
        let mut st = None;
        let mut mapped = src.var_mapping(of, j).is_some();

        if !mapped {
            st = find_struct(src, src.var_type_name(of, j));
            if let Some(st) = st {
                mapped = struct_mapped(src, st);
            }
        }

        if mapped {
            if !first {
                out.cat(b", ");
            }
            first = false;

            out.cat(src.var_type_name(of, j));
            out.cat(b" ");
            out.cat(src.var_name(of, j));

            if st.is_none() {
                out.cat(b" : ");
                out.cat(src.var_mapping(of, j).unwrap_or_default());
            }
        }
        j += 1;
    }
}

/// `ep_write_main`: `main()` over function `i`, returning the call.
pub fn write_main<P: WriterParser + ?Sized>(
    src: &P,
    out: &mut impl ShaderOut,
    i: usize,
    call_str: &[u8],
) {
    out.cat(b"\n");
    out.cat(src.func_ret_type(i));
    out.cat(b" main(");

    write_main_params(src, out, i);

    out.cat(b")");
    if let Some(mapping) = src.func_mapping(i) {
        out.cat(b" : ");
        out.cat(mapping);
    }

    out.cat(b"\n{\n\treturn ");
    out.cat_dstr(call_str);
    out.cat(b"\n}\n");
}

/// `ep_reset_written`: every `written` flag cleared.
pub fn reset_written<P: WriterParser + ?Sized>(src: &mut P) {
    let mut i = 0;
    while i < src.param_count() {
        src.set_param_written(i, false);
        i += 1;
    }
    i = 0;
    while i < src.struct_count() {
        src.set_struct_written(i, false);
        i += 1;
    }
    i = 0;
    while i < src.func_count() {
        src.set_func_written(i, false);
        i += 1;
    }
    i = 0;
    while i < src.sampler_count() {
        src.set_sampler_written(i, false);
        i += 1;
    }
}

/// `ep_makeshaderstring`: the shader for a pass call. `call` is `None` for
/// a call with no token array (`shader_call->array == NULL`). Nothing of
/// the parser is read before the call's function name is found.
pub fn make_shader_string<P: WriterParser + ?Sized, C: CallTokens + ?Sized>(
    src: &mut P,
    out: &mut impl ShaderOut,
    call: Option<&C>,
) {
    let mut call_str = ShaderString::default();

    let Some(call) = call else {
        return;
    };

    let mut k = 0;
    while !call.is_end(k) && is_whitespace(call.lead(k)) {
        k += 1;
    }

    if call.is_end(k) || token_names(call.text(k), b"NULL") {
        return;
    }

    let func_name = k;

    while !call.is_end(k) {
        if let Some(param) = find_param_token(src, call.text(k)) {
            write_param(src, out, param);
        }

        call_str.cat(call.text(k));
        k += 1;
    }

    if let Some(func) = find_func_token(src, call.text(func_name)) {
        write_func(src, out, func);
        write_main(src, out, func, &call_str.text);
    }

    reset_written(src);
}
