//! Tier 1: mirrors test_matrix_entrypoints.c over a model context (a `Vec`
//! matrix stack and its top index) and a recording log, without FFI or a GPU.
use obs_graphics::axisang::AxisAng;
use obs_graphics::matrix_entrypoints::{self as m, Entry, Host, NOT_IN_CONTEXT, POP_LAST};
use obs_graphics::matrix4::Matrix4;
use obs_graphics::quat::Quat;
use obs_graphics::state_stacks::{Stack, identity};
use obs_graphics::vec3::Vec3;
use obs_graphics::vec4::Vec4;

const LOG_ERROR: i32 = 100;
const LOG_DEBUG: i32 = 400;

/// A context (or none), its matrix stack as `graphics_init` leaves it (one
/// identity matrix), and every log line.
struct Fixture {
    context: bool,
    stack: Vec<Matrix4>,
    cur: usize,
    logs: Vec<(i32, String)>,
    /// How many times a call asked for the context.
    lookups: usize,
}

impl Fixture {
    fn new(context: bool) -> Self {
        Self {
            context,
            stack: vec![identity()],
            cur: 0,
            logs: Vec::new(),
            lookups: 0,
        }
    }

    fn expect_no_context_log(&mut self, entry: Entry) {
        assert_eq!(
            self.logs,
            [(LOG_DEBUG, format!("{}: {NOT_IN_CONTEXT}", entry.name()))]
        );
        self.logs.clear();
    }

    fn top(&mut self) -> Matrix4 {
        m::get(self).expect("a top matrix")
    }
}

impl Host for Fixture {
    type Stack<'a> = Model<'a>;

    fn stack(&mut self) -> Option<(Self::Stack<'_>, &mut usize)> {
        self.lookups += 1;
        self.context
            .then_some((Model(&mut self.stack), &mut self.cur))
    }
    fn debug(&mut self, entry: Entry) {
        self.logs
            .push((LOG_DEBUG, format!("{}: {NOT_IN_CONTEXT}", entry.name())));
    }
    fn pop_error(&mut self) {
        self.logs.push((LOG_ERROR, POP_LAST.to_owned()));
    }
}

/// The model stack, borrowed for one call.
pub struct Model<'a>(&'a mut Vec<Matrix4>);

impl Stack<Matrix4> for Model<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }
    fn get(&self, index: usize) -> Option<Matrix4> {
        Stack::get(&*self.0, index)
    }
    fn set(&mut self, index: usize, value: Matrix4) {
        Stack::set(&mut *self.0, index, value);
    }
    fn push(&mut self, value: Matrix4) {
        Stack::push(&mut *self.0, value);
    }
    fn erase(&mut self, index: usize) {
        Stack::erase(&mut *self.0, index);
    }
    fn pop(&mut self) {
        Stack::pop(&mut *self.0);
    }
}

/// test_matrix(): a matrix with sixteen different values.
fn test_matrix(base: f32) -> Matrix4 {
    let f = |i: u8| base + f32::from(i) * 0.25 - f32::from(i % 3);
    let row = |r: u8| Vec4::new(f(r * 4), f(r * 4 + 1), f(r * 4 + 2), f(r * 4 + 3));
    Matrix4::new(row(0), row(1), row(2), row(3))
}

fn q() -> Quat {
    Quat::new(0.2, -0.4, 0.1, 0.88)
}

fn aa() -> AxisAng {
    AxisAng::new(0.3, 0.5, -0.8, 1.25)
}

fn v() -> Vec3 {
    Vec3::new(1.5, -2.0, 0.75)
}

fn bits(m: &Matrix4) -> [u32; 16] {
    let rows = [m.x, m.y, m.z, m.t];
    let mut out = [0; 16];
    for (i, r) in rows.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&[
            r.x.to_bits(),
            r.y.to_bits(),
            r.z.to_bits(),
            r.w.to_bits(),
        ]);
    }
    out
}

/// No argument is read without a context.
fn unread<T>() -> Option<T> {
    panic!("an argument read without a context")
}

