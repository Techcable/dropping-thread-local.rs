
format: && spellcheck
    taplo format
    cargo +nightly fmt

check: check-format
    -just spellcheck
    cargo clippy

check-format: && spellcheck
    taplo format
    cargo +nightly fmt --check

spellcheck:
    typos
    git log | typos -

fix-spelling:
    typos --write-changes
    git log | typos -
