# Nautilus 2.0

Nautilus is a coverage guided, grammar based fuzzer. You can use it to improve your test coverage and find more bugs. By specifying the grammar of semi valid inputs, Nautilus is able to perform complex mutation and to uncover more interesting test cases. Many of the ideas behind this fuzzer are documented in a Paper published at NDSS 2019.

<p>
<a href="https://www.syssec.ruhr-uni-bochum.de/media/emma/veroeffentlichungen/2018/12/17/NDSS19-Nautilus.pdf"> <img align="right" width="200"  src="https://github.com/RUB-SysSec/nautilus/raw/master/paper.png"> </a>
</p>


Version 2.0 has added many improvements to this early prototype and is now 100% compatible with AFL++. Besides general usability improvements, Version 2.0 includes lots of shiny new features:

* Support for AFL-Qemu mode
* Support for grammars specified in python
* Support for non-context free grammars using python scripts to generate inputs from the structure
* Support for specifying binary protocols/formats
* Support for specifying regex based terminals that aren't part of the directed mutations
* Better ability to avoid generating the same very short inputs over and over
* Massive cleanup of the code base
* Helpful error output on invalid grammars
* Fixed a bug in the the timeout code that occasionally deadlocked the fuzzer


## How Does Nautilus Work?

You specify a grammar using rules such as `EXPR -> EXPR + EXPR` or `EXPR -> NUM` and `NUM -> 1`. From these rules, the fuzzer constructs a tree. This internal representation allows to apply much more complex mutations than raw bytes. This tree is then turned into a real input for the target application. In normal Context Free Grammars, this process is straightforward: all leaves are concatenated. The left tree in the example below would unparse to the input `a=1+2` and the right one to `a=1+1+1+2`. To increase the expressiveness of your grammars, using Nautilus you are able to provide python functions for the unparsing process to allow much more complex specifications. 

<p align="center">
<img width="400" align="center" src="https://github.com/RUB-SysSec/nautilus/raw/master/tree.png">
</p>

## Development toolchain

