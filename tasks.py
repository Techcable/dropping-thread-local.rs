import os
import shlex
import sys

from invoke import Collection, task

HAS_COLORS: bool = (
    sys.stderr.isatty() or os.getenv("CLICOLOR_FORCE")
) and not os.getenv("NO_COLOR")


def apply_colors(msg: object, /, *, code: str) -> str:
    if HAS_COLORS:
        return f"\x1b[{code}m{msg}\x1b[0m"
    else:
        return str(msg)


def log_info(msg: object):
    print(
        apply_colors("INFO:", code="1;32"),
        apply_colors(msg, code="1"),
    )


@task
def test(ctx):
    check(ctx, format=False)
    ctx.run("cargo nextest run --workspace", pty=True)
    # needed because nextest doesn't support doctests
    ctx.run("cargo test --doc --workspace", pty=True)
    run_format(ctx, check=True)


@task
def check(ctx, format=True):
    clippy(ctx)
    ctx.run(
        "cargo +nightly doc --document-private-items --no-deps --workspace --all-features"
    )
    ctx.run("cargo shear")
    ctx.run("cargo deny check")
    cargo_rdme(ctx, check=True)
    # by default, check formatting as well
    if format:
        run_format(ctx, check=True)


REEDME_SYNC_PKGS = ["dropping-thread-local"]
"""
Packages to sync with cargo-rdme.
"""


@task
def cargo_rdme(ctx, check=False):
    # TODO: Switch to cargo-reedme?
    args = ["cargo", "rdme"]
    if check:
        args.append("--check")
    for pkg in REEDME_SYNC_PKGS:
        args.extend(("-w", pkg))
    ctx.run(shlex.join(args))


@task
def clippy(ctx):
    ctx.run("cargo clippy --workspace --all-targets", pty=True)


@task(name="format")
def run_format(ctx, check=False):
    verb = "Checking" if check else "Fixing"
    log_info(f"{verb} formatting")
    maybe_check = " --check" if check else ""
    maybe_fix = " --fix" if not check else ""
    ctx.run("cargo +nightly fmt --all" + maybe_check)
    ctx.run("tombi format" + maybe_check)

    # need python format for invoke.py
    ctx.run("ruff format" + maybe_check)
    ctx.run("ruff check --select=I" + maybe_fix)  # works like isort
    check_spelling(ctx, fix=False)


TYPOS_VER = "1.51.1"  # pinned to avoid update breakage


@task(name="typos")
def check_spelling(ctx, fix=False):
    maybe_write = " --write-changes" if fix else ""
    ctx.run(f"uvx typos@{TYPOS_VER}" + maybe_write)


ns = Collection(test, check, clippy, cargo_rdme, run_format, check_spelling)
ns.configure(
    {
        "run": {
            "echo": True,
            "env": {"CLICOLOR_FORCE": "1" if HAS_COLORS else "0"},
        }
    }
)
