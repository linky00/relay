#!/bin/bash
set -e

if [ -z "$(ls -A /store)" ]; then
  echo "Store is empty. Initializing..."
  relayt init-store store
else
  echo "Store already contains data. Skipping initialization."
fi

exec relayt start config store