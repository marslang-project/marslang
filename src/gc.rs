//! Cycle collection for runtime values.
//!
//! Values are reference counted (`Rc`), which frees everything except cycles:
//! an array that contains itself, or an instance whose field holds one of its
//! bound methods. This collector finds unreachable cycles the way CPython's does:
//!
//! 1. Every object that can hold values (arrays, sets, maps, pairs, instances,
//!    bound methods, closures, packages) is registered when it is created, and
//!    a call frame once a closure captures it: a closure stored in the frame it
//!    closes over is a cycle.
//! 2. For each live object, start from its strong count and subtract one for
//!    every reference held by another registered object. Whatever remains is
//!    held from outside the object graph: globals, call frames, or temporaries
//!    on the interpreter's Rust stack. Those objects are roots.
//! 3. Everything reachable from a root is alive. The rest is unreachable, so
//!    its contents are cleared, which breaks the cycles and lets `Rc` free it.
//!
//! No explicit root set is needed, so collection is safe at any point where no
//! container is mutably borrowed; the interpreter collects between statements
//! and once more when a run ends. The heap is per thread, and each run has its
//! own interpreter thread.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use crate::value::*;

/// Collect after this many new objects, or after as many as survived the last
/// collection when that is larger, so collection cost stays proportional to
/// allocation.
const MIN_THRESHOLD: usize = 10_000;

#[derive(Default)]
struct Heap {
    objects: Vec<Tracked>,
    allocated: usize,
    threshold: usize,
}

thread_local! {
    static HEAP: RefCell<Heap> = RefCell::new(Heap { threshold: MIN_THRESHOLD, ..Heap::default() });
    /// Set when a collection is due; checked at every statement, so kept cheap.
    static DUE: Cell<bool> = const { Cell::new(false) };
}

enum Tracked {
    Array(Weak<Array>),
    Set(Weak<Set>),
    Map(Weak<Map>),
    Pair(Weak<Pair>),
    Instance(Weak<Instance>),
    Func(Weak<Callable>),
    Package(Weak<Package>),
    Frame(Weak<Frame>),
}

enum Node {
    Array(Rc<Array>),
    Set(Rc<Set>),
    Map(Rc<Map>),
    Pair(Rc<Pair>),
    Instance(Rc<Instance>),
    Func(Rc<Callable>),
    Package(Rc<Package>),
    Frame(Rc<Frame>),
}

/// Register a newly created value that can hold other values.
pub(crate) fn track(value: &Value) {
    let tracked = match value {
        Value::Array(o) => Tracked::Array(Rc::downgrade(o)),
        Value::Set(o) => Tracked::Set(Rc::downgrade(o)),
        Value::Map(o) => Tracked::Map(Rc::downgrade(o)),
        Value::Pair(o) => Tracked::Pair(Rc::downgrade(o)),
        Value::Instance(o) => Tracked::Instance(Rc::downgrade(o)),
        Value::Func(f) if matches!(f.as_ref(), Callable::Method(..) | Callable::Closure(..)) => Tracked::Func(Rc::downgrade(f)),
        Value::Package(o) => Tracked::Package(Rc::downgrade(o)),
        _ => return,
    };
    register(tracked);
}

/// Register a call frame that a closure captures, and the frames around it,
/// each once. Frames no closure captures are never registered: nothing can
/// point back at them, so reference counting alone frees them.
pub(crate) fn track_frame(frame: &Rc<Frame>) {
    let mut current = Some(frame.clone());
    while let Some(frame) = current {
        if frame.tracked.replace(true) { return; }
        register(Tracked::Frame(Rc::downgrade(&frame)));
        current = frame.parent.borrow().clone();
    }
}

fn register(tracked: Tracked) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        heap.objects.push(tracked);
        heap.allocated += 1;
        if heap.allocated >= heap.threshold { DUE.with(|due| due.set(true)); }
    });
}

/// Whether enough objects were created since the last collection.
pub(crate) fn due() -> bool {
    DUE.with(Cell::get)
}

/// Number of registered objects that are still alive.
#[cfg(test)]
pub(crate) fn live() -> usize {
    HEAP.with(|heap| heap.borrow().objects.iter().filter(|t| t.upgrade().is_some()).count())
}

