# !/bin/bash

git format-patch -1 HEAD --stdout | sed 's/^From: .* <.*>$/From: Anonymous <>/'