/// Without a context each call does nothing to the stack, reads no
/// argument and logs its own name at debug level; gs_matrix_get gives
/// nothing, so its output stays as it was.
#[test]
fn no_context() {
    let mut f = Fixture::new(false);
    let before = f.stack.clone();

    m::push(&mut f);
    f.expect_no_context_log(Entry::Push);
    m::pop(&mut f);
    f.expect_no_context_log(Entry::Pop);
    m::identity(&mut f);
    f.expect_no_context_log(Entry::Identity);
    m::transpose(&mut f);
    f.expect_no_context_log(Entry::Transpose);
    m::set(&mut f, unread);
    f.expect_no_context_log(Entry::Set);
    assert_eq!(m::get(&mut f), None);
    f.expect_no_context_log(Entry::Get);
    m::mul(&mut f, unread);
    f.expect_no_context_log(Entry::Mul);
    m::rotquat(&mut f, unread);
    f.expect_no_context_log(Entry::RotQuat);
    m::rotaa(&mut f, unread);
    f.expect_no_context_log(Entry::RotAa);
    m::translate(&mut f, unread);
    f.expect_no_context_log(Entry::Translate);
    m::scale(&mut f, unread);
    f.expect_no_context_log(Entry::Scale);
    m::rotaa4f(&mut f, 0.3, 0.5, -0.8, 1.25);
    f.expect_no_context_log(Entry::RotAa4f);
    m::translate3f(&mut f, 1.5, -2.0, 0.75);
    f.expect_no_context_log(Entry::Translate3f);
    m::scale3f(&mut f, 1.5, -2.0, 0.75);
    f.expect_no_context_log(Entry::Scale3f);

    assert_eq!(f.stack, before);
    assert_eq!(f.cur, 0);
    // One context read per call.
    assert_eq!(f.lookups, Entry::ALL.len());
}

/// Push copies the top, set and get work on the new top, and pop goes back
/// to the earlier one, which push and set left alone.
#[test]
fn push_set_get_pop() {
    let a = test_matrix(1.0);
    let b = test_matrix(-2.0);
    let mut f = Fixture::new(true);

    m::set(&mut f, || Some(a));
    assert_eq!(f.top(), a);

    for depth in 1..=4 {
        m::push(&mut f);
        assert_eq!(f.cur, depth);
        assert_eq!(f.stack.len(), depth + 1);
        assert_eq!(f.top(), a);
    }

    m::set(&mut f, || Some(b));
    assert_eq!(f.top(), b);
    assert_eq!(f.stack[3], a);

    m::pop(&mut f);
    assert_eq!(f.cur, 3);
    assert_eq!(f.stack.len(), 4);
    assert_eq!(f.top(), a);

    for _ in 0..3 {
        m::pop(&mut f);
    }
    assert_eq!(f.cur, 0);
    assert_eq!(f.stack.len(), 1);
    assert_eq!(f.top(), a);
    assert!(f.logs.is_empty());
}

/// Pop on the last matrix logs at error level and changes nothing.
#[test]
fn pop_last_matrix() {
    let a = test_matrix(4.0);
    let mut f = Fixture::new(true);
    m::set(&mut f, || Some(a));

    for i in 1..=2 {
        m::pop(&mut f);
        assert_eq!(f.logs.len(), i);
        assert_eq!(f.logs[i - 1], (LOG_ERROR, POP_LAST.to_owned()));
        assert_eq!(f.cur, 0);
        assert_eq!(f.stack.len(), 1);
        assert_eq!(f.top(), a);
    }

    // After a push, one pop works again and logs nothing.
    m::push(&mut f);
    m::pop(&mut f);
    assert_eq!(f.logs.len(), 2);
    assert_eq!(f.cur, 0);
}

/// A NULL argument (`None`) passes the context check, then leaves the
/// stack alone, as gs_matrix_stack_* do.
#[test]
fn null_arguments_leave_the_top() {
    let a = test_matrix(2.5);
    let mut f = Fixture::new(true);
    m::set(&mut f, || Some(a));
    m::set(&mut f, || None);
    m::mul(&mut f, || None);
    m::rotquat(&mut f, || None);
    m::rotaa(&mut f, || None);
    m::translate(&mut f, || None);
    m::scale(&mut f, || None);
    assert_eq!(f.stack, [a]);
    assert!(f.logs.is_empty());
}

#[derive(Clone, Copy, Debug)]
enum Op {
    Identity,
    Transpose,
    Mul,
    RotQuat,
    RotAa,
    Translate,
    Scale,
    RotAa4f,
    Translate3f,
    Scale3f,
}

