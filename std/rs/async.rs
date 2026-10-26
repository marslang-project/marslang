//! `takepkg rs.async;` - the waits std.async is built from: a timer, waiting
//! for the first of several tasks to finish, and waiting on a worker thread.
//! Tasks and the event loop are the interpreter's own (src/runtime.rs);
//! everything std.async does with these is Marslang.

use std::rc::Rc;
use std::time::Duration;

use crate::runtime::{self, Outcome, Task};
use crate::value::*;

/// A length of time in seconds, already checked by std.async to be a
/// nonnegative float; infinity is forever.
fn length(value: &Value, name: &str) -> RResult<Duration> {
    let seconds = match value {
        Value::Float(f) => *f,
        _ => return type_err(format!("rs.async.{name} expects a float")),
    };
    if seconds.is_nan() || seconds < 0.0 { return range_err(format!("rs.async.{name} needs a nonnegative number of seconds")); }
    Ok(Duration::try_from_secs_f64(seconds).unwrap_or(Duration::MAX))
}

fn task(value: &Value, name: &str) -> RResult<Rc<Task>> {
    match value {
        Value::Task(task) => Ok(task.clone()),
        other => type_err(format!("rs.async.{name} expects a task, got {}", other.type_name())),
    }
}

fn tasks(value: &Value, name: &str) -> RResult<Vec<Rc<Task>>> {
    let Value::Array(array) = value else { return type_err(format!("rs.async.{name} expects an array of tasks")) };
    array.items.borrow().iter().map(|item| task(item, name)).collect()
}

pub fn package() -> Value {
    NativePackage::new("rs.async")
        // Wait seconds: on a timer inside a task, so only the task waits.
        .function("sleep", 1, |args| {
            runtime::sleep(length(&args[0], "sleep")?)?;
            Ok(Value::Null)
        })
        // The position of the first of the tasks to finish, waiting for one.
        .function("wait_any", 1, |args| {
            let tasks = tasks(&args[0], "wait_any")?;
            Ok(Value::Int(runtime::wait_any(&tasks)? as i64))
        })
        // Whether a task finished by raising an error, cancellation included.
        .function("failed", 1, |args| {
            Ok(Value::Bool(matches!(task(&args[0], "failed")?.outcome(), Some(Outcome::Error(..)))))
        })
        // Count a task's error as dealt with, as all() and race() do for the
        // tasks whose errors they leave out.
        .function("observe", 1, |args| {
            task(&args[0], "observe")?.observed.set(true);
            Ok(Value::Null)
        })
        .function("name", 1, |args| Ok(Value::str(&task(&args[0], "name")?.name)))
        // Wait seconds on a worker thread, as native code waits for slow
        // work: inside a task only that task waits.
        .function("work", 1, |args| {
            let length = length(&args[0], "work")?;
            runtime::block_on_worker(move || std::thread::sleep(length))?;
            Ok(Value::Null)
        })
        .build()
}
