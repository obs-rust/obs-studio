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
//! The parser is a [`ShaderSource`]: the four collections the writer
//! reads, each item with its `written` flag, in the C's order. Text goes to
//! a [`ShaderOut`] one piece at a time, in the order and with the pieces the
//! C appends (`dstr_cat`, `dstr_cat_dstr`, `dstr_catf`), so a C string the
//! shim appends them to grows as the C's does; [`ShaderString`] is the
//! plain one. A `dstr_cat` piece that is empty or starts with a NUL byte
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
pub fn find_func(src: &ShaderSource<'_>, name: &[u8]) -> Option<usize> {
    src.funcs.iter().position(|f| same_name(name, f.name))
}

/// `ep_getparam`: the first param named `name`.
#[must_use]
pub fn find_param(src: &ShaderSource<'_>, name: &[u8]) -> Option<usize> {
    src.params.iter().position(|p| same_name(name, p.name))
}

/// `ep_getstruct`: the first struct named `name`.
#[must_use]
pub fn find_struct(src: &ShaderSource<'_>, name: &[u8]) -> Option<usize> {
    src.structs.iter().position(|s| same_name(name, s.name))
}

/// `ep_getsampler`: the first sampler named `name`.
#[must_use]
pub fn find_sampler(src: &ShaderSource<'_>, name: &[u8]) -> Option<usize> {
    src.samplers.iter().position(|s| same_name(name, s.name))
}

/// `ep_getfunc_strref`: the first function whose name the token's text is.
#[must_use]
pub fn find_func_token(src: &ShaderSource<'_>, text: &[u8]) -> Option<usize> {
    src.funcs.iter().position(|f| token_names(text, f.name))
}

/// `ep_getparam_strref`: the first param whose name the token's text is.
#[must_use]
pub fn find_param_token(src: &ShaderSource<'_>, text: &[u8]) -> Option<usize> {
    src.params.iter().position(|p| token_names(text, p.name))
}

/// `ep_write_param`: the declaration of `params[index]`, once.
pub fn write_param(src: &mut ShaderSource<'_>, out: &mut impl ShaderOut, index: usize) {
    let param = &mut src.params[index];
    if param.written {
        return;
    }

    if param.is_const {
        out.cat(b"const ");
    } else if param.is_uniform {
        out.push_used(param.name);
        out.cat(b"uniform ");
    }

    out.cat(param.type_name);
    out.cat(b" ");
    out.cat(param.name);

    if param.array_count != 0 {
        // dstr_catf("[%u]") of the int: its bits as unsigned.
        #[allow(clippy::cast_sign_loss)]
        let count = param.array_count as u32;
        out.cat(format!("[{count}]").as_bytes());
    }

    out.cat(b";\n");

    param.written = true;
}

/// `ep_write_func_param_deps`.
fn write_func_param_deps(src: &mut ShaderSource<'_>, out: &mut impl ShaderOut, func: usize) {
    let deps = src.funcs[func].param_deps.clone();
    for name in &deps {
        if let Some(param) = find_param(src, name) {
            write_param(src, out, param);
        }
    }

    if !deps.is_empty() {
        out.cat(b"\n\n");
    }
}

/// `ep_write_sampler`: the `sampler_state` block of `samplers[index]`,
/// once.
pub fn write_sampler(src: &mut ShaderSource<'_>, out: &mut impl ShaderOut, index: usize) {
    let sampler = &mut src.samplers[index];
    if sampler.written {
        return;
    }

    out.cat(b"sampler_state ");
    out.cat(sampler.name);
    out.cat(b" {");

    for (i, value) in sampler.values.iter().enumerate() {
        out.cat(b"\n\t");
        out.cat(sampler.states.get(i).copied().unwrap_or_default());
        out.cat(b" = ");
        out.cat(value);
        out.cat(b";\n");
    }

    out.cat(b"\n};\n");
    sampler.written = true;
}

/// `ep_write_func_sampler_deps`.
fn write_func_sampler_deps(src: &mut ShaderSource<'_>, out: &mut impl ShaderOut, func: usize) {
    let deps = src.funcs[func].sampler_deps.clone();
    for name in &deps {
        if let Some(sampler) = find_sampler(src, name) {
            write_sampler(src, out, sampler);
            out.cat(b"\n");
        }
    }
}

/// `ep_write_var`: `[modifier ]type name[ : MAPPING]`.
pub fn write_var(out: &mut impl ShaderOut, var: &WriterVar<'_>) {
    match var.var_type {
        VarType::InOut => out.cat(b"inout "),
        VarType::Out => out.cat(b"out "),
        VarType::Uniform => out.cat(b"uniform "),
        // The "in" input modifier is implied by default, so leave it blank
        // in that case.
        VarType::In => {}
    }

    out.cat(var.type_name);
    out.cat(b" ");
    out.cat(var.name);

    if let Some(mapping) = var.mapping {
        out.cat(b" : ");
        out.cat(mapping);
    }
}