/// Free unreachable cycles. Returns how many objects were found unreachable.
pub(crate) fn collect() -> usize {
    let tracked = HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        heap.allocated = 0;
        DUE.with(|due| due.set(false));
        std::mem::take(&mut heap.objects)
    });
    let nodes: Vec<Node> = tracked.iter().filter_map(Tracked::upgrade).collect();
    drop(tracked);

    let index: HashMap<usize, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id(), i)).collect();
    // References from outside the graph; `nodes` itself holds one of each count.
    let mut external: Vec<isize> = nodes.iter().map(|n| n.strong_count() as isize - 1).collect();
    let mut edges: Vec<Vec<usize>> = Vec::with_capacity(nodes.len());
    for node in &nodes {
        let mut children = Vec::new();
        let complete = node.visit(&mut |id| {
            if let Some(&j) = index.get(&id) { children.push(j); }
        });
        if !complete {
            // A container is mutably borrowed: not a safe point. Retry later.
            restore(&nodes);
            return 0;
        }
        for &j in &children { external[j] -= 1; }
        edges.push(children);
    }

    let mut alive = vec![false; nodes.len()];
    let mut stack: Vec<usize> = (0..nodes.len()).filter(|&i| external[i] > 0).collect();
    while let Some(i) = stack.pop() {
        if alive[i] { continue; }
        alive[i] = true;
        stack.extend(edges[i].iter().copied().filter(|&j| !alive[j]));
    }

    let (survivors, garbage): (Vec<_>, Vec<_>) = nodes.into_iter().zip(alive).partition(|(_, alive)| *alive);
    let survivors: Vec<Node> = survivors.into_iter().map(|(node, _)| node).collect();
    restore(&survivors);
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        heap.threshold = MIN_THRESHOLD.max(survivors.len());
    });
    drop(survivors);

    // Clearing drops the references that form the cycles; dropping `garbage`
    // then releases the last handles and frees the objects.
    let freed = garbage.len();
    let mut released = Vec::new();
    for (node, _) in &garbage { node.clear(&mut released); }
    drop(released);
    drop(garbage);
    freed
}

fn restore(nodes: &[Node]) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let mut objects: Vec<Tracked> = nodes.iter().map(Node::downgrade).collect();
        // Keep anything registered while collecting.
        objects.append(&mut heap.objects);
        heap.objects = objects;
    });
}

/// Identity of a child value that may be a registered node.
fn child_id(value: &Value) -> Option<usize> {
    match value {
        Value::Func(f) => Some(Rc::as_ptr(f) as *const () as usize),
        other => other.object_id(),
    }
}

impl Tracked {
    fn upgrade(&self) -> Option<Node> {
        Some(match self {
            Tracked::Array(w) => Node::Array(w.upgrade()?),
            Tracked::Set(w) => Node::Set(w.upgrade()?),
            Tracked::Map(w) => Node::Map(w.upgrade()?),
            Tracked::Pair(w) => Node::Pair(w.upgrade()?),
            Tracked::Instance(w) => Node::Instance(w.upgrade()?),
            Tracked::Func(w) => Node::Func(w.upgrade()?),
            Tracked::Package(w) => Node::Package(w.upgrade()?),
            Tracked::Frame(w) => Node::Frame(w.upgrade()?),
        })
    }
}

impl Node {
    fn id(&self) -> usize {
        match self {
            Node::Array(o) => Rc::as_ptr(o) as *const () as usize,
            Node::Set(o) => Rc::as_ptr(o) as *const () as usize,
            Node::Map(o) => Rc::as_ptr(o) as *const () as usize,
            Node::Pair(o) => Rc::as_ptr(o) as *const () as usize,
            Node::Instance(o) => Rc::as_ptr(o) as *const () as usize,
            Node::Func(o) => Rc::as_ptr(o) as *const () as usize,
            Node::Package(o) => Rc::as_ptr(o) as *const () as usize,
            Node::Frame(o) => Rc::as_ptr(o) as *const () as usize,
        }
    }

    fn strong_count(&self) -> usize {
        match self {
            Node::Array(o) => Rc::strong_count(o),
            Node::Set(o) => Rc::strong_count(o),
            Node::Map(o) => Rc::strong_count(o),
            Node::Pair(o) => Rc::strong_count(o),
            Node::Instance(o) => Rc::strong_count(o),
            Node::Func(o) => Rc::strong_count(o),
            Node::Package(o) => Rc::strong_count(o),
            Node::Frame(o) => Rc::strong_count(o),
        }
    }

    fn downgrade(&self) -> Tracked {
        match self {
            Node::Array(o) => Tracked::Array(Rc::downgrade(o)),
            Node::Set(o) => Tracked::Set(Rc::downgrade(o)),
            Node::Map(o) => Tracked::Map(Rc::downgrade(o)),
            Node::Pair(o) => Tracked::Pair(Rc::downgrade(o)),
            Node::Instance(o) => Tracked::Instance(Rc::downgrade(o)),
            Node::Func(o) => Tracked::Func(Rc::downgrade(o)),
            Node::Package(o) => Tracked::Package(Rc::downgrade(o)),
            Node::Frame(o) => Tracked::Frame(Rc::downgrade(o)),
        }
    }

