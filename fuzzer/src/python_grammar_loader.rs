// Nautilus
// Copyright (C) 2024  Daniel Teuchert, Cornelius Aschermann, Sergej Schumilo

use pyo3::prelude::*;
use pyo3::types::{IntoPyDict, PyAny};
use pyo3::{Bound, Py, PyResult, Python};

use crate::Context;

#[pyclass]
struct PyContext {
    ctx: Context,
}
impl PyContext {
    fn get_context(&self) -> Context {
        self.ctx.clone()
    }
}

#[pymethods]
impl PyContext {
    #[new]
    fn new() -> Self {
        PyContext {
            ctx: Context::new(),
        }
    }

    fn rule(&mut self, nt: &str, format: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Ok(s) = format.extract::<&str>() {
            self.ctx.add_rule(nt, s.as_bytes());
        } else if let Ok(s) = format.extract::<&[u8]>() {
            self.ctx.add_rule(nt, s);
        } else {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "format argument should be string or bytes",
            ));
        }
        Ok(())
    }

    fn script(&mut self, nt: &str, nts: Vec<String>, script: Py<PyAny>) {
        self.ctx.add_script(nt, nts, script);
    }

    fn regex(&mut self, nt: &str, regex: &str) {
        self.ctx.add_regex(nt, regex);
    }
}

fn main_(py: Python<'_>, grammar_path: &str) -> PyResult<Context> {
    let py_ctx = Bound::new(py, PyContext::new())?;
    let locals = [("ctx", py_ctx.clone())].into_py_dict(py)?;
    let code = std::ffi::CString::new(
        std::fs::read_to_string(grammar_path).expect("couldn't read grammar file"),
    )
    .expect("grammar file must not contain zero bytes");
    py.run(&code, None, Some(&locals))?;
    Ok(py_ctx.borrow().get_context())
}

pub fn load_python_grammar(grammar_path: &str) -> Context {
    Python::attach(|py| {
        main_(py, grammar_path)
            .map_err(|e| e.print_and_set_sys_last_vars(py))
            .unwrap()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use grammartec::tree::TreeLike;

    #[test]
    fn loads_and_unparses_script_rule() {
        let path = std::env::temp_dir().join(format!(
            "nautilus-python-grammar-loader-{}.py",
            std::process::id()
        ));
        std::fs::write(
            &path,
            "ctx.rule('VALUE', b'value')\nctx.script('START', ['VALUE'], lambda value: b'<' + value + b'>')\n",
        )
        .unwrap();

        let mut ctx = load_python_grammar(path.to_str().unwrap());
        std::fs::remove_file(path).unwrap();
        ctx.initialize(10);
        let tree = ctx.generate_tree_from_nt(ctx.nt_id("START"), 10);

        assert_eq!(tree.unparse_to_vec(&ctx), b"<value>");
    }
}