/// `ep_write_struct`: the declaration of `structs[index]`, once.
pub fn write_struct(src: &mut ShaderSource<'_>, out: &mut impl ShaderOut, index: usize) {
    let st = &mut src.structs[index];
    if st.written {
        return;
    }

    out.cat(b"struct ");
    out.cat(st.name);
    out.cat(b" {");

    for var in &st.vars {
        out.cat(b"\n\t");
        write_var(out, var);
        out.cat(b";");
    }

    out.cat(b"\n};\n");
    st.written = true;
}

/// `ep_write_func_struct_deps`.
fn write_func_struct_deps(src: &mut ShaderSource<'_>, out: &mut impl ShaderOut, func: usize) {
    let deps = src.funcs[func].struct_deps.clone();
    for name in &deps {
        let Some(st) = find_struct(src, name) else {
            continue;
        };

        if !src.structs[st].written {
            write_struct(src, out, st);
            out.cat(b"\n");
            src.structs[st].written = true;
        }
    }
}

/// `ep_write_func_func_deps`.
fn write_func_func_deps(src: &mut ShaderSource<'_>, out: &mut impl ShaderOut, func: usize) {
    let deps = src.funcs[func].func_deps.clone();
    for name in &deps {
        let Some(func_dep) = find_func(src, name) else {
            continue;
        };

        if !src.funcs[func_dep].written {
            write_func(src, out, func_dep);
            out.cat(b"\n\n");
        }
    }
}

/// `ep_write_func`: `funcs[index]` after what it depends on: its params,
/// samplers, structs and functions, each written once.
pub fn write_func(src: &mut ShaderSource<'_>, out: &mut impl ShaderOut, index: usize) {
    src.funcs[index].written = true;

    write_func_param_deps(src, out, index);
    write_func_sampler_deps(src, out, index);
    write_func_struct_deps(src, out, index);
    write_func_func_deps(src, out, index);

    /* ------------------------------------ */

    let func = &src.funcs[index];
    out.cat(func.ret_type);
    out.cat(b" ");
    out.cat(func.name);
    out.cat(b"(");

    for (i, var) in func.param_vars.iter().enumerate() {
        if i != 0 {
            out.cat(b", ");
        }
        write_var(out, var);
    }

    out.cat(b")\n");
    out.cat_dstr(func.contents);
    out.cat(b"\n");
}

/// `ep_write_main_params`: the params of `funcs[index]` that are mapped,
/// on their own or as a struct whose first member is, as `main()`'s.
fn write_main_params(src: &ShaderSource<'_>, out: &mut impl ShaderOut, index: usize) {
    for var in &src.funcs[index].param_vars {
        let mut st = None;
        let mut mapped = var.mapping.is_some();

        if !mapped {
            st = find_struct(src, var.type_name);
            if let Some(st) = st {
                mapped = src.structs[st].mapped();
            }
        }

        if mapped {
            out.cat(var.type_name);
            out.cat(b" ");
            out.cat(var.name);

            if st.is_none() {
                out.cat(b" : ");
                out.cat(var.mapping.unwrap_or_default());
            }
        }
    }
}

/// `ep_write_main`: `main()` over `funcs[index]`, returning the call.
pub fn write_main(src: &ShaderSource<'_>, out: &mut impl ShaderOut, index: usize, call_str: &[u8]) {
    let func = &src.funcs[index];
    out.cat(b"\n");
    out.cat(func.ret_type);
    out.cat(b" main(");

    write_main_params(src, out, index);

    out.cat(b")");
    if let Some(mapping) = func.mapping {
        out.cat(b" : ");
        out.cat(mapping);
    }

    out.cat(b"\n{\n\treturn ");
    out.cat_dstr(call_str);
    out.cat(b"\n}\n");
}

/// `ep_reset_written`: every `written` flag cleared.
pub fn reset_written(src: &mut ShaderSource<'_>) {
    for param in &mut src.params {
        param.written = false;
    }
    for st in &mut src.structs {
        st.written = false;
    }
    for func in &mut src.funcs {
        func.written = false;
    }
    for sampler in &mut src.samplers {
        sampler.written = false;
    }
}

/// `ep_makeshaderstring`: the shader for a pass call. `call` is `None` for
/// a call with no token array (`shader_call->array == NULL`), else its
/// tokens before the end token.
pub fn make_shader_string(
    src: &mut ShaderSource<'_>,
    out: &mut impl ShaderOut,
    call: Option<&[CallToken<'_>]>,
) {
    let Some(call) = call else {
        return;
    };

    let start = call
        .iter()
        .position(|token| !is_whitespace(token.lead))
        .unwrap_or(call.len());
    let Some(func_name) = call.get(start) else {
        return;
    };
    if token_names(func_name.text, b"NULL") {
        return;
    }

    let mut call_str = ShaderString::default();
    for token in &call[start..] {
        if let Some(param) = find_param_token(src, token.text) {
            write_param(src, out, param);
        }

        call_str.cat(token.text);
    }

    if let Some(func) = find_func_token(src, func_name.text) {
        write_func(src, out, func);
        write_main(src, out, func, &call_str.text);
    }

    reset_written(src);
}