    /// Visit the identity of everything this node holds that may be a node:
    /// the values in it, and for closures and frames the frames they hold.
    /// Returns false if a container is currently mutably borrowed.
    fn visit(&self, child: &mut dyn FnMut(usize)) -> bool {
        fn value(v: &Value, child: &mut dyn FnMut(usize)) { if let Some(id) = child_id(v) { child(id) } }
        fn frame(f: &Rc<Frame>, child: &mut dyn FnMut(usize)) { child(Rc::as_ptr(f) as *const () as usize) }
        match self {
            Node::Array(o) => match o.items.try_borrow() { Ok(items) => items.iter().for_each(|v| value(v, child)), Err(_) => return false },
            Node::Set(o) => match o.items.try_borrow() { Ok(items) => items.values().for_each(|v| value(v, child)), Err(_) => return false },
            Node::Map(o) => match o.items.try_borrow() {
                Ok(items) => items.values().for_each(|(k, v)| { value(k, child); value(v, child); }),
                Err(_) => return false,
            },
            Node::Pair(o) => match (o.first.try_borrow(), o.second.try_borrow()) {
                (Ok(first), Ok(second)) => { value(&first, child); value(&second, child); }
                _ => return false,
            },
            Node::Instance(o) => match o.fields.try_borrow() { Ok(fields) => fields.values().for_each(|v| value(v, child)), Err(_) => return false },
            Node::Func(f) => match f.as_ref() {
                Callable::Method(receiver, _) => value(receiver, child),
                Callable::Closure(_, captured, me) => {
                    if let Some(captured) = captured { frame(captured, child); }
                    if let Some(me) = me { value(me, child); }
                }
                _ => {}
            },
            Node::Package(o) => o.members.values().for_each(|v| value(v, child)),
            Node::Frame(o) => match (o.values.try_borrow(), o.parent.try_borrow()) {
                (Ok(values), Ok(parent)) => {
                    values.values().for_each(|v| value(v, child));
                    if let Some(parent) = parent.as_ref() { frame(parent, child); }
                }
                _ => return false,
            },
        }
        true
    }

    /// Move this node's contents into `released`, breaking its references.
    /// Bound methods and packages are immutable; every cycle through them also
    /// passes through a container or instance, which is cleared.
    fn clear(&self, released: &mut Vec<Value>) {
        match self {
            Node::Array(o) => released.append(&mut o.items.borrow_mut()),
            Node::Set(o) => released.extend(std::mem::take(&mut *o.items.borrow_mut()).into_values()),
            Node::Map(o) => released.extend(std::mem::take(&mut *o.items.borrow_mut()).into_values().flat_map(|(k, v)| [k, v])),
            Node::Pair(o) => {
                released.push(o.first.replace(Value::Null));
                released.push(o.second.replace(Value::Null));
            }
            Node::Instance(o) => released.extend(std::mem::take(&mut *o.fields.borrow_mut()).into_values()),
            // Every other node of the cycle is garbage too and still held, so
            // dropping the parent here cannot free anything mid-collection.
            Node::Frame(o) => {
                released.extend(std::mem::take(&mut *o.values.borrow_mut()).into_values());
                o.parent.borrow_mut().take();
            }
            Node::Func(_) | Node::Package(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push(array: &Value, item: Value) {
        let Value::Array(a) = array else { unreachable!() };
        a.items.borrow_mut().push(item);
    }

    #[test]
    fn unreachable_cycles_are_freed_and_reachable_ones_kept() {
        collect();
        let kept = Value::array(Vec::new());
        push(&kept, kept.clone());
        let child = Value::array(Vec::new());
        push(&child, child.clone());
        push(&kept, child);
        for _ in 0..100 {
            let garbage = Value::array(Vec::new());
            push(&garbage, garbage.clone());
            let pair = Value::pair(garbage.clone(), Value::Null);
            push(&garbage, pair);
        }
        assert_eq!(live(), 2 + 200);
        assert_eq!(collect(), 200);
        assert_eq!(live(), 2);
        // The kept cycle and the cycle it reaches are intact.
        if let Value::Array(a) = &kept {
            assert_eq!(a.items.borrow().len(), 2);
            let Value::Array(c) = &a.items.borrow()[1] else { unreachable!() };
            assert_eq!(c.items.borrow().len(), 1);
        }
        drop(kept);
        collect();
        assert_eq!(live(), 0);
    }

    #[test]
    fn collection_waits_while_a_container_is_borrowed() {
        collect();
        let a = Value::array(Vec::new());
        push(&a, a.clone());
        let Value::Array(inner) = &a else { unreachable!() };
        let guard = inner.items.borrow_mut();
        assert_eq!(collect(), 0);
        drop(guard);
        drop(a);
        assert_eq!(collect(), 1);
    }
}
