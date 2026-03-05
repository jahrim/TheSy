#!/bin/bash
git diff --ignore-all-space cav2021..HEAD src/ Cargo.toml > src.diff
git diff --ignore-all-space cav2021..HEAD -- . ':!src' ':!Cargo.toml' > notsrc.diff