const OPS: [Op; 10] = [
    Op::Identity,
    Op::Transpose,
    Op::Mul,
    Op::RotQuat,
    Op::RotAa,
    Op::Translate,
    Op::Scale,
    Op::RotAa4f,
    Op::Translate3f,
    Op::Scale3f,
];

/// Each operation changes only the top, as its matrix4 function does, at
/// stack depths 0 to 2. The 3f and 4f forms give the same matrix as their
/// vector forms.
#[test]
fn operations_on_top() {
    let start = test_matrix(2.0);
    let below = test_matrix(7.0);
    let arg = test_matrix(-1.5);
    let mut f = Fixture::new(true);

    for depth in 0..3 {
        for op in OPS {
            while f.cur != 0 {
                m::pop(&mut f);
            }
            m::set(&mut f, || Some(below));
            for _ in 0..depth {
                m::push(&mut f);
            }
            m::set(&mut f, || Some(start));

            let expect = match op {
                Op::Identity => {
                    m::identity(&mut f);
                    identity()
                }
                Op::Transpose => {
                    m::transpose(&mut f);
                    start.transpose()
                }
                Op::Mul => {
                    m::mul(&mut f, || Some(arg));
                    arg * &start
                }
                Op::RotQuat => {
                    m::rotquat(&mut f, || Some(q()));
                    start.rotate_i(q())
                }
                Op::RotAa => {
                    m::rotaa(&mut f, || Some(aa()));
                    start.rotate_aa_i(aa())
                }
                Op::Translate => {
                    m::translate(&mut f, || Some(v()));
                    start.translate3v_i(v())
                }
                Op::Scale => {
                    m::scale(&mut f, || Some(v()));
                    start.scale_i(v())
                }
                Op::RotAa4f => {
                    m::rotaa4f(&mut f, 0.3, 0.5, -0.8, 1.25);
                    start.rotate_aa_i(aa())
                }
                Op::Translate3f => {
                    m::translate3f(&mut f, 1.5, -2.0, 0.75);
                    start.translate3v_i(v())
                }
                Op::Scale3f => {
                    m::scale3f(&mut f, 1.5, -2.0, 0.75);
                    start.scale_i(v())
                }
            };

            assert_eq!(f.cur, depth, "{op:?}");
            assert_eq!(f.stack.len(), depth + 1, "{op:?}");
            let top = f.top();
            assert_eq!(bits(&top), bits(&expect), "{op:?} at depth {depth}");
            assert_eq!(bits(&f.stack[depth]), bits(&expect), "{op:?}");
            for d in 0..depth {
                assert_eq!(f.stack[d], below, "{op:?}");
            }
        }
    }
    assert!(f.logs.is_empty());
}

/// One call on the model context.
type Call<'a> = dyn Fn(&mut Fixture) + 'a;

/// The 4f and 3f forms pass their floats unchanged, NaN payloads and
/// infinities too: the same bits as the vector forms.
#[test]
fn float_forms_keep_every_bit() {
    let odd = [
        f32::from_bits(0x7fc0_1234),
        f32::INFINITY,
        f32::NEG_INFINITY,
        -0.0,
        f32::MIN_POSITIVE / 2.0,
    ];
    for (i, &x) in odd.iter().enumerate() {
        let (y, z, w) = (odd[(i + 1) % 5], odd[(i + 2) % 5], odd[(i + 3) % 5]);
        let start = test_matrix(0.5);
        let cases: [(&Call<'_>, &Call<'_>); 3] = [
            (&|f| m::rotaa4f(f, x, y, z, w), &|f| {
                m::rotaa(f, || Some(AxisAng::new(x, y, z, w)))
            }),
            (&|f| m::translate3f(f, x, y, z), &|f| {
                m::translate(f, || Some(Vec3::new(x, y, z)))
            }),
            (&|f| m::scale3f(f, x, y, z), &|f| {
                m::scale(f, || Some(Vec3::new(x, y, z)))
            }),
        ];
        for (floats, vector) in cases {
            let mut a = Fixture::new(true);
            let mut b = Fixture::new(true);
            m::set(&mut a, || Some(start));
            m::set(&mut b, || Some(start));
            floats(&mut a);
            vector(&mut b);
            assert_eq!(bits(&a.top()), bits(&b.top()));
        }
    }
}
