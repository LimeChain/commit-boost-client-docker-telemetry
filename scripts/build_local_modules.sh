#!/bin/bash

set -euo pipefail

# docker build -t test_da_commit . -f examples/da_commit/Dockerfile
# docker build -t test_builder_log . -f examples/builder_log/Dockerfile
docker build -t test_preconf . -f examples/preconf/Dockerfile