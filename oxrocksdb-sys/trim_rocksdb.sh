#!/bin/bash

cd "$(dirname "$0")"/rocksdb || return
rm -rf java .circleci .github build_tools coverage db_stress_tool docs examples fuzz java microbench ./**/*_test.cc
git commit -a -m "Makes directory smaller by removing files not used by Oxigraph"