[mise](https://mise.jdx.dev) installs Python 3.12, Ruff, basedPyright, and the Rust
components declared in `rust-toolchain.toml`: Rust 1.98.1, rustfmt, Clippy,
rust-analyzer, and rust-src. The toolchain file is the sole maintained Rust version
pin; mise discovers it rather than repeating the version in `mise.toml`. Existing
`MISE_CARGO_HOME` and `MISE_RUSTUP_HOME` settings are respected, so mise reuses those
homes instead of creating a competing Rust installation.

```bash
mise trust
mise install
mise run check
```

Focused Python checks remain available as `mise run lint`, `mise run fmt:check`,
`mise run typecheck`, and `mise run syntax`. Focused Rust checks are:

```bash
mise run rust:build   # cargo build --workspace --locked
mise run rust:fmt     # cargo fmt --all -- --check
mise run rust:clippy  # workspace/all-target Clippy with warnings denied
mise run rust:test    # cargo test --workspace --locked
```

`mise run check` runs every Python and Rust check above. The full Rust tests require
an AFL++-instrumented `./test`; the task fails with its preparation command when that
executable is absent instead of reporting a skipped test as successful:

```bash
mise run target:prepare
mise run rust:test
```

For editor discovery, start the editor from an activated mise shell or configure its
Rust language-server command from `mise which rust-analyzer`. Verify the selected
executables and components with:

```bash
mise exec -- rustc --version
mise exec -- cargo --version
mise exec -- rustfmt --version
mise exec -- cargo clippy --version
mise exec -- rust-analyzer --version
mise exec -- rustup component list --installed
```

### Docker development/test image

`Dockerfile` pins a Debian 12 (glibc 2.36) base by digest, the matching Debian
package snapshot, mise 2026.9.3, and AFL++ 4.10c (the latest tagged release using
the legacy forkserver handshake consumed here). mise then installs the locked
Python 3.12 and Rust toolchains. `PYO3_PYTHON` selects that mise Python explicitly;
the image build checks its embedding header and shared library before preparing the
bundled AFL++ target through `mise run target:prepare`.

Build from a clean checkout with the host user's IDs so bind-mounted outputs remain
owned by that user. These commands are deliberately bounded and start without Docker
layer reuse:

```bash
timeout 30m docker build --pull --no-cache \
  --build-arg USER_ID="$(id -u)" --build-arg GROUP_ID="$(id -g)" \
  -t nautilus-dev:issue-11 .

timeout 10m docker run --rm nautilus-dev:issue-11 mise run rust:build
timeout 10m docker run --rm nautilus-dev:issue-11 \
  mise exec -- cargo test --locked -p fuzzer --bin fuzzer \
  python_grammar_loader::tests::loads_and_unparses_script_rule -- --exact
timeout 10m docker run --rm --ulimit core=0 nautilus-dev:issue-11 \
  mise exec -- cargo test --locked -p forksrv tests::run_forkserver -- --exact
```

The last test performs the real AFL++ forkserver handshake and checks normal exit,
SIGABRT, and non-empty coverage. It is not replaced with an uninstrumented target.
Disabling core dumps preserves the SIGABRT result while preventing a host core-dump
handler inherited by Docker from exceeding the test's 200 ms target timeout. The
complete `mise run rust:test` and `mise run check` remain available; issue #12 owns
shared-directory test isolation, so this image does not serialize, retry, or skip
those tests.

For development, mount the checkout and a pre-created writable work directory. Run
the preparation task after mounting because the mount hides the target baked into
the image:

```bash
mkdir -p .docker-work
docker run --rm -it --ulimit core=0 \
  --mount type=bind,src="$PWD",dst=/workspace \
  --mount type=bind,src="$PWD/.docker-work",dst=/work \
  nautilus-dev:issue-11 bash
# inside the container
mise trust mise.toml
mise run target:prepare
```

The image runs as the normal `nautilus` user whose IDs were selected at build time;
it needs neither privileged mode nor host IPC. Put corpora, findings, and other
writable fuzzing artifacts under `/work` rather than an image layer. The build
context excludes host mise/Cargo/Rust homes, compiled targets, and common fuzzing
outputs through `.dockerignore`.

The verified platform is native Linux ARM64; that is this image's initial automation
architecture contract. The Dockerfile also has checked mise downloads for Linux
AMD64, but that path is not yet verified. No emulated or cross-architecture result is
claimed.

> **Trust boundary:** Docker shares the host kernel and is only a reproducible
> development environment here, not sufficient isolation for arbitrary untrusted
> fuzz targets. Use a separately hardened sandbox or VM for those targets. Registry
> publication, a production runtime image, CI wiring, and broader test isolation are
> intentionally outside this image.

### Native prerequisites

mise manages the executables above. Cargo resolves Rust crates, including the PyO3
embedding dependency, from the committed `Cargo.lock`; Cargo dependencies do not
install system software. The host must separately provide Linux, a native C
compiler/linker, and a Python 3.12 installation with headers and a linkable Python
library. The mise Python distribution supplies the Python development files on
supported systems. Running the fork-server test additionally needs AFL++'s
`afl-clang-fast` to build the instrumented target shown above. Distribution package
names vary, and these OS packages and fuzz-target provisioning are outside mise.

## Setup
```bash
# checkout the git
git clone 'git@github.com:nautilus-fuzz/nautilus.git'
cd nautilus
/path/to/AFLplusplus/afl-clang-fast test.c -o test #afl-clang-fast as provided by AFL

# all arguments can also be set using the config.ron file
cargo run --release -- -g grammars/grammar_py_example.py -o /tmp/workdir -- ./test @@

# or if you want to use QEMU mode:
cargo run /path/to/AFLplusplus/afl-qemu-trace -- ./test_bin @@

```

## Examples

Here, we use python to generate a grammar for valid xml-like inputs. Notice the use of a script rule to ensure the the opening
and closing tags match.

```python 
#ctx.rule(NONTERM: string, RHS: string|bytes) adds a rule NONTERM->RHS. We can use {NONTERM} in the RHS to request a recursion. 
ctx.rule("START","<document>{XML_CONTENT}</document>")
ctx.rule("XML_CONTENT","{XML}{XML_CONTENT}")
ctx.rule("XML_CONTENT","")

#ctx.script(NONTERM:string, RHS: [string]], func) adds a rule NONTERM->func(*RHS). 
# In contrast to normal `rule`, RHS is an array of nonterminals. 
# It's up to the function to combine the values returned for the NONTERMINALS with any fixed content used.
ctx.script("XML",["TAG","ATTR","XML_CONTENT"], lambda tag,attr,body: b"<%s %s>%s</%s>"%(tag,attr,body,tag) )
ctx.rule("ATTR","foo=bar")
ctx.rule("TAG","some_tag")
ctx.rule("TAG","other_tag")

#sometimes we don't want to explore the set of possible inputs in more detail. For example, if we fuzz a script
#interpreter, we don't want to spend time on fuzzing all different variable names. In such cases we can use Regex
#terminals. Regex terminals are only mutated during generation, but not during normal mutation stages, saving a lot of time. 
#The fuzzer still explores different values for the regex, but it won't be able to learn interesting values incrementally. 
#Use this when incremantal exploration would most likely waste time.

ctx.regex("TAG","[a-z]+")
```

To test your grammars you can use the generator:

```
$ cargo run --bin generator -- -g grammars/grammar_py_exmaple.py -t 100 
<document><some_tag foo=bar><other_tag foo=bar><other_tag foo=bar><some_tag foo=bar></some_tag></other_tag><some_tag foo=bar><other_tag foo=bar></other_tag></some_tag><other_tag foo=bar></other_tag><some_tag foo=bar></some_tag></other_tag><other_tag foo=bar></other_tag><some_tag foo=bar></some_tag></some_tag></document>
```

You can also use Nautilus in combination with AFL. Simply point AFL `-o` to the same workdir, and AFL will synchronize
with Nautilus. Note that this is one way. AFL imports Nautilus inputs, but not the other way around.

```
#Terminal/Screen 1
./afl-fuzz -Safl -i /tmp/seeds -o /tmp/workdir/ ./test @@

#Terminal/Screen 2
cargo run --release -- -o /tmp/workdir -- ./test @@
```

## Trophies

*  https://github.com/Microsoft/ChakraCore/issues/5503
*  https://github.com/mruby/mruby/issues/3995  (**CVE-2018-10191**)
*  https://github.com/mruby/mruby/issues/4001  (**CVE-2018-10199**)
*  https://github.com/mruby/mruby/issues/4038  (**CVE-2018-12248**)
*  https://github.com/mruby/mruby/issues/4027  (**CVE-2018-11743**)
*  https://github.com/mruby/mruby/issues/4036  (**CVE-2018-12247**)
*  https://github.com/mruby/mruby/issues/4037  (**CVE-2018-12249**)
*  https://bugs.php.net/bug.php?id=76410
*  https://bugs.php.net/bug.php?id=76244
