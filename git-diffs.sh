#!/bin/bash
git diff cav2021..HEAD src/ Cargo.toml > src.diff
git diff cav2021..HEAD -- . ':!src' ':!Cargo.toml' > notsrc